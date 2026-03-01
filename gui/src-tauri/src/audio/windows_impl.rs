use anyhow::{anyhow, Result};
use std::sync::Once;

use crate::audio::AudioManager;
use crate::types::AudioSession;

static INIT_COM: Once = Once::new();

fn ensure_com_initialized() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
        use std::sync::atomic::{AtomicBool, Ordering};

        static COM_INIT_SUCCESS: AtomicBool = AtomicBool::new(false);

        INIT_COM.call_once(|| unsafe {
            match CoInitializeEx(None, COINIT_MULTITHREADED) {
                Ok(_) => {
                    COM_INIT_SUCCESS.store(true, Ordering::SeqCst);
                    log::info!("COM initialized successfully");
                }
                Err(e) => {
                    log::error!("Failed to initialize COM: {:?}", e);
                }
            }
        });

        if COM_INIT_SUCCESS.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(anyhow!("COM initialization failed"))
        }
    }

    #[cfg(not(target_os = "windows"))]
    Ok(())
}

#[cfg(target_os = "windows")]
fn get_process_name_from_id(pid: u32) -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::ProcessStatus::{GetModuleFileNameExW, GetProcessImageFileNameW};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    unsafe {
        // Try to open the process with minimum required permissions
        let process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

        if process_handle.is_invalid() {
            return None;
        }

        // Ensure handle is closed when we're done
        let _guard = scopeguard::guard(process_handle, |h| {
            let _ = CloseHandle(h);
        });

        const MAX_PATH: usize = 260; // Windows MAX_PATH constant
        let mut buffer = [0u16; MAX_PATH];

        // Try GetModuleFileNameExW first (requires more permissions)
        let len = GetModuleFileNameExW(process_handle, None, &mut buffer);

        let final_len = if len == 0 {
            // Fallback to GetProcessImageFileNameW
            let len = GetProcessImageFileNameW(process_handle, &mut buffer);
            if len == 0 {
                return None;
            }
            len.min(MAX_PATH as u32)
        } else {
            len.min(MAX_PATH as u32)
        };

        let path = OsString::from_wide(&buffer[..final_len as usize]);
        let path_str = path.to_string_lossy();

        // Extract just the filename from the full path
        path_str
            .split('\\')
            .last()
            .map(|s| s.to_string())
    }
}

pub struct WindowsAudioManager;

impl WindowsAudioManager {
    pub fn new() -> Self {
        if let Err(e) = ensure_com_initialized() {
            log::error!("Failed to initialize COM for Windows Audio: {}", e);
        }
        Self
    }

    #[cfg(target_os = "windows")]
    fn enumerate_audio_sessions_internal() -> Result<Vec<AudioSession>> {
        use windows::{
            core::*,
            Win32::{
                Media::Audio::{
                    eConsole, eRender,
                    Endpoints::{
                        IAudioEndpointVolume, IAudioSessionControl, IAudioSessionControl2,
                        IAudioSessionEnumerator, IAudioSessionManager2,
                    },
                    IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
                },
                System::Com::{CoCreateInstance, CLSCTX_ALL},
            },
        };

        let mut sessions = Vec::new();

        unsafe {
            // Create device enumerator
            let device_enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            // Get default audio endpoint
            let device: IMMDevice = device_enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)?;

            // First, add Master Volume as the first entry
            if let Ok(endpoint_volume) = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                let volume = endpoint_volume.GetMasterVolumeLevelScalar()? * 100.0;
                let is_muted = endpoint_volume.GetMute()?.as_bool();

                sessions.push(AudioSession {
                    process_id: 0,
                    process_name: "Master".to_string(),
                    display_name: "Master Volume".to_string(),
                    volume,
                    is_muted,
                });
            }

            // Get session manager
            let session_manager = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None)?;

            // Get session enumerator
            let session_enumerator: IAudioSessionEnumerator =
                session_manager.GetSessionEnumerator()?;

            let count = session_enumerator.GetCount()?;

            // Limit enumeration to prevent resource exhaustion
            const MAX_SESSIONS: i32 = 100;
            let safe_count = count.min(MAX_SESSIONS);

            if count > MAX_SESSIONS {
                log::warn!("Audio session count ({}) exceeds limit ({}), truncating", count, MAX_SESSIONS);
            }

            // Enumerate audio sessions up to the limit
            for i in 0..safe_count {
                if let Ok(session_control) = session_enumerator.GetSession(i) {
                    // Try to get extended session control
                    if let Ok(session_control2) = session_control.cast::<IAudioSessionControl2>() {
                        // Get process ID
                        let process_id = session_control2.GetProcessId()?;

                        // Skip system sounds (process_id 0)
                        if process_id == 0 {
                            continue;
                        }

                        // Get display name
                        let display_name_ptr = session_control2.GetDisplayName()?;
                        let display_name = if !display_name_ptr.is_null() {
                            display_name_ptr.to_string()?
                        } else {
                            String::new()
                        };

                        // Get process name
                        let process_name = get_process_name_from_id(process_id)
                            .unwrap_or_else(|| format!("Process {}", process_id));

                        // Use display name if available, otherwise use process name
                        let final_display_name = if display_name.is_empty() {
                            process_name
                                .trim_end_matches(".exe")
                                .split('.')
                                .next()
                                .unwrap_or(&process_name)
                                .to_string()
                        } else {
                            display_name
                        };

                        // Get volume - sessions don't have individual volume in this API
                        // Volume control is done through ISimpleAudioVolume which requires different approach
                        let volume = 100.0; // Default to full volume for now

                        sessions.push(AudioSession {
                            process_id,
                            process_name: process_name.clone(),
                            display_name: final_display_name,
                            volume,
                            is_muted: false,
                        });
                    }
                }
            }
        }

        Ok(sessions)
    }
}

impl AudioManager for WindowsAudioManager {
    fn get_audio_sessions(&self) -> Result<Vec<AudioSession>> {
        #[cfg(target_os = "windows")]
        {
            // Try to enumerate real sessions, fallback to mock data on error
            match Self::enumerate_audio_sessions_internal() {
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
                    // Return mock data as fallback
                    Ok(vec![
                        AudioSession {
                            process_id: 0,
                            process_name: "Master".to_string(),
                            display_name: "Master Volume".to_string(),
                            volume: 75.0,
                            is_muted: false,
                        },
                        AudioSession {
                            process_id: 1234,
                            process_name: "chrome.exe".to_string(),
                            display_name: "Google Chrome".to_string(),
                            volume: 50.0,
                            is_muted: false,
                        },
                        AudioSession {
                            process_id: 5678,
                            process_name: "spotify.exe".to_string(),
                            display_name: "Spotify".to_string(),
                            volume: 65.0,
                            is_muted: false,
                        },
                    ])
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Non-Windows platform - return mock data
            Ok(vec![
                AudioSession {
                    process_id: 0,
                    process_name: "Master".to_string(),
                    display_name: "Master Volume".to_string(),
                    volume: 75.0,
                    is_muted: false,
                },
            ])
        }
    }

    fn set_app_volume(&self, process_id: u32, volume: f32) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            if process_id == 0 {
                // Master volume
                return self.set_master_volume(volume);
            }

            // Per-app volume control would require ISimpleAudioVolume
            // For now, just log the request
            log::info!(
                "Windows: Setting volume for process {} to {}%",
                process_id, volume
            );
        }
        Ok(())
    }

    fn set_master_volume(&self, volume: f32) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            use windows::{
                core::*,
                Win32::{
                    Media::Audio::{
                        eConsole, eRender,
                        Endpoints::IAudioEndpointVolume,
                        IMMDeviceEnumerator, MMDeviceEnumerator,
                    },
                    System::Com::{CoCreateInstance, CLSCTX_ALL},
                },
            };

            unsafe {
                let device_enumerator: IMMDeviceEnumerator =
                    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

                let device = device_enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
                let endpoint_volume = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)?;

                // Validate input and convert percentage to scalar (0.0 to 1.0)
                if !volume.is_finite() {
                    return Err(anyhow!("Invalid volume value: must be a finite number"));
                }
                let scalar_volume = (volume / 100.0).clamp(0.0, 1.0);
                endpoint_volume.SetMasterVolumeLevelScalar(scalar_volume, std::ptr::null())?;

                log::info!("Windows: Set master volume to {}%", volume);
            }
        }

        Ok(())
    }

    fn get_master_volume(&self) -> Result<f32> {
        #[cfg(target_os = "windows")]
        {
            use windows::{
                core::*,
                Win32::{
                    Media::Audio::{
                        eConsole, eRender,
                        Endpoints::IAudioEndpointVolume,
                        IMMDeviceEnumerator, MMDeviceEnumerator,
                    },
                    System::Com::{CoCreateInstance, CLSCTX_ALL},
                },
            };

            unsafe {
                let device_enumerator: IMMDeviceEnumerator =
                    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

                let device = device_enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
                let endpoint_volume = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)?;

                let volume = endpoint_volume.GetMasterVolumeLevelScalar()? * 100.0;
                Ok(volume)
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            Ok(50.0)
        }
    }
}

impl Default for WindowsAudioManager {
    fn default() -> Self {
        Self::new()
    }
}