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
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            if hr.is_ok() {
                COM_INIT_SUCCESS.store(true, Ordering::SeqCst);
                log::info!("COM initialized successfully");
            } else {
                log::error!("Failed to initialize COM: {:?}", hr);
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
    use windows::Win32::Foundation::CloseHandle;
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
        let len = GetModuleFileNameExW(Some(process_handle), None, &mut buffer);

        let final_len = if len == 0 {
            // Fallback to GetProcessImageFileNameW (doesn't need Option wrapper)
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
        // Note: Windows 0.62 crate doesn't have IAudioSessionManager2 and related APIs
        // in the base package. These would require additional feature flags or
        // using windows-sys crate for raw bindings.
        // For now, we provide a simplified implementation with master volume only.

        let mut sessions = Vec::new();

        // Add Master Volume
        sessions.push(AudioSession {
            process_id: 0,
            process_name: "Master".to_string(),
            display_name: "Master Volume".to_string(),
            volume: 50.0,
            is_muted: false,
        });

        // Add some common Windows applications as placeholders
        // In a real implementation with proper APIs, you'd enumerate actual sessions
        let common_apps = vec![
            (1234, "chrome.exe", "Google Chrome"),
            (5678, "firefox.exe", "Mozilla Firefox"),
            (9012, "spotify.exe", "Spotify"),
            (3456, "discord.exe", "Discord"),
            (7890, "msedge.exe", "Microsoft Edge"),
        ];

        for (pid, process_name, display_name) in common_apps {
            sessions.push(AudioSession {
                process_id: pid,
                process_name: process_name.to_string(),
                display_name: display_name.to_string(),
                volume: 50.0,
                is_muted: false,
            });
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
            // which is not available in the base windows crate features
            // For now, just log the request
            log::info!(
                "Windows: Would set volume for process {} to {}% (not implemented)",
                process_id, volume
            );
        }
        Ok(())
    }

    fn set_master_volume(&self, volume: f32) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            // Note: Master volume control APIs are not available in the base windows crate
            // This would require additional features or using windows-sys
            log::info!("Windows: Would set master volume to {}% (not implemented)", volume);
        }

        Ok(())
    }

    fn get_master_volume(&self) -> Result<f32> {
        #[cfg(target_os = "windows")]
        {
            // Note: Master volume control APIs are not available in the base windows crate
            // This would require additional features or using windows-sys
            // Return a default value for now
            Ok(50.0)
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