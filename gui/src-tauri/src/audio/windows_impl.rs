use anyhow::{anyhow, Result};

use crate::audio::AudioManager;
use crate::types::AudioSession;

/// Initialize COM on the current thread.
///
/// COM must be initialized per-thread on Windows. This function is safe to call
/// multiple times on the same thread (returns S_FALSE) and handles the case where
/// the thread was already initialized with a different apartment model by
/// Tauri/WebView2 (RPC_E_CHANGED_MODE).
fn ensure_com_initialized() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

        const RPC_E_CHANGED_MODE: i32 = -2147417850; // 0x80010106

        // SAFETY: CoInitializeEx is safe to call multiple times per thread.
        // S_FALSE is returned if COM is already initialized with the same mode.
        // RPC_E_CHANGED_MODE is returned if already initialized with a different
        // mode (e.g., STA by Tauri/WebView2), which is fine — COM is still usable.
        unsafe {
            let hr = CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED as u32);
            if hr >= 0 || hr == RPC_E_CHANGED_MODE {
                Ok(())
            } else {
                log::error!("Failed to initialize COM on thread: 0x{:08x}", hr);
                Err(anyhow!("COM initialization failed: 0x{:08x}", hr))
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    Ok(())
}

#[cfg(target_os = "windows")]
mod windows_audio {
    use super::*;
    use std::ffi::c_void;
    use std::mem;
    use std::ptr;
    use windows_sys::core::GUID;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Media::Audio::*;
    use windows_sys::Win32::System::Com::*;

    // Type aliases for missing types in windows-sys
    type BOOL = i32;
    type HRESULT = i32;
    type PWSTR = *mut u16;
    type HANDLE = *mut c_void;

    /// RAII wrapper for COM interfaces - automatically calls Release on drop
    struct ComPtr {
        ptr: *mut c_void,
        vtbl_offset: usize, // Offset to IUnknown vtable for Release
    }

    impl ComPtr {
        fn new(ptr: *mut c_void) -> Option<Self> {
            if ptr.is_null() {
                None
            } else {
                Some(Self {
                    ptr,
                    vtbl_offset: 0,
                })
            }
        }
    }

    impl Drop for ComPtr {
        fn drop(&mut self) {
            if !self.ptr.is_null() {
                unsafe {
                    // SAFETY: We only create ComPtr with valid COM interfaces
                    let vtbl = *(self.ptr as *mut *mut IUnknownVtbl);
                    ((*vtbl).Release)(self.ptr);
                }
            }
        }
    }

    // COM Interface GUIDs
    const CLSID_MMDEVICEENUMERATOR: GUID = GUID {
        data1: 0xBCDE0395,
        data2: 0xE52F,
        data3: 0x467C,
        data4: [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E],
    };

    const IID_IMMDEVICEENUMERATOR: GUID = GUID {
        data1: 0xA95664D2,
        data2: 0x9614,
        data3: 0x4F35,
        data4: [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6],
    };

    const IID_IAUDIOSESSIONMANAGER2: GUID = GUID {
        data1: 0x77AA99A0,
        data2: 0x1BD6,
        data3: 0x484F,
        data4: [0x8B, 0xC7, 0x2C, 0x65, 0x4C, 0x9A, 0x9B, 0x6F],
    };

    const IID_IAUDIOENDPOINTVOLUME: GUID = GUID {
        data1: 0x5CDF2C82,
        data2: 0x841E,
        data3: 0x4546,
        data4: [0x97, 0x22, 0x0C, 0xF7, 0x40, 0x78, 0x22, 0x9A],
    };

    fn get_process_name_from_id(pid: u32) -> Option<String> {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::ProcessStatus::{
            GetModuleFileNameExW, GetProcessImageFileNameW,
        };
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        unsafe {
            // Try to open the process with minimum required permissions
            let process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false as _, pid);

            if process_handle.is_null() {
                return None;
            }

            // Ensure handle is closed when we're done
            let _guard = scopeguard::guard(process_handle, |h| {
                let _ = CloseHandle(h);
            });

            const MAX_PATH: usize = 260; // Windows MAX_PATH constant
            let mut buffer = [0u16; MAX_PATH];

            // SAFETY: GetModuleFileNameExW writes at most MAX_PATH characters to buffer.
            // We've allocated exactly MAX_PATH u16s, preventing buffer overflow.
            let len = GetModuleFileNameExW(
                process_handle,
                std::ptr::null_mut(),
                buffer.as_mut_ptr(),
                MAX_PATH as u32,
            );

            let final_len = if len == 0 {
                // SAFETY: GetProcessImageFileNameW writes at most MAX_PATH characters.
                // Buffer is properly sized and process_handle is valid (checked above).
                let len =
                    GetProcessImageFileNameW(process_handle, buffer.as_mut_ptr(), MAX_PATH as u32);
                if len == 0 {
                    return None;
                }
                // Ensure we never read beyond buffer bounds
                len.min(MAX_PATH as u32)
            } else {
                // Ensure we never read beyond buffer bounds
                len.min(MAX_PATH as u32)
            };

            let path = OsString::from_wide(&buffer[..final_len as usize]);
            let path_str = path.to_string_lossy();

            // Extract just the filename from the full path
            path_str.split('\\').last().map(|s| s.to_string())
        }
    }

    /// Gets the default audio device.
    ///
    /// # Safety
    /// Caller must ensure COM is initialized.
    /// The returned pointer must be released with Release() when done.
    pub unsafe fn get_default_audio_device() -> Result<*mut c_void> {
        let mut enumerator: *mut c_void = ptr::null_mut();

        // SAFETY: CoCreateInstance is safe with valid GUIDs and null-initialized output pointer
        let hr = CoCreateInstance(
            &CLSID_MMDEVICEENUMERATOR,
            ptr::null_mut(),
            CLSCTX_ALL,
            &IID_IMMDEVICEENUMERATOR,
            &mut enumerator as *mut _ as *mut *mut c_void,
        );

        if hr < 0 {
            return Err(anyhow!("Failed to create MMDeviceEnumerator: 0x{:08x}", hr));
        }

        if enumerator.is_null() {
            return Err(anyhow!("MMDeviceEnumerator returned null"));
        }

        let enumerator_vtbl = *(enumerator as *mut *mut IMMDeviceEnumeratorVtbl);
        let mut device: *mut c_void = ptr::null_mut();

        let hr = ((*enumerator_vtbl).GetDefaultAudioEndpoint)(
            enumerator,
            eRender,
            eConsole,
            &mut device,
        );

        // Release enumerator
        ((*enumerator_vtbl).parent.Release)(enumerator);

        if hr < 0 {
            return Err(anyhow!(
                "Failed to get default audio endpoint: 0x{:08x}",
                hr
            ));
        }

        Ok(device)
    }

    pub unsafe fn get_audio_session_manager(device: *mut c_void) -> Result<*mut c_void> {
        let device_vtbl = *(device as *mut *mut IMMDeviceVtbl);
        let mut session_manager: *mut c_void = ptr::null_mut();

        let hr = ((*device_vtbl).Activate)(
            device,
            &IID_IAUDIOSESSIONMANAGER2,
            CLSCTX_ALL,
            ptr::null_mut(),
            &mut session_manager,
        );

        if hr < 0 {
            return Err(anyhow!("Failed to get audio session manager: 0x{:08x}", hr));
        }

        Ok(session_manager)
    }

    pub unsafe fn get_endpoint_volume(device: *mut c_void) -> Result<*mut c_void> {
        let device_vtbl = *(device as *mut *mut IMMDeviceVtbl);
        let mut endpoint_volume: *mut c_void = ptr::null_mut();

        let hr = ((*device_vtbl).Activate)(
            device,
            &IID_IAUDIOENDPOINTVOLUME,
            CLSCTX_ALL,
            ptr::null_mut(),
            &mut endpoint_volume,
        );

        if hr < 0 {
            return Err(anyhow!("Failed to get endpoint volume: 0x{:08x}", hr));
        }

        Ok(endpoint_volume)
    }

    pub unsafe fn enumerate_audio_sessions_internal() -> Result<Vec<AudioSession>> {
        // COM must be initialized on the calling thread (tokio worker threads)
        ensure_com_initialized()?;

        let mut sessions = Vec::new();

        // Get the default audio device
        let device = match get_default_audio_device() {
            Ok(d) => d,
            Err(e) => {
                log::error!("Failed to get default audio device: {}", e);
                return Err(e);
            }
        };
        let device_vtbl = *(device as *mut *mut IMMDeviceVtbl);

        // Get master volume
        match get_endpoint_volume(device) {
            Ok(endpoint_volume) => {
                let endpoint_vtbl = *(endpoint_volume as *mut *mut IAudioEndpointVolumeVtbl);

                let mut volume_level: f32 = 0.0;
                let mut is_muted: BOOL = 0;

                let _ = ((*endpoint_vtbl).GetMasterVolumeLevelScalar)(
                    endpoint_volume,
                    &mut volume_level,
                );
                let _ = ((*endpoint_vtbl).GetMute)(endpoint_volume, &mut is_muted);

                sessions.push(AudioSession {
                    process_id: 0,
                    process_name: "Master".to_string(),
                    display_name: "Master Volume".to_string(),
                    volume: volume_level * 100.0,
                    is_muted: is_muted != 0,
                });

                ((*endpoint_vtbl).parent.Release)(endpoint_volume);
            }
            Err(e) => {
                log::warn!("Failed to get master volume: {}", e);
                // Add a default master volume entry
                sessions.push(AudioSession {
                    process_id: 0,
                    process_name: "Master".to_string(),
                    display_name: "Master Volume".to_string(),
                    volume: 50.0,
                    is_muted: false,
                });
            }
        }

        // Get the session manager
        let session_manager = match get_audio_session_manager(device) {
            Ok(mgr) => mgr,
            Err(e) => {
                log::error!("Failed to get session manager: {}", e);
                ((*device_vtbl).parent.Release)(device);
                return Ok(sessions); // Return with just master volume
            }
        };
        let session_mgr_vtbl = *(session_manager as *mut *mut IAudioSessionManager2Vtbl);

        // Get the session enumerator
        let mut session_enum: *mut IAudioSessionEnumerator = ptr::null_mut();
        let hr = ((*session_mgr_vtbl).GetSessionEnumerator)(session_manager, &mut session_enum);

        if hr < 0 {
            log::error!("Failed to get session enumerator: 0x{:08x}", hr);
            ((*session_mgr_vtbl).parent.parent.Release)(session_manager);
            ((*device_vtbl).parent.Release)(device);
            return Ok(sessions);
        }

        let session_enum_vtbl = *(session_enum as *mut *mut IAudioSessionEnumeratorVtbl);

        // Get the count of sessions
        let mut count: i32 = 0;
        let hr = ((*session_enum_vtbl).GetCount)(session_enum, &mut count);

        // Track which process names we've already seen to deduplicate.
        // Apps like Discord can create multiple audio sessions (voice, notifications, etc.)
        // but the user should see only one entry per application.
        let mut seen_process_names: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        if hr < 0 {
            log::error!("Failed to get session count: 0x{:08x}", hr);
        } else if count > 0 && count < 1000 {
            // Sanity check to prevent excessive iteration
            // Enumerate sessions
            for i in 0..count {
                let mut session_control: *mut IAudioSessionControl = ptr::null_mut();
                let hr = ((*session_enum_vtbl).GetSession)(session_enum, i, &mut session_control);

                if hr < 0 {
                    continue;
                }

                // Query for IAudioSessionControl2
                let session_ctrl_vtbl = *(session_control as *mut *mut IAudioSessionControlVtbl);
                let mut session_control2: *mut IAudioSessionControl2 = ptr::null_mut();

                // IID_IAudioSessionControl2 = {bfb7ff88-7239-4fc9-8fa2-07c950be9c6d}
                let iid_control2 = GUID {
                    data1: 0xbfb7ff88,
                    data2: 0x7239,
                    data3: 0x4fc9,
                    data4: [0x8f, 0xa2, 0x07, 0xc9, 0x50, 0xbe, 0x9c, 0x6d],
                };

                let hr = ((*session_ctrl_vtbl).parent.QueryInterface)(
                    session_control as *mut IUnknown,
                    &iid_control2,
                    &mut session_control2 as *mut _ as *mut *mut c_void,
                );

                if hr >= 0 && !session_control2.is_null() {
                    let session_ctrl2_vtbl =
                        *(session_control2 as *mut *mut IAudioSessionControl2Vtbl);

                    // Get process ID
                    let mut process_id: u32 = 0;
                    let hr =
                        ((*session_ctrl2_vtbl).GetProcessId)(session_control2, &mut process_id);

                    if hr >= 0 && process_id != 0 {
                        // Get process name
                        let process_name = get_process_name_from_id(process_id)
                            .unwrap_or_else(|| format!("Unknown App (PID: {})", process_id));

                        // Skip duplicate sessions for the same application.
                        let name_lower = process_name.to_lowercase();
                        if seen_process_names.contains(&name_lower) {
                            ((*session_ctrl2_vtbl).parent.parent.Release)(
                                session_control2 as *mut IUnknown,
                            );
                            ((*session_ctrl_vtbl).parent.Release)(session_control as *mut IUnknown);
                            continue;
                        }
                        seen_process_names.insert(name_lower);

                        // Get display name (often empty)
                        let mut display_name_ptr: PWSTR = ptr::null_mut();
                        let hr = ((*session_ctrl_vtbl).GetDisplayName)(
                            session_control,
                            &mut display_name_ptr,
                        );

                        let display_name = if hr >= 0 && !display_name_ptr.is_null() {
                            // SAFETY: Find null terminator safely
                            let mut len = 0;
                            while len < 256 && *display_name_ptr.offset(len as isize) != 0 {
                                len += 1;
                            }
                            let slice = std::slice::from_raw_parts(display_name_ptr, len);
                            String::from_utf16_lossy(slice)
                        } else {
                            // Use process name as display name
                            process_name.clone()
                        };

                        // Get volume through ISimpleAudioVolume
                        let mut simple_volume: *mut ISimpleAudioVolume = ptr::null_mut();
                        let iid_simple_volume = GUID {
                            data1: 0x87CE5498,
                            data2: 0x68D6,
                            data3: 0x44E5,
                            data4: [0x92, 0x15, 0x6D, 0xA4, 0x7E, 0xF8, 0x83, 0xD8],
                        };

                        let hr = ((*session_ctrl_vtbl).parent.QueryInterface)(
                            session_control as *mut IUnknown,
                            &iid_simple_volume,
                            &mut simple_volume as *mut _ as *mut *mut c_void,
                        );

                        let (volume, is_muted) = if hr >= 0 && !simple_volume.is_null() {
                            let simple_vol_vtbl =
                                *(simple_volume as *mut *mut ISimpleAudioVolumeVtbl);

                            let mut vol: f32 = 0.0;
                            let mut muted: BOOL = 0;

                            let _ = ((*simple_vol_vtbl).GetMasterVolume)(simple_volume, &mut vol);
                            let _ = ((*simple_vol_vtbl).GetMute)(simple_volume, &mut muted);

                            ((*simple_vol_vtbl).parent.Release)(simple_volume as *mut IUnknown);

                            (vol * 100.0, muted != 0)
                        } else {
                            (50.0, false)
                        };

                        sessions.push(AudioSession {
                            process_id,
                            process_name,
                            display_name,
                            volume,
                            is_muted,
                        });
                    }

                    ((*session_ctrl2_vtbl).parent.parent.Release)(
                        session_control2 as *mut IUnknown,
                    );
                }

                ((*session_ctrl_vtbl).parent.Release)(session_control as *mut IUnknown);
            }
        }

        // Cleanup
        ((*session_enum_vtbl).parent.Release)(session_enum as *mut IUnknown);
        ((*session_mgr_vtbl).parent.parent.Release)(session_manager);
        ((*device_vtbl).parent.Release)(device);

        Ok(sessions)
    }

    pub unsafe fn set_app_volume_internal(process_id: u32, volume: f32) -> Result<()> {
        // COM must be initialized on the calling thread (tokio worker threads)
        ensure_com_initialized()?;

        // Get the default audio device
        let device = get_default_audio_device()?;
        let device_vtbl = *(device as *mut *mut IMMDeviceVtbl);

        // Special case: Master volume (process_id == 0)
        if process_id == 0 {
            let endpoint_volume = get_endpoint_volume(device)?;
            let endpoint_vtbl = *(endpoint_volume as *mut *mut IAudioEndpointVolumeVtbl);

            let volume_scalar = (volume / 100.0).clamp(0.0, 1.0);
            let hr = ((*endpoint_vtbl).SetMasterVolumeLevelScalar)(
                endpoint_volume,
                volume_scalar,
                ptr::null_mut(),
            );

            ((*endpoint_vtbl).parent.Release)(endpoint_volume);
            ((*device_vtbl).parent.Release)(device);

            if hr < 0 {
                return Err(anyhow!("Failed to set master volume: 0x{:08x}", hr));
            }
            return Ok(());
        }

        // Get the session manager
        let session_manager = get_audio_session_manager(device)?;
        let session_mgr_vtbl = *(session_manager as *mut *mut IAudioSessionManager2Vtbl);

        // Get the session enumerator
        let mut session_enum: *mut IAudioSessionEnumerator = ptr::null_mut();
        let hr = ((*session_mgr_vtbl).GetSessionEnumerator)(session_manager, &mut session_enum);

        if hr < 0 {
            ((*session_mgr_vtbl).parent.parent.Release)(session_manager);
            ((*device_vtbl).parent.Release)(device);
            return Err(anyhow!("Failed to get session enumerator: 0x{:08x}", hr));
        }

        let session_enum_vtbl = *(session_enum as *mut *mut IAudioSessionEnumeratorVtbl);

        // Get the count of sessions
        let mut count: i32 = 0;
        let _ = ((*session_enum_vtbl).GetCount)(session_enum, &mut count);

        let mut found = false;

        // Find the session with matching process ID
        for i in 0..count {
            let mut session_control: *mut IAudioSessionControl = ptr::null_mut();
            let hr = ((*session_enum_vtbl).GetSession)(session_enum, i, &mut session_control);

            if hr < 0 {
                continue;
            }

            // Query for IAudioSessionControl2
            let session_ctrl_vtbl = *(session_control as *mut *mut IAudioSessionControlVtbl);
            let mut session_control2: *mut IAudioSessionControl2 = ptr::null_mut();

            // IID_IAudioSessionControl2 = {bfb7ff88-7239-4fc9-8fa2-07c950be9c6d}
            let iid_control2 = GUID {
                data1: 0xbfb7ff88,
                data2: 0x7239,
                data3: 0x4fc9,
                data4: [0x8f, 0xa2, 0x07, 0xc9, 0x50, 0xbe, 0x9c, 0x6d],
            };

            let hr = ((*session_ctrl_vtbl).parent.QueryInterface)(
                session_control as *mut IUnknown,
                &iid_control2,
                &mut session_control2 as *mut _ as *mut *mut c_void,
            );

            if hr >= 0 && !session_control2.is_null() {
                let session_ctrl2_vtbl = *(session_control2 as *mut *mut IAudioSessionControl2Vtbl);

                // Get process ID
                let mut pid: u32 = 0;
                let hr = ((*session_ctrl2_vtbl).GetProcessId)(session_control2, &mut pid);

                if hr >= 0 && pid == process_id {
                    // Found a matching session - set volume through ISimpleAudioVolume.
                    // Don't break — continue to set volume on ALL sessions for this PID
                    // (apps like Discord can have multiple audio sessions).
                    let mut simple_volume: *mut ISimpleAudioVolume = ptr::null_mut();
                    let iid_simple_volume = GUID {
                        data1: 0x87CE5498,
                        data2: 0x68D6,
                        data3: 0x44E5,
                        data4: [0x92, 0x15, 0x6D, 0xA4, 0x7E, 0xF8, 0x83, 0xD8],
                    };

                    let hr = ((*session_ctrl_vtbl).parent.QueryInterface)(
                        session_control as *mut IUnknown,
                        &iid_simple_volume,
                        &mut simple_volume as *mut _ as *mut *mut c_void,
                    );

                    if hr >= 0 && !simple_volume.is_null() {
                        let simple_vol_vtbl = *(simple_volume as *mut *mut ISimpleAudioVolumeVtbl);

                        let volume_scalar = (volume / 100.0).clamp(0.0, 1.0);
                        let hr = ((*simple_vol_vtbl).SetMasterVolume)(
                            simple_volume,
                            volume_scalar,
                            ptr::null_mut(),
                        );

                        ((*simple_vol_vtbl).parent.Release)(simple_volume as *mut IUnknown);

                        if hr >= 0 {
                            found = true;
                        }
                    }
                }

                ((*session_ctrl2_vtbl).parent.parent.Release)(session_control2 as *mut IUnknown);
            }

            ((*session_ctrl_vtbl).parent.Release)(session_control as *mut IUnknown);
        }

        // Cleanup
        ((*session_enum_vtbl).parent.Release)(session_enum as *mut IUnknown);
        ((*session_mgr_vtbl).parent.parent.Release)(session_manager);
        ((*device_vtbl).parent.Release)(device);

        if found {
            Ok(())
        } else {
            Err(anyhow!(
                "No audio session found for process ID {}",
                process_id
            ))
        }
    }

    pub unsafe fn get_master_volume_internal() -> Result<f32> {
        // COM must be initialized on the calling thread (tokio worker threads)
        ensure_com_initialized()?;

        let device = get_default_audio_device()?;
        let device_vtbl = *(device as *mut *mut IMMDeviceVtbl);

        let endpoint_volume = get_endpoint_volume(device)?;
        let endpoint_vtbl = *(endpoint_volume as *mut *mut IAudioEndpointVolumeVtbl);

        let mut volume_level: f32 = 0.0;

        // SAFETY: endpoint_vtbl is valid (checked in get_endpoint_volume)
        // volume_level pointer is valid stack variable
        let hr = ((*endpoint_vtbl).GetMasterVolumeLevelScalar)(endpoint_volume, &mut volume_level);

        // Always release COM interfaces, even on error
        ((*endpoint_vtbl).parent.Release)(endpoint_volume);
        ((*device_vtbl).parent.Release)(device);

        if hr < 0 {
            return Err(anyhow!("Failed to get master volume: 0x{:08x}", hr));
        }

        // Clamp to valid percentage range
        Ok((volume_level * 100.0).clamp(0.0, 100.0))
    }

    /// Set volumes for multiple processes in a single COM enumeration pass.
    pub unsafe fn set_volumes_batch_internal(volumes: &[(u32, f32)]) -> Result<()> {
        ensure_com_initialized()?;

        let device = get_default_audio_device()?;
        let device_vtbl = *(device as *mut *mut IMMDeviceVtbl);

        // Handle master volume if present
        for &(pid, vol) in volumes {
            if pid == 0 {
                let endpoint_volume = get_endpoint_volume(device)?;
                let endpoint_vtbl = *(endpoint_volume as *mut *mut IAudioEndpointVolumeVtbl);
                let volume_scalar = (vol / 100.0).clamp(0.0, 1.0);
                let _ = ((*endpoint_vtbl).SetMasterVolumeLevelScalar)(
                    endpoint_volume,
                    volume_scalar,
                    ptr::null_mut(),
                );
                ((*endpoint_vtbl).parent.Release)(endpoint_volume);
                break;
            }
        }

        // Collect non-master volumes to set
        let app_volumes: Vec<(u32, f32)> = volumes
            .iter()
            .filter(|(pid, _)| *pid != 0)
            .copied()
            .collect();

        if app_volumes.is_empty() {
            ((*device_vtbl).parent.Release)(device);
            return Ok(());
        }

        // Single enumeration for all app volumes
        let session_manager = match get_audio_session_manager(device) {
            Ok(mgr) => mgr,
            Err(e) => {
                ((*device_vtbl).parent.Release)(device);
                return Err(e);
            }
        };
        let session_mgr_vtbl = *(session_manager as *mut *mut IAudioSessionManager2Vtbl);

        let mut session_enum: *mut IAudioSessionEnumerator = ptr::null_mut();
        let hr = ((*session_mgr_vtbl).GetSessionEnumerator)(session_manager, &mut session_enum);

        if hr < 0 {
            ((*session_mgr_vtbl).parent.parent.Release)(session_manager);
            ((*device_vtbl).parent.Release)(device);
            return Err(anyhow!("Failed to get session enumerator: 0x{:08x}", hr));
        }

        let session_enum_vtbl = *(session_enum as *mut *mut IAudioSessionEnumeratorVtbl);

        let mut count: i32 = 0;
        let _ = ((*session_enum_vtbl).GetCount)(session_enum, &mut count);

        // IID_IAudioSessionControl2 = {bfb7ff88-7239-4fc9-8fa2-07c950be9c6d}
        let iid_control2 = GUID {
            data1: 0xbfb7ff88,
            data2: 0x7239,
            data3: 0x4fc9,
            data4: [0x8f, 0xa2, 0x07, 0xc9, 0x50, 0xbe, 0x9c, 0x6d],
        };

        let iid_simple_volume = GUID {
            data1: 0x87CE5498,
            data2: 0x68D6,
            data3: 0x44E5,
            data4: [0x92, 0x15, 0x6D, 0xA4, 0x7E, 0xF8, 0x83, 0xD8],
        };

        // Build a set of PIDs we need to match, along with the volume to apply.
        // Also collect process names so we can match ALL sessions belonging to
        // the same application (an app like Discord may spawn multiple audio
        // sessions that share a process name but have different PIDs or session
        // instances).
        let pid_set: std::collections::HashMap<u32, f32> = app_volumes.iter().copied().collect();

        for i in 0..count {
            let mut session_control: *mut IAudioSessionControl = ptr::null_mut();
            let hr = ((*session_enum_vtbl).GetSession)(session_enum, i, &mut session_control);
            if hr < 0 {
                continue;
            }

            let session_ctrl_vtbl = *(session_control as *mut *mut IAudioSessionControlVtbl);
            let mut session_control2: *mut IAudioSessionControl2 = ptr::null_mut();

            let hr = ((*session_ctrl_vtbl).parent.QueryInterface)(
                session_control as *mut IUnknown,
                &iid_control2,
                &mut session_control2 as *mut _ as *mut *mut c_void,
            );

            if hr >= 0 && !session_control2.is_null() {
                let session_ctrl2_vtbl = *(session_control2 as *mut *mut IAudioSessionControl2Vtbl);

                let mut pid: u32 = 0;
                let hr = ((*session_ctrl2_vtbl).GetProcessId)(session_control2, &mut pid);

                if hr >= 0 {
                    // Match by exact PID first
                    let vol = pid_set.get(&pid).copied().or_else(|| {
                        // Also match by process name so that all sessions
                        // belonging to the same app get their volume set,
                        // even if the enumerated PID differs from the one
                        // we stored (e.g. multiple sessions for Discord).
                        let session_name = get_process_name_from_id(pid);
                        session_name.and_then(|name| {
                            pid_set.iter().find_map(|(&target_pid, &v)| {
                                let target_name = get_process_name_from_id(target_pid);
                                match target_name {
                                    Some(tn) if tn.eq_ignore_ascii_case(&name) => Some(v),
                                    _ => None,
                                }
                            })
                        })
                    });

                    if let Some(vol) = vol {
                        // Set volume via ISimpleAudioVolume
                        let mut simple_volume: *mut ISimpleAudioVolume = ptr::null_mut();
                        let hr = ((*session_ctrl_vtbl).parent.QueryInterface)(
                            session_control as *mut IUnknown,
                            &iid_simple_volume,
                            &mut simple_volume as *mut _ as *mut *mut c_void,
                        );

                        if hr >= 0 && !simple_volume.is_null() {
                            let simple_vol_vtbl =
                                *(simple_volume as *mut *mut ISimpleAudioVolumeVtbl);
                            let volume_scalar = (vol / 100.0).clamp(0.0, 1.0);
                            let _ = ((*simple_vol_vtbl).SetMasterVolume)(
                                simple_volume,
                                volume_scalar,
                                ptr::null_mut(),
                            );
                            ((*simple_vol_vtbl).parent.Release)(simple_volume as *mut IUnknown);
                        }
                    }
                }

                ((*session_ctrl2_vtbl).parent.parent.Release)(session_control2 as *mut IUnknown);
            }

            ((*session_ctrl_vtbl).parent.Release)(session_control as *mut IUnknown);
        }

        ((*session_enum_vtbl).parent.Release)(session_enum as *mut IUnknown);
        ((*session_mgr_vtbl).parent.parent.Release)(session_manager);
        ((*device_vtbl).parent.Release)(device);

        Ok(())
    }

    // COM Interface definitions (vtables)
    #[repr(C)]
    struct IUnknownVtbl {
        QueryInterface:
            unsafe extern "system" fn(*mut IUnknown, *const GUID, *mut *mut c_void) -> HRESULT,
        AddRef: unsafe extern "system" fn(*mut IUnknown) -> u32,
        Release: unsafe extern "system" fn(*mut IUnknown) -> u32,
    }

    #[repr(C)]
    struct IMMDeviceEnumeratorVtbl {
        parent: IUnknownVtbl,
        EnumAudioEndpoints: *const c_void,
        GetDefaultAudioEndpoint:
            unsafe extern "system" fn(*mut c_void, EDataFlow, ERole, *mut *mut c_void) -> HRESULT,
        GetDevice: *const c_void,
        RegisterEndpointNotificationCallback: *const c_void,
        UnregisterEndpointNotificationCallback: *const c_void,
    }

    #[repr(C)]
    struct IMMDeviceVtbl {
        parent: IUnknownVtbl,
        Activate: unsafe extern "system" fn(
            *mut c_void,
            *const GUID,
            u32,
            *const c_void,
            *mut *mut c_void,
        ) -> HRESULT,
        OpenPropertyStore: *const c_void,
        GetId: *const c_void,
        GetState: *const c_void,
    }

    #[repr(C)]
    struct IAudioSessionManagerVtbl {
        parent: IUnknownVtbl,
        GetAudioSessionControl: *const c_void,
        GetSimpleAudioVolume: *const c_void,
    }

    #[repr(C)]
    struct IAudioSessionManager2Vtbl {
        parent: IAudioSessionManagerVtbl,
        GetSessionEnumerator:
            unsafe extern "system" fn(*mut c_void, *mut *mut IAudioSessionEnumerator) -> HRESULT,
        RegisterSessionNotification: *const c_void,
        UnregisterSessionNotification: *const c_void,
        RegisterDuckNotification: *const c_void,
        UnregisterDuckNotification: *const c_void,
    }

    #[repr(C)]
    struct IAudioSessionEnumeratorVtbl {
        parent: IUnknownVtbl,
        GetCount: unsafe extern "system" fn(*mut IAudioSessionEnumerator, *mut i32) -> HRESULT,
        GetSession: unsafe extern "system" fn(
            *mut IAudioSessionEnumerator,
            i32,
            *mut *mut IAudioSessionControl,
        ) -> HRESULT,
    }

    #[repr(C)]
    struct IAudioSessionControlVtbl {
        parent: IUnknownVtbl,
        GetState: *const c_void,
        GetDisplayName: unsafe extern "system" fn(*mut IAudioSessionControl, *mut PWSTR) -> HRESULT,
        SetDisplayName: *const c_void,
        GetIconPath: *const c_void,
        SetIconPath: *const c_void,
        GetGroupingParam: *const c_void,
        SetGroupingParam: *const c_void,
        RegisterAudioSessionNotification: *const c_void,
        UnregisterAudioSessionNotification: *const c_void,
    }

    #[repr(C)]
    struct IAudioSessionControl2Vtbl {
        parent: IAudioSessionControlVtbl,
        GetSessionIdentifier: *const c_void,
        GetSessionInstanceIdentifier: *const c_void,
        GetProcessId: unsafe extern "system" fn(*mut IAudioSessionControl2, *mut u32) -> HRESULT,
        IsSystemSoundsSession: *const c_void,
        SetDuckingPreference: *const c_void,
    }

    #[repr(C)]
    struct ISimpleAudioVolumeVtbl {
        parent: IUnknownVtbl,
        SetMasterVolume:
            unsafe extern "system" fn(*mut ISimpleAudioVolume, f32, *const GUID) -> HRESULT,
        GetMasterVolume: unsafe extern "system" fn(*mut ISimpleAudioVolume, *mut f32) -> HRESULT,
        SetMute: unsafe extern "system" fn(*mut ISimpleAudioVolume, BOOL, *const GUID) -> HRESULT,
        GetMute: unsafe extern "system" fn(*mut ISimpleAudioVolume, *mut BOOL) -> HRESULT,
    }

    #[repr(C)]
    struct IAudioEndpointVolumeVtbl {
        parent: IUnknownVtbl,
        RegisterControlChangeNotify: *const c_void,
        UnregisterControlChangeNotify: *const c_void,
        GetChannelCount: *const c_void,
        SetMasterVolumeLevel: *const c_void,
        SetMasterVolumeLevelScalar:
            unsafe extern "system" fn(*mut c_void, f32, *const GUID) -> HRESULT,
        GetMasterVolumeLevel: *const c_void,
        GetMasterVolumeLevelScalar: unsafe extern "system" fn(*mut c_void, *mut f32) -> HRESULT,
        SetChannelVolumeLevel: *const c_void,
        SetChannelVolumeLevelScalar: *const c_void,
        GetChannelVolumeLevel: *const c_void,
        GetChannelVolumeLevelScalar: *const c_void,
        SetMute: unsafe extern "system" fn(*mut c_void, BOOL, *const GUID) -> HRESULT,
        GetMute: unsafe extern "system" fn(*mut c_void, *mut BOOL) -> HRESULT,
        GetVolumeStepInfo: *const c_void,
        VolumeStepUp: *const c_void,
        VolumeStepDown: *const c_void,
        QueryHardwareSupport: *const c_void,
        GetVolumeRange: *const c_void,
    }

    // Type aliases for cleaner code
    type IUnknown = c_void;
    type IAudioSessionEnumerator = c_void;
    type IAudioSessionControl = c_void;
    type IAudioSessionControl2 = c_void;
    type ISimpleAudioVolume = c_void;
}

pub struct WindowsAudioManager;

impl WindowsAudioManager {
    pub fn new() -> Self {
        log::info!("Initializing Windows Audio Manager");
        if let Err(e) = ensure_com_initialized() {
            log::error!("Failed to initialize COM for Windows Audio: {}", e);
            log::error!("This will prevent audio session enumeration from working");
        } else {
            log::info!("COM initialized successfully for Windows Audio");
        }
        Self
    }
}

impl AudioManager for WindowsAudioManager {
    fn get_audio_sessions(&self) -> Result<Vec<AudioSession>> {
        #[cfg(target_os = "windows")]
        {
            unsafe {
                match windows_audio::enumerate_audio_sessions_internal() {
                    Ok(sessions) if !sessions.is_empty() => Ok(sessions),
                    Ok(_) => {
                        // No sessions found, return at least Master Volume
                        Ok(vec![AudioSession {
                            process_id: 0,
                            process_name: "Master".to_string(),
                            display_name: "Master Volume".to_string(),
                            volume: 75.0,
                            is_muted: false,
                        }])
                    }
                    Err(e) => {
                        log::error!("Failed to enumerate audio sessions: {}", e);
                        Err(anyhow!("Windows audio enumeration failed: {}", e))
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Non-Windows platform - return mock data
            Ok(vec![AudioSession {
                process_id: 0,
                process_name: "Master".to_string(),
                display_name: "Master Volume".to_string(),
                volume: 75.0,
                is_muted: false,
            }])
        }
    }

    fn set_app_volume(&self, process_id: u32, volume: f32) -> Result<()> {
        // Validate input
        if !volume.is_finite() || volume < 0.0 || volume > 100.0 {
            return Err(anyhow!("Volume must be between 0 and 100, got {}", volume));
        }

        #[cfg(target_os = "windows")]
        {
            unsafe {
                windows_audio::set_app_volume_internal(process_id, volume)
                    .map_err(|e| anyhow!("Failed to set volume for process {}: {}", process_id, e))
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            log::info!("Would set volume for process {} to {}%", process_id, volume);
            Ok(())
        }
    }

    fn set_master_volume(&self, volume: f32) -> Result<()> {
        self.set_app_volume(0, volume)
    }

    fn get_master_volume(&self) -> Result<f32> {
        #[cfg(target_os = "windows")]
        {
            unsafe {
                windows_audio::get_master_volume_internal()
                    .map_err(|e| anyhow!("Failed to get master volume: {}", e))
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            Ok(50.0)
        }
    }

    fn set_volumes_batch(&self, volumes: &[(u32, f32)]) -> Result<()> {
        if volumes.is_empty() {
            return Ok(());
        }

        #[cfg(target_os = "windows")]
        {
            unsafe {
                windows_audio::set_volumes_batch_internal(volumes)
                    .map_err(|e| anyhow!("Failed to set volumes batch: {}", e))
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            for (pid, vol) in volumes {
                log::info!("Would set volume for process {} to {}%", pid, vol);
            }
            Ok(())
        }
    }
}

impl Default for WindowsAudioManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WindowsAudioManager {
    fn drop(&mut self) {
        // COM cleanup is handled automatically by Windows when the process exits
        // Individual interface releases are done within each function
        log::debug!("WindowsAudioManager dropped");
    }
}
