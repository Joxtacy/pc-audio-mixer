use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use core_foundation::base::{kCFAllocatorDefault, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;
use core_foundation_sys::array::CFArrayCreate;
use core_foundation_sys::dictionary::{
    CFDictionaryCreateMutable, CFDictionaryRef, CFDictionarySetValue,
};
use core_foundation_sys::string::CFStringRef;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2::msg_send;

use crate::audio::AudioManager;
use crate::types::AudioSession;

// =============================================================================
// CoreAudio FFI types and constants
// =============================================================================

type AudioObjectID = u32;
type OSStatus = i32;
type AudioDeviceIOProcID = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioObjectPropertyAddress {
    m_selector: u32,
    m_scope: u32,
    m_element: u32,
}

// AudioBufferList FFI types for IOProc callback
#[repr(C)]
struct AudioBuffer {
    m_number_channels: u32,
    m_data_byte_size: u32,
    m_data: *mut c_void,
}

// Match the C AudioBufferList layout: the flexible array member starts at the
// offset of mBuffers[1], which #[repr(C)] computes correctly (including
// alignment padding between the u32 count and the first AudioBuffer).
#[repr(C)]
struct AudioBufferList {
    m_number_buffers: u32,
    m_buffers: [AudioBuffer; 1], // First element of the flexible array
}

impl AudioBufferList {
    fn buffers_ptr(&self) -> *const AudioBuffer {
        self.m_buffers.as_ptr()
    }

    fn buffers_mut_ptr(&mut self) -> *mut AudioBuffer {
        self.m_buffers.as_mut_ptr()
    }
}

#[repr(C)]
struct AudioTimeStamp {
    _data: [u8; 64], // Opaque — matches CoreAudio's 64-byte AudioTimeStamp
}

// AudioDeviceIOProc function pointer type
type AudioDeviceIOProc = unsafe extern "C" fn(
    device: AudioObjectID,
    now: *const AudioTimeStamp,
    input_data: *const AudioBufferList,
    input_time: *const AudioTimeStamp,
    output_data: *mut AudioBufferList,
    output_time: *const AudioTimeStamp,
    client_data: *mut c_void,
) -> OSStatus;

// Property selector FourCC constants (big-endian packed bytes)
const K_AUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE: u32 =
    u32::from_be_bytes([b'd', b'O', b'u', b't']); // 'dOut'
const K_AUDIO_HARDWARE_SERVICE_DEVICE_PROPERTY_VIRTUAL_MAIN_VOLUME: u32 =
    u32::from_be_bytes([b'v', b'm', b'v', b'c']); // 'vmvc'
const K_AUDIO_HARDWARE_PROPERTY_PROCESS_OBJECT_LIST: u32 =
    u32::from_be_bytes([b'p', b'r', b's', b'#']); // 'prs#'
const K_AUDIO_PROCESS_PROPERTY_PID: u32 = u32::from_be_bytes([b'p', b'p', b'i', b'd']); // 'ppid'
const K_AUDIO_PROCESS_PROPERTY_BUNDLE_ID: u32 =
    u32::from_be_bytes([b'p', b'b', b'i', b'd']); // 'pbid'
const K_AUDIO_PROCESS_PROPERTY_IS_RUNNING_OUTPUT: u32 =
    u32::from_be_bytes([b'p', b'i', b'r', b'o']); // 'piro'

const K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 =
    u32::from_be_bytes([b'g', b'l', b'o', b'b']); // 'glob'
const K_AUDIO_OBJECT_PROPERTY_SCOPE_OUTPUT: u32 =
    u32::from_be_bytes([b'o', b'u', b't', b'p']); // 'outp'
const K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN: u32 = 0;

const K_AUDIO_OBJECT_SYSTEM_OBJECT: AudioObjectID = 1;

// Aggregate device dictionary keys
const K_AUDIO_AGGREGATE_DEVICE_NAME_KEY: &str = "name";
const K_AUDIO_AGGREGATE_DEVICE_UID_KEY: &str = "uid";
const K_AUDIO_AGGREGATE_DEVICE_TAP_LIST_KEY: &str = "taps";
const K_AUDIO_AGGREGATE_DEVICE_TAP_AUTO_START_KEY: &str = "tap_auto_start";
const K_AUDIO_AGGREGATE_DEVICE_IS_PRIVATE_KEY: &str = "private";

// CATapMuteBehavior values
const CA_TAP_MUTED: i32 = 1;

// =============================================================================
// CoreAudio framework extern functions
// =============================================================================

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyDataSize(
        object_id: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
        qualifier_data_size: u32,
        qualifier_data: *const c_void,
        out_data_size: *mut u32,
    ) -> OSStatus;

    fn AudioObjectGetPropertyData(
        object_id: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
        qualifier_data_size: u32,
        qualifier_data: *const c_void,
        io_data_size: *mut u32,
        out_data: *mut c_void,
    ) -> OSStatus;

    fn AudioObjectSetPropertyData(
        object_id: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
        qualifier_data_size: u32,
        qualifier_data: *const c_void,
        data_size: u32,
        data: *const c_void,
    ) -> OSStatus;

    fn AudioHardwareCreateProcessTap(
        tap_description: *mut c_void, // CATapDescription*
        tap_id: *mut AudioObjectID,
    ) -> OSStatus;

    fn AudioHardwareDestroyProcessTap(tap_id: AudioObjectID) -> OSStatus;

    fn AudioHardwareCreateAggregateDevice(
        description: CFDictionaryRef,
        aggregate_device_id: *mut AudioObjectID,
    ) -> OSStatus;

    fn AudioHardwareDestroyAggregateDevice(aggregate_device_id: AudioObjectID) -> OSStatus;

    fn AudioDeviceCreateIOProcID(
        device_id: AudioObjectID,
        io_proc: AudioDeviceIOProc,
        client_data: *mut c_void,
        out_io_proc_id: *mut AudioDeviceIOProcID,
    ) -> OSStatus;

    fn AudioDeviceDestroyIOProcID(
        device_id: AudioObjectID,
        io_proc_id: AudioDeviceIOProcID,
    ) -> OSStatus;

    fn AudioDeviceStart(
        device_id: AudioObjectID,
        io_proc_id: AudioDeviceIOProcID,
    ) -> OSStatus;

    fn AudioDeviceStop(
        device_id: AudioObjectID,
        io_proc_id: AudioDeviceIOProcID,
    ) -> OSStatus;
}

// =============================================================================
// Helper functions
// =============================================================================

fn check_os_status(status: OSStatus, context: &str) -> Result<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(anyhow!("{}: OSStatus {}", context, status))
    }
}

fn get_default_output_device() -> Result<AudioObjectID> {
    let address = AudioObjectPropertyAddress {
        m_selector: K_AUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
        m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    let mut device_id: AudioObjectID = 0;
    let mut size = std::mem::size_of::<AudioObjectID>() as u32;

    let status = unsafe {
        AudioObjectGetPropertyData(
            K_AUDIO_OBJECT_SYSTEM_OBJECT,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            &mut device_id as *mut _ as *mut c_void,
        )
    };

    check_os_status(status, "get default output device")?;

    if device_id == 0 {
        return Err(anyhow!("No default output device found"));
    }

    Ok(device_id)
}

/// Check if a process with the given PID is still alive.
fn is_process_alive(pid: u32) -> bool {
    // signal(pid, 0) checks process existence without sending a signal
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

// =============================================================================
// Master volume
// =============================================================================

fn get_master_volume_internal() -> Result<f32> {
    let device_id = get_default_output_device()?;

    let address = AudioObjectPropertyAddress {
        m_selector: K_AUDIO_HARDWARE_SERVICE_DEVICE_PROPERTY_VIRTUAL_MAIN_VOLUME,
        m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_OUTPUT,
        m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    let mut volume: f32 = 0.0;
    let mut size = std::mem::size_of::<f32>() as u32;

    let status = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            &mut volume as *mut _ as *mut c_void,
        )
    };

    check_os_status(status, "get master volume")?;

    // CoreAudio returns 0.0-1.0, convert to 0.0-100.0
    Ok(volume * 100.0)
}

fn set_master_volume_internal(volume: f32) -> Result<()> {
    let device_id = get_default_output_device()?;

    let address = AudioObjectPropertyAddress {
        m_selector: K_AUDIO_HARDWARE_SERVICE_DEVICE_PROPERTY_VIRTUAL_MAIN_VOLUME,
        m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_OUTPUT,
        m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    // Convert from 0.0-100.0 to 0.0-1.0 and clamp
    let hw_volume = (volume / 100.0).clamp(0.0, 1.0);

    let status = unsafe {
        AudioObjectSetPropertyData(
            device_id,
            &address,
            0,
            std::ptr::null(),
            std::mem::size_of::<f32>() as u32,
            &hw_volume as *const _ as *const c_void,
        )
    };

    check_os_status(status, "set master volume")
}

// =============================================================================
// Process enumeration
// =============================================================================

// F4 fix: Use wrap_under_create_rule for safety. AudioObjectGetPropertyData
// for CFType properties like bundle ID returns a +1 retained reference that
// the caller must release. wrap_under_create_rule takes ownership correctly.
fn cfstring_ref_to_string(cf_str: CFStringRef) -> Option<String> {
    if cf_str.is_null() {
        return None;
    }
    unsafe {
        let cf = CFString::wrap_under_create_rule(cf_str);
        Some(cf.to_string())
    }
}

// F7 fix: Better process name extraction from bundle IDs.
// Uses a known-names table for common apps, falls back to second-to-last
// component (which is usually the app name), then last component.
fn process_name_from_bundle_id(bundle_id: &str) -> String {
    // Common bundle ID -> display name mappings
    let known: &[(&str, &str)] = &[
        ("com.spotify.client", "Spotify"),
        ("com.apple.Music", "Music"),
        ("com.apple.Safari", "Safari"),
        ("com.google.Chrome", "Chrome"),
        ("org.mozilla.firefox", "Firefox"),
        ("com.microsoft.teams2", "Teams"),
        ("us.zoom.xos", "Zoom"),
        ("com.hnc.Discord", "Discord"),
        ("com.tinyspeck.slackmacgap", "Slack"),
        ("com.apple.FaceTime", "FaceTime"),
        ("org.videolan.vlc", "VLC"),
    ];

    for &(id, name) in known {
        if bundle_id.eq_ignore_ascii_case(id) {
            return name.to_string();
        }
    }

    // Heuristic: for "com.company.AppName" patterns, take the last component.
    // For "com.company.AppName.Helper" style, still take last.
    // Capitalize first letter, preserve rest.
    let parts: Vec<&str> = bundle_id.split('.').collect();
    let candidate = if parts.len() >= 3 {
        // Try last component first; if it's a generic name like "client" or "helper",
        // fall back to second-to-last
        let last = parts[parts.len() - 1];
        let generic = ["client", "helper", "app", "main", "agent", "xpc", "service"];
        if generic.iter().any(|g| last.eq_ignore_ascii_case(g)) && parts.len() >= 3 {
            parts[parts.len() - 2]
        } else {
            last
        }
    } else {
        parts.last().copied().unwrap_or(bundle_id)
    };

    // Capitalize first letter
    let mut chars = candidate.chars();
    match chars.next() {
        None => bundle_id.to_string(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

fn enumerate_audio_sessions_internal(tap_manager: &TapManager) -> Result<Vec<AudioSession>> {
    let mut sessions = Vec::new();

    // Master volume entry
    let master_volume = get_master_volume_internal().unwrap_or(50.0);
    sessions.push(AudioSession {
        process_id: 0,
        process_name: "Master".to_string(),
        display_name: "Master Volume".to_string(),
        volume: master_volume,
        is_muted: false,
    });

    // Get process object list
    let address = AudioObjectPropertyAddress {
        m_selector: K_AUDIO_HARDWARE_PROPERTY_PROCESS_OBJECT_LIST,
        m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    let mut data_size: u32 = 0;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(
            K_AUDIO_OBJECT_SYSTEM_OBJECT,
            &address,
            0,
            std::ptr::null(),
            &mut data_size,
        )
    };

    if status != 0 || data_size == 0 {
        return Ok(sessions);
    }

    let count = data_size as usize / std::mem::size_of::<AudioObjectID>();
    let mut process_ids = vec![0u32; count];

    let status = unsafe {
        AudioObjectGetPropertyData(
            K_AUDIO_OBJECT_SYSTEM_OBJECT,
            &address,
            0,
            std::ptr::null(),
            &mut data_size,
            process_ids.as_mut_ptr() as *mut c_void,
        )
    };

    if status != 0 {
        return Ok(sessions);
    }

    for &process_obj_id in &process_ids {
        // Get PID
        let pid_address = AudioObjectPropertyAddress {
            m_selector: K_AUDIO_PROCESS_PROPERTY_PID,
            m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
            m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
        };

        let mut pid: i32 = 0;
        let mut pid_size = std::mem::size_of::<i32>() as u32;
        let status = unsafe {
            AudioObjectGetPropertyData(
                process_obj_id,
                &pid_address,
                0,
                std::ptr::null(),
                &mut pid_size,
                &mut pid as *mut _ as *mut c_void,
            )
        };

        if status != 0 || pid <= 0 {
            continue;
        }

        // Check if running output
        let running_address = AudioObjectPropertyAddress {
            m_selector: K_AUDIO_PROCESS_PROPERTY_IS_RUNNING_OUTPUT,
            m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
            m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
        };

        let mut is_running: u32 = 0;
        let mut running_size = std::mem::size_of::<u32>() as u32;
        let status = unsafe {
            AudioObjectGetPropertyData(
                process_obj_id,
                &running_address,
                0,
                std::ptr::null(),
                &mut running_size,
                &mut is_running as *mut _ as *mut c_void,
            )
        };

        if status != 0 || is_running == 0 {
            continue;
        }

        // Get bundle ID
        let bundle_address = AudioObjectPropertyAddress {
            m_selector: K_AUDIO_PROCESS_PROPERTY_BUNDLE_ID,
            m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
            m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
        };

        let mut bundle_ref: CFStringRef = std::ptr::null();
        let mut bundle_size = std::mem::size_of::<CFStringRef>() as u32;
        let status = unsafe {
            AudioObjectGetPropertyData(
                process_obj_id,
                &bundle_address,
                0,
                std::ptr::null(),
                &mut bundle_size,
                &mut bundle_ref as *mut _ as *mut c_void,
            )
        };

        let (process_name, display_name) = if status == 0 {
            if let Some(bundle_id) = cfstring_ref_to_string(bundle_ref) {
                let name = process_name_from_bundle_id(&bundle_id);
                (name.clone(), name)
            } else {
                (format!("PID {}", pid), format!("PID {}", pid))
            }
        } else {
            (format!("PID {}", pid), format!("PID {}", pid))
        };

        // Use tap volume if active, otherwise 100.0
        let volume = tap_manager.get_volume(pid as u32).unwrap_or(100.0);

        sessions.push(AudioSession {
            process_id: pid as u32,
            process_name,
            display_name,
            volume,
            is_muted: false,
        });
    }

    Ok(sessions)
}

// =============================================================================
// Tap lifecycle management
// =============================================================================

/// Shared data between the IOProc callback and the main thread.
/// The IOProc reads volume and writes diagnostics via atomics — lock-free.
struct IoCallbackData {
    /// f32 volume factor (0.0-1.0) stored as u32 bits.
    volume: AtomicU32,
    /// Diagnostic: counts down from N, IOProc logs info while > 0.
    diag_countdown: AtomicU32,
    /// Diagnostic: last seen input buffer count.
    diag_in_bufs: AtomicU32,
    /// Diagnostic: last seen output buffer count.
    diag_out_bufs: AtomicU32,
    /// Diagnostic: last seen input data byte size (first buffer).
    diag_in_bytes: AtomicU32,
    /// Diagnostic: last seen output data byte size (first buffer).
    diag_out_bytes: AtomicU32,
    /// Diagnostic: input channel count (first buffer).
    diag_in_channels: AtomicU32,
    /// Diagnostic: output channel count (first buffer).
    diag_out_channels: AtomicU32,
}

struct ProcessTap {
    tap_id: AudioObjectID,
    aggregate_device_id: AudioObjectID,
    io_proc_id: AudioDeviceIOProcID,
    /// Shared data read/written by the real-time audio callback.
    callback_data: Arc<IoCallbackData>,
    _process_id: u32,
}

// SAFETY: ProcessTap's raw pointers (io_proc_id) are only used in CoreAudio API calls
// which are thread-safe. The Arc<AtomicU32> is inherently thread-safe.
unsafe impl Send for ProcessTap {}

struct TapManager {
    active_taps: HashMap<u32, ProcessTap>, // keyed by process_id
}

impl TapManager {
    fn new() -> Self {
        Self {
            active_taps: HashMap::new(),
        }
    }

    fn create_tap(&mut self, pid: u32, initial_volume: f32) -> Result<()> {
        if self.active_taps.contains_key(&pid) {
            self.set_volume(pid, initial_volume);
            return Ok(());
        }

        // 1. Create CATapDescription via Obj-C
        let tap_description = unsafe { create_tap_description(pid)? };

        // 2. Create the process tap
        let mut tap_id: AudioObjectID = 0;
        let tap_desc_ptr = Retained::as_ptr(&tap_description) as *mut c_void;
        log::info!("Calling AudioHardwareCreateProcessTap with desc ptr {:?}", tap_desc_ptr);
        let status = unsafe {
            AudioHardwareCreateProcessTap(tap_desc_ptr, &mut tap_id)
        };
        if status != 0 {
            // Decode OSStatus as FourCC for readable error
            let bytes = status.to_be_bytes();
            let fourcc: String = bytes.iter().map(|&b| {
                if b.is_ascii_graphic() || b == b' ' { b as char } else { '?' }
            }).collect();
            log::error!(
                "AudioHardwareCreateProcessTap failed: OSStatus {} ('{}')",
                status, fourcc
            );
            return Err(anyhow!("AudioHardwareCreateProcessTap: OSStatus {} ('{}')", status, fourcc));
        }

        if tap_id == 0 {
            return Err(anyhow!("AudioHardwareCreateProcessTap returned tap_id=0 (invalid)"));
        }

        // 3. Get tap UUID for aggregate device config
        let tap_uuid = unsafe { get_tap_uuid(&tap_description)? };
        log::info!("Process tap created: tap_id={}, UUID={}", tap_id, tap_uuid);

        // 4. Create aggregate device
        let aggregate_device_id = match create_aggregate_device(&tap_uuid, pid) {
            Ok(id) => id,
            Err(e) => {
                unsafe { AudioHardwareDestroyProcessTap(tap_id); }
                return Err(e);
            }
        };

        // 5. Create shared callback data
        let volume_factor = (initial_volume / 100.0).clamp(0.0, 1.0);
        let callback_data = Arc::new(IoCallbackData {
            volume: AtomicU32::new(f32::to_bits(volume_factor)),
            diag_countdown: AtomicU32::new(5),
            diag_in_bufs: AtomicU32::new(0),
            diag_out_bufs: AtomicU32::new(0),
            diag_in_bytes: AtomicU32::new(0),
            diag_out_bytes: AtomicU32::new(0),
            diag_in_channels: AtomicU32::new(0),
            diag_out_channels: AtomicU32::new(0),
        });

        // 6. Create IOProc with function pointer and start
        let io_proc_id = match create_and_start_io_proc(aggregate_device_id, &callback_data) {
            Ok(id) => id,
            Err(e) => {
                unsafe {
                    AudioHardwareDestroyAggregateDevice(aggregate_device_id);
                    AudioHardwareDestroyProcessTap(tap_id);
                }
                return Err(e);
            }
        };

        self.active_taps.insert(
            pid,
            ProcessTap {
                tap_id,
                aggregate_device_id,
                io_proc_id,
                callback_data,
                _process_id: pid,
            },
        );

        log::info!(
            "Created audio tap for PID {} (tap_id={}, agg_id={})",
            pid,
            tap_id,
            aggregate_device_id
        );
        Ok(())
    }

    fn destroy_tap(&mut self, pid: u32) {
        if let Some(tap) = self.active_taps.remove(&pid) {
            destroy_tap_resources(&tap);
            log::info!("Destroyed audio tap for PID {}", pid);
        }
    }

    fn set_volume(&self, pid: u32, volume: f32) {
        if let Some(tap) = self.active_taps.get(&pid) {
            let factor = (volume / 100.0).clamp(0.0, 1.0);
            tap.callback_data.volume.store(f32::to_bits(factor), Ordering::Relaxed);
        }
    }

    fn get_volume(&self, pid: u32) -> Option<f32> {
        self.active_taps.get(&pid).map(|tap| {
            let bits = tap.callback_data.volume.load(Ordering::Relaxed);
            f32::from_bits(bits) * 100.0
        })
    }

    // F2 fix: Reap taps for processes that have exited.
    fn reap_dead_taps(&mut self) {
        let dead_pids: Vec<u32> = self
            .active_taps
            .keys()
            .copied()
            .filter(|&pid| !is_process_alive(pid))
            .collect();

        for pid in dead_pids {
            log::info!("Reaping tap for dead process PID {}", pid);
            self.destroy_tap(pid);
        }
    }

    fn cleanup(&mut self) {
        let pids: Vec<u32> = self.active_taps.keys().copied().collect();
        for pid in pids {
            if let Some(tap) = self.active_taps.remove(&pid) {
                destroy_tap_resources(&tap);
            }
        }
    }
}

// F1 fix: After stopping the IOProc, destroy the IOProcID (which ensures the
// callback is fully quiesced — AudioDeviceDestroyIOProcID waits for any
// in-flight invocation to complete), then sleep briefly as an extra safety
// margin before releasing the Arc.
fn destroy_tap_resources(tap: &ProcessTap) {
    unsafe {
        // Stop accepting new callbacks
        let _ = AudioDeviceStop(tap.aggregate_device_id, tap.io_proc_id);
        // DestroyIOProcID waits for any in-flight callback to finish
        let _ = AudioDeviceDestroyIOProcID(tap.aggregate_device_id, tap.io_proc_id);
    }
    // Extra safety margin: give the audio thread time to fully exit the callback.
    // AudioDeviceDestroyIOProcID should handle this, but belt-and-suspenders
    // for a use-after-free scenario.
    std::thread::sleep(std::time::Duration::from_millis(5));
    unsafe {
        let _ = AudioHardwareDestroyAggregateDevice(tap.aggregate_device_id);
        let _ = AudioHardwareDestroyProcessTap(tap.tap_id);
    }
    // Now it's safe to drop the ProcessTap (and its Arc<AtomicU32>).
}

// =============================================================================
// Obj-C interop for CATapDescription
// =============================================================================

/// Translate a Unix PID to a CoreAudio process AudioObjectID.
fn pid_to_audio_object(pid: u32) -> Result<AudioObjectID> {
    let address = AudioObjectPropertyAddress {
        m_selector: u32::from_be_bytes([b'i', b'd', b'2', b'p']), // 'id2p'
        m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    let mut audio_obj_id: AudioObjectID = 0;
    let mut size = std::mem::size_of::<AudioObjectID>() as u32;
    let pid_val: u32 = pid;

    let status = unsafe {
        AudioObjectGetPropertyData(
            K_AUDIO_OBJECT_SYSTEM_OBJECT,
            &address,
            std::mem::size_of::<u32>() as u32,
            &pid_val as *const _ as *const c_void,
            &mut size,
            &mut audio_obj_id as *mut _ as *mut c_void,
        )
    };

    check_os_status(status, "translate PID to AudioObjectID")?;

    if audio_obj_id == 0 {
        return Err(anyhow!("No audio object found for PID {}", pid));
    }

    Ok(audio_obj_id)
}

/// Log available CATapDescription initializers for debugging.
unsafe fn log_tap_description_methods() {
    use std::ffi::CStr;

    let tap_desc_class = match AnyClass::get(c"CATapDescription") {
        Some(cls) => cls,
        None => return,
    };

    // Use Objective-C runtime to enumerate instance methods
    extern "C" {
        fn class_copyMethodList(
            cls: *const std::ffi::c_void,
            out_count: *mut u32,
        ) -> *mut *const std::ffi::c_void;
        fn method_getName(method: *const std::ffi::c_void) -> *const std::ffi::c_void;
        fn sel_getName(sel: *const std::ffi::c_void) -> *const std::ffi::c_char;
    }

    let mut count: u32 = 0;
    let methods = class_copyMethodList(
        tap_desc_class as *const AnyClass as *const std::ffi::c_void,
        &mut count,
    );
    if methods.is_null() {
        return;
    }

    let mut init_methods = Vec::new();
    for i in 0..count as isize {
        let method = *methods.offset(i);
        let sel = method_getName(method);
        let name = CStr::from_ptr(sel_getName(sel));
        if let Ok(s) = name.to_str() {
            if s.starts_with("init") {
                init_methods.push(s.to_string());
            }
        }
    }
    libc::free(methods as *mut _);

    log::info!("CATapDescription init methods: {:?}", init_methods);
}

/// Get the first output stream AudioObjectID for a device.
fn get_first_output_stream(device_id: AudioObjectID) -> Result<AudioObjectID> {
    let address = AudioObjectPropertyAddress {
        m_selector: u32::from_be_bytes([b's', b't', b'm', b'#']), // 'stm#'
        m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_OUTPUT,
        m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    let mut data_size: u32 = 0;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(device_id, &address, 0, std::ptr::null(), &mut data_size)
    };
    check_os_status(status, "get output stream count")?;

    let count = data_size as usize / std::mem::size_of::<AudioObjectID>();
    if count == 0 {
        return Err(anyhow!("No output streams on device {}", device_id));
    }

    let mut stream_ids = vec![0u32; count];
    let status = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &address,
            0,
            std::ptr::null(),
            &mut data_size,
            stream_ids.as_mut_ptr() as *mut c_void,
        )
    };
    check_os_status(status, "get output streams")?;

    Ok(stream_ids[0])
}

unsafe fn create_tap_description(pid: u32) -> Result<Retained<AnyObject>> {
    // Log available methods once for debugging
    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, Ordering::Relaxed) {
        log_tap_description_methods();
    }

    // Translate Unix PID to CoreAudio process AudioObjectID
    let audio_obj_id = pid_to_audio_object(pid)?;
    log::info!(
        "PID {} -> AudioObjectID {}, creating CATapDescription",
        pid,
        audio_obj_id
    );

    let ns_number_class =
        AnyClass::get(c"NSNumber").ok_or_else(|| anyhow!("NSNumber class not found"))?;
    let obj_id_number: *mut AnyObject =
        msg_send![ns_number_class, numberWithUnsignedInt: audio_obj_id];
    if obj_id_number.is_null() {
        return Err(anyhow!("Failed to create NSNumber for AudioObjectID"));
    }

    let ns_array_class =
        AnyClass::get(c"NSArray").ok_or_else(|| anyhow!("NSArray class not found"))?;
    let processes_array: *mut AnyObject =
        msg_send![ns_array_class, arrayWithObject: obj_id_number];
    if processes_array.is_null() {
        return Err(anyhow!("Failed to create NSArray"));
    }

    let tap_desc_class = AnyClass::get(c"CATapDescription")
        .ok_or_else(|| anyhow!("CATapDescription class not found — requires macOS 14.2+"))?;

    let tap_desc: *mut AnyObject = msg_send![tap_desc_class, alloc];
    if tap_desc.is_null() {
        return Err(anyhow!("Failed to alloc CATapDescription"));
    }

    let tap_desc: *mut AnyObject =
        msg_send![tap_desc, initStereoMixdownOfProcesses: processes_array];
    if tap_desc.is_null() {
        return Err(anyhow!(
            "initStereoMixdownOfProcesses: returned nil for AudioObjectID {}",
            audio_obj_id
        ));
    }

    log::info!("CATapDescription created successfully");

    // Set muteBehavior = CATapMuted (1) to intercept the process's audio.
    // The IOProc will read from input and write volume-scaled audio to output.
    let _: () = msg_send![tap_desc, setMuteBehavior: CA_TAP_MUTED];

    // Set as private tap
    let _: () = msg_send![tap_desc, setPrivate: Bool::YES];

    // Retain into a Retained wrapper for safe memory management
    Retained::retain(tap_desc).ok_or_else(|| anyhow!("Failed to retain CATapDescription"))
}

unsafe fn get_tap_uuid(tap_description: &Retained<AnyObject>) -> Result<String> {
    let uuid: *mut AnyObject = msg_send![&**tap_description, UUID];
    if uuid.is_null() {
        return Err(anyhow!("CATapDescription UUID is null"));
    }

    let uuid_string: *mut AnyObject = msg_send![uuid, UUIDString];
    if uuid_string.is_null() {
        return Err(anyhow!("Failed to get UUID string"));
    }

    let utf8: *const std::ffi::c_char = msg_send![uuid_string, UTF8String];
    if utf8.is_null() {
        return Err(anyhow!("Failed to get UTF8 string from UUID"));
    }

    Ok(std::ffi::CStr::from_ptr(utf8)
        .to_string_lossy()
        .into_owned())
}

// =============================================================================
// Aggregate device creation
// =============================================================================

/// Get the UID string for an audio device (needed for aggregate sub-device list).
fn get_device_uid(device_id: AudioObjectID) -> Result<String> {
    let address = AudioObjectPropertyAddress {
        m_selector: u32::from_be_bytes([b'u', b'i', b'd', b' ']), // 'uid ' = kAudioDevicePropertyDeviceUID
        m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    let mut uid_ref: CFStringRef = std::ptr::null();
    let mut size = std::mem::size_of::<CFStringRef>() as u32;

    let status = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            &mut uid_ref as *mut _ as *mut c_void,
        )
    };

    check_os_status(status, "get device UID")?;
    cfstring_ref_to_string(uid_ref).ok_or_else(|| anyhow!("Device UID is null"))
}

fn create_aggregate_device(tap_uuid: &str, pid: u32) -> Result<AudioObjectID> {
    let output_device_id = get_default_output_device()?;
    let output_uid = get_device_uid(output_device_id)?;
    log::info!(
        "Creating aggregate device for PID {} with tap UUID: {}, output UID: {}",
        pid,
        tap_uuid,
        output_uid
    );

    unsafe {
        let dict = CFDictionaryCreateMutable(
            kCFAllocatorDefault,
            0,
            &core_foundation_sys::dictionary::kCFTypeDictionaryKeyCallBacks,
            &core_foundation_sys::dictionary::kCFTypeDictionaryValueCallBacks,
        );
        if dict.is_null() {
            return Err(anyhow!("Failed to create CFMutableDictionary"));
        }

        // Ensure dict is released on all exit paths
        let _dict_guard = scopeguard::guard(dict, |d| {
            core_foundation_sys::base::CFRelease(d as *const c_void);
        });

        let name = CFString::new(&format!("PCMixer_Tap_{}", pid));
        let uid = CFString::new(&format!(
            "com.joxtacy.pcaudiomixer.tap.{}.{}",
            pid,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ));

        let key_name = CFString::new(K_AUDIO_AGGREGATE_DEVICE_NAME_KEY);
        let key_uid = CFString::new(K_AUDIO_AGGREGATE_DEVICE_UID_KEY);
        let key_taps = CFString::new(K_AUDIO_AGGREGATE_DEVICE_TAP_LIST_KEY);
        let key_auto_start = CFString::new(K_AUDIO_AGGREGATE_DEVICE_TAP_AUTO_START_KEY);
        let key_private = CFString::new(K_AUDIO_AGGREGATE_DEVICE_IS_PRIVATE_KEY);
        let key_sub_devices = CFString::new("subdevices"); // kAudioAggregateDeviceSubDeviceListKey

        CFDictionarySetValue(dict, key_name.as_CFTypeRef(), name.as_CFTypeRef());
        CFDictionarySetValue(dict, key_uid.as_CFTypeRef(), uid.as_CFTypeRef());

        // Sub-device list: array of DICTIONARIES (not bare strings!).
        // Each entry needs a "uid" key (kAudioSubDeviceUIDKey).
        let output_uid_cf = CFString::new(&output_uid);
        let sub_device_entry = CFDictionaryCreateMutable(
            kCFAllocatorDefault,
            0,
            &core_foundation_sys::dictionary::kCFTypeDictionaryKeyCallBacks,
            &core_foundation_sys::dictionary::kCFTypeDictionaryValueCallBacks,
        );
        if sub_device_entry.is_null() {
            return Err(anyhow!("Failed to create sub-device entry dict"));
        }
        let key_sub_device_uid = CFString::new("uid"); // kAudioSubDeviceUIDKey
        CFDictionarySetValue(
            sub_device_entry,
            key_sub_device_uid.as_CFTypeRef(),
            output_uid_cf.as_CFTypeRef(),
        );
        let sub_device_ref = sub_device_entry as *const c_void;
        let sub_device_array = CFArrayCreate(
            kCFAllocatorDefault,
            &sub_device_ref as *const _,
            1,
            &core_foundation_sys::array::kCFTypeArrayCallBacks,
        );
        core_foundation_sys::base::CFRelease(sub_device_entry as *const c_void);
        if sub_device_array.is_null() {
            return Err(anyhow!("Failed to create sub-device array"));
        }
        CFDictionarySetValue(
            dict,
            key_sub_devices.as_CFTypeRef(),
            sub_device_array as *const c_void,
        );
        core_foundation_sys::base::CFRelease(sub_device_array as *const c_void);

        // Tap list: array of dictionaries, each with kAudioSubTapUIDKey ("uid").
        let tap_entry_dict = CFDictionaryCreateMutable(
            kCFAllocatorDefault,
            0,
            &core_foundation_sys::dictionary::kCFTypeDictionaryKeyCallBacks,
            &core_foundation_sys::dictionary::kCFTypeDictionaryValueCallBacks,
        );
        if tap_entry_dict.is_null() {
            return Err(anyhow!("Failed to create tap entry dictionary"));
        }
        let key_sub_tap_uid = CFString::new("uid"); // kAudioSubTapUIDKey
        let tap_uuid_cf = CFString::new(tap_uuid);
        CFDictionarySetValue(
            tap_entry_dict,
            key_sub_tap_uid.as_CFTypeRef(),
            tap_uuid_cf.as_CFTypeRef(),
        );

        let tap_entry_ref = tap_entry_dict as *const c_void;
        let tap_array = CFArrayCreate(
            kCFAllocatorDefault,
            &tap_entry_ref as *const _,
            1,
            &core_foundation_sys::array::kCFTypeArrayCallBacks,
        );
        core_foundation_sys::base::CFRelease(tap_entry_dict as *const c_void);

        if tap_array.is_null() {
            return Err(anyhow!("Failed to create tap list array"));
        }

        CFDictionarySetValue(dict, key_taps.as_CFTypeRef(), tap_array as *const c_void);
        core_foundation_sys::base::CFRelease(tap_array as *const c_void);

        CFDictionarySetValue(
            dict,
            key_auto_start.as_CFTypeRef(),
            CFBoolean::true_value().as_CFTypeRef(),
        );
        CFDictionarySetValue(
            dict,
            key_private.as_CFTypeRef(),
            CFBoolean::true_value().as_CFTypeRef(),
        );

        let mut aggregate_device_id: AudioObjectID = 0;
        let status = AudioHardwareCreateAggregateDevice(
            dict as CFDictionaryRef,
            &mut aggregate_device_id,
        );

        check_os_status(status, "AudioHardwareCreateAggregateDevice")?;
        log::info!("Aggregate device created: {}", aggregate_device_id);

        // Give CoreAudio time to configure streams on the new aggregate device.
        // Stream setup happens asynchronously after creation returns.
        std::thread::sleep(std::time::Duration::from_millis(100));

        Ok(aggregate_device_id)
    }
}

// =============================================================================
// IOProc callback (C function pointer — runs on real-time audio thread)
// =============================================================================

/// Real-time audio callback. MUST NOT allocate, lock, or block.
/// Reads tapped audio from input, scales by volume factor, writes to output.
unsafe extern "C" fn audio_io_proc(
    _device: AudioObjectID,
    _now: *const AudioTimeStamp,
    input_data: *const AudioBufferList,
    _input_time: *const AudioTimeStamp,
    output_data: *mut AudioBufferList,
    _output_time: *const AudioTimeStamp,
    client_data: *mut c_void,
) -> OSStatus {
    if client_data.is_null() {
        return 0;
    }

    let cb_data = &*(client_data as *const IoCallbackData);
    let vol_factor = f32::from_bits(cb_data.volume.load(Ordering::Relaxed));

    let in_num = if !input_data.is_null() { (*input_data).m_number_buffers } else { 0 };
    let out_num = if !output_data.is_null() { (*output_data).m_number_buffers } else { 0 };

    // Store diagnostic info for the first few calls (read by main thread)
    let countdown = cb_data.diag_countdown.load(Ordering::Relaxed);
    if countdown > 0 {
        cb_data.diag_countdown.store(countdown - 1, Ordering::Relaxed);
        cb_data.diag_in_bufs.store(in_num, Ordering::Relaxed);
        cb_data.diag_out_bufs.store(out_num, Ordering::Relaxed);
        if !input_data.is_null() && in_num > 0 {
            let first_in = &*(*input_data).buffers_ptr();
            cb_data.diag_in_bytes.store(first_in.m_data_byte_size, Ordering::Relaxed);
            cb_data.diag_in_channels.store(first_in.m_number_channels, Ordering::Relaxed);
        }
        if !output_data.is_null() && out_num > 0 {
            let first_out = &*(*output_data).buffers_ptr();
            cb_data.diag_out_bytes.store(first_out.m_data_byte_size, Ordering::Relaxed);
            cb_data.diag_out_channels.store(first_out.m_number_channels, Ordering::Relaxed);
        }
    }

    if input_data.is_null() || output_data.is_null() {
        return 0;
    }

    // Copy input (tapped audio) to output (speakers), scaled by volume factor
    let num_buffers = in_num.min(out_num) as usize;
    let in_bufs = (*input_data).buffers_ptr();
    let out_bufs = (*output_data).buffers_mut_ptr();

    for i in 0..num_buffers {
        let in_buf = &*in_bufs.add(i);
        let out_buf = &mut *out_bufs.add(i);

        if in_buf.m_data.is_null() || out_buf.m_data.is_null() {
            continue;
        }

        let sample_count = in_buf.m_data_byte_size.min(out_buf.m_data_byte_size) as usize
            / std::mem::size_of::<f32>();

        let in_samples = std::slice::from_raw_parts(in_buf.m_data as *const f32, sample_count);
        let out_samples =
            std::slice::from_raw_parts_mut(out_buf.m_data as *mut f32, sample_count);

        for j in 0..sample_count {
            out_samples[j] = in_samples[j] * vol_factor;
        }
    }

    0 // noErr
}

/// Query the number of streams on a device for a given scope.
fn get_stream_count(device_id: AudioObjectID, scope: u32) -> u32 {
    let address = AudioObjectPropertyAddress {
        m_selector: u32::from_be_bytes([b's', b't', b'm', b'#']), // 'stm#' = kAudioDevicePropertyStreams
        m_scope: scope,
        m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    let mut data_size: u32 = 0;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(device_id, &address, 0, std::ptr::null(), &mut data_size)
    };

    if status != 0 {
        return 0;
    }

    data_size / std::mem::size_of::<AudioObjectID>() as u32
}

fn create_and_start_io_proc(
    device_id: AudioObjectID,
    callback_data: &Arc<IoCallbackData>,
) -> Result<AudioDeviceIOProcID> {
    // Diagnostic: check streams on the aggregate device
    let input_streams = get_stream_count(device_id, u32::from_be_bytes([b'i', b'n', b'p', b't'])); // 'inpt'
    let output_streams = get_stream_count(device_id, K_AUDIO_OBJECT_PROPERTY_SCOPE_OUTPUT);
    log::info!(
        "Aggregate device {} streams: {} input, {} output",
        device_id,
        input_streams,
        output_streams
    );

    // Pass a raw pointer to the IoCallbackData as client_data.
    // The Arc is kept alive by the ProcessTap struct. We must ensure
    // the IOProc is destroyed before the Arc is dropped (see destroy_tap_resources).
    let client_data = Arc::as_ptr(callback_data) as *mut c_void;

    let mut io_proc_id: AudioDeviceIOProcID = std::ptr::null_mut();

    let status = unsafe {
        AudioDeviceCreateIOProcID(device_id, audio_io_proc, client_data, &mut io_proc_id)
    };

    check_os_status(status, "AudioDeviceCreateIOProcID")?;
    log::info!("IOProc registered on device {}, starting...", device_id);

    // Start the device IO
    let status = unsafe { AudioDeviceStart(device_id, io_proc_id) };
    if status != 0 {
        let bytes = status.to_be_bytes();
        let fourcc: String = bytes
            .iter()
            .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '?' })
            .collect();
        log::error!(
            "AudioDeviceStart failed on device {}: OSStatus {} ('{}')",
            device_id,
            status,
            fourcc
        );
        unsafe {
            AudioDeviceDestroyIOProcID(device_id, io_proc_id);
        }
        return Err(anyhow!("AudioDeviceStart failed: OSStatus {} ('{}')", status, fourcc));
    }

    // Wait briefly then log IOProc diagnostic data
    std::thread::sleep(std::time::Duration::from_millis(50));
    log::info!(
        "IOProc diagnostics: in_bufs={} ({}ch, {}B), out_bufs={} ({}ch, {}B)",
        callback_data.diag_in_bufs.load(Ordering::Relaxed),
        callback_data.diag_in_channels.load(Ordering::Relaxed),
        callback_data.diag_in_bytes.load(Ordering::Relaxed),
        callback_data.diag_out_bufs.load(Ordering::Relaxed),
        callback_data.diag_out_channels.load(Ordering::Relaxed),
        callback_data.diag_out_bytes.load(Ordering::Relaxed),
    );

    Ok(io_proc_id)
}

// =============================================================================
// MacosAudioManager — public API
// =============================================================================

pub struct MacosAudioManager {
    tap_manager: Mutex<TapManager>,
}

impl MacosAudioManager {
    pub fn new() -> Self {
        Self {
            tap_manager: Mutex::new(TapManager::new()),
        }
    }
}

impl Default for MacosAudioManager {
    fn default() -> Self {
        Self::new()
    }
}

// F5 fix: Force cleanup even if mutex is poisoned.
impl Drop for MacosAudioManager {
    fn drop(&mut self) {
        let mut manager = self
            .tap_manager
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        manager.cleanup();
    }
}

impl AudioManager for MacosAudioManager {
    fn get_audio_sessions(&self) -> Result<Vec<AudioSession>> {
        let mut manager = self
            .tap_manager
            .lock()
            .map_err(|e| anyhow!("Failed to lock tap manager: {}", e))?;

        // F2 fix: Reap taps for dead processes on each session poll
        manager.reap_dead_taps();

        enumerate_audio_sessions_internal(&manager)
    }

    fn set_app_volume(&self, process_id: u32, volume: f32) -> Result<()> {
        if process_id == 0 {
            return set_master_volume_internal(volume);
        }

        let mut manager = self
            .tap_manager
            .lock()
            .map_err(|e| anyhow!("Failed to lock tap manager: {}", e))?;

        if manager.active_taps.contains_key(&process_id) {
            manager.set_volume(process_id, volume);
        } else {
            manager.create_tap(process_id, volume)?;
        }

        Ok(())
    }

    fn set_master_volume(&self, volume: f32) -> Result<()> {
        set_master_volume_internal(volume)
    }

    fn get_master_volume(&self) -> Result<f32> {
        get_master_volume_internal()
    }

    // F6 fix: Separate volume updates (fast, lock-only) from tap creation (slow,
    // requires CoreAudio calls). Collect new taps needed, release lock for the
    // fast path, then create taps individually with short lock holds.
    fn set_volumes_batch(&self, volumes: &[(u32, f32)]) -> Result<()> {
        // First pass: update existing taps and collect PIDs that need new taps
        let new_taps: Vec<(u32, f32)> = {
            let manager = self
                .tap_manager
                .lock()
                .map_err(|e| anyhow!("Failed to lock tap manager: {}", e))?;

            let mut needs_tap = Vec::new();
            for &(pid, volume) in volumes {
                if pid == 0 {
                    let _ = set_master_volume_internal(volume);
                } else if manager.active_taps.contains_key(&pid) {
                    manager.set_volume(pid, volume);
                } else {
                    needs_tap.push((pid, volume));
                }
            }
            needs_tap
        }; // lock released here

        // Second pass: create new taps (blocking CoreAudio calls) with minimal lock holds
        for (pid, volume) in new_taps {
            let mut manager = self
                .tap_manager
                .lock()
                .map_err(|e| anyhow!("Failed to lock tap manager: {}", e))?;
            if let Err(e) = manager.create_tap(pid, volume) {
                log::error!("Failed to create tap for PID {}: {}", pid, e);
            }
            // Lock is released each iteration, giving other threads a chance
        }

        Ok(())
    }
}
