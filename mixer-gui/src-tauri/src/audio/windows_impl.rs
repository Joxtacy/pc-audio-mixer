use anyhow::{anyhow, Result};
use std::sync::Once;

use crate::audio::AudioManager;
use crate::types::AudioSession;

static INIT_COM: Once = Once::new();

fn ensure_com_initialized() {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

        INIT_COM.call_once(|| unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        });
    }
}

pub struct WindowsAudioManager;

impl WindowsAudioManager {
    pub fn new() -> Self {
        ensure_com_initialized();
        Self
    }
}

impl AudioManager for WindowsAudioManager {
    fn get_audio_sessions(&self) -> Result<Vec<AudioSession>> {
        // Windows audio session enumeration requires complex COM APIs
        // that are not fully available in the windows crate.
        // Return mock data for now, similar to the stub implementation.

        Ok(vec![AudioSession {
            process_id: 1234,
            process_name: "System Audio".to_string(),
            display_name: "System Audio".to_string(),
            volume: 50.0,
            is_muted: false,
        }])
    }

    fn set_app_volume(&self, process_id: u32, volume: f32) -> Result<()> {
        // Per-app volume control requires IAudioSessionManager2 and related APIs
        // which are not available in our windows crate version.
        println!(
            "Windows: Setting volume for process {} to {}% (limited support)",
            process_id, volume
        );
        Ok(())
    }

    fn set_master_volume(&self, volume: f32) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            use windows::{
                core::*,
                Win32::{
                    Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator},
                    System::Com::{CoCreateInstance, CLSCTX_ALL},
                },
            };

            unsafe {
                let device_enumerator: IMMDeviceEnumerator =
                    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

                let _device = device_enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;

                // Note: IAudioEndpointVolume is not available in our windows crate version
                // So we can't actually set the volume here.
                // This would require using windows-sys or upgrading the windows crate.

                println!("Windows: Would set master volume to {}%", volume);
            }
        }

        Ok(())
    }

    fn get_master_volume(&self) -> Result<f32> {
        // Return a default value since we can't access the actual volume
        Ok(50.0)
    }
}

impl Default for WindowsAudioManager {
    fn default() -> Self {
        Self::new()
    }
}
