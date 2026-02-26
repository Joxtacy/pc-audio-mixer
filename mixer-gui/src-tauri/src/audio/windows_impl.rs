use anyhow::{anyhow, Result};
use std::sync::Once;
#[cfg(target_os = "windows")]
use windows::{
    core::*,
    Win32::{
        Media::Audio::{
            eConsole, eRender,
            Endpoints::{
                IAudioEndpointVolume, IAudioSessionControl2, IAudioSessionEnumerator,
                IAudioSessionManager2,
            },
            IMMDevice, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
        },
        System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
    },
};

use crate::audio::AudioManager;
use crate::types::AudioSession;

static INIT_COM: Once = Once::new();

fn ensure_com_initialized() {
    INIT_COM.call_once(|| unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    });
}

pub struct WindowsAudioManager;

impl WindowsAudioManager {
    pub fn new() -> Self {
        ensure_com_initialized();
        Self
    }

    fn get_default_device() -> Result<IMMDevice> {
        unsafe {
            let device_enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            let device = device_enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            Ok(device)
        }
    }

    fn get_session_manager() -> Result<IAudioSessionManager2> {
        unsafe {
            let device = Self::get_default_device()?;
            let session_manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
            Ok(session_manager)
        }
    }
}

impl AudioManager for WindowsAudioManager {
    fn get_audio_sessions(&self) -> Result<Vec<AudioSession>> {
        unsafe {
            let session_manager = Self::get_session_manager()?;
            let enumerator: IAudioSessionEnumerator = session_manager.GetSessionEnumerator()?;

            let count = enumerator.GetCount()?;
            let mut sessions = Vec::new();

            for i in 0..count {
                if let Ok(session_control) = enumerator.GetSession(i) {
                    let session_control2: IAudioSessionControl2 = session_control.cast()?;

                    // Get process ID
                    let process_id = session_control2.GetProcessId()?;
                    if process_id == 0 {
                        continue; // Skip system sounds
                    }

                    // Get display name
                    let display_name = match session_control2.GetDisplayName() {
                        Ok(name) => name.to_string()?,
                        Err(_) => format!("Process {}", process_id),
                    };

                    // Get simple volume interface
                    let simple_volume = session_control.cast::<ISimpleAudioVolume>()?;
                    let volume = simple_volume.GetMasterVolume()?;
                    let is_muted = simple_volume.GetMute()?.as_bool();

                    // Try to get process name from process ID
                    let process_name = get_process_name(process_id)
                        .unwrap_or_else(|| format!("PID: {}", process_id));

                    sessions.push(AudioSession {
                        process_id,
                        process_name,
                        display_name,
                        volume: volume * 100.0,
                        is_muted,
                    });
                }
            }

            Ok(sessions)
        }
    }

    fn set_app_volume(&self, process_id: u32, volume: f32) -> Result<()> {
        unsafe {
            let session_manager = Self::get_session_manager()?;
            let enumerator: IAudioSessionEnumerator = session_manager.GetSessionEnumerator()?;

            let count = enumerator.GetCount()?;

            for i in 0..count {
                if let Ok(session_control) = enumerator.GetSession(i) {
                    let session_control2: IAudioSessionControl2 = session_control.cast()?;

                    if session_control2.GetProcessId()? == process_id {
                        let simple_volume = session_control.cast::<ISimpleAudioVolume>()?;
                        simple_volume.SetMasterVolume(volume / 100.0, std::ptr::null())?;
                        return Ok(());
                    }
                }
            }

            Err(anyhow!(
                "Audio session not found for process {}",
                process_id
            ))
        }
    }

    fn set_master_volume(&self, volume: f32) -> Result<()> {
        unsafe {
            let device = Self::get_default_device()?;
            let endpoint_volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
            endpoint_volume.SetMasterVolumeLevelScalar(volume / 100.0, std::ptr::null())?;
            Ok(())
        }
    }

    fn get_master_volume(&self) -> Result<f32> {
        unsafe {
            let device = Self::get_default_device()?;
            let endpoint_volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
            let volume = endpoint_volume.GetMasterVolumeLevelScalar()?;
            Ok(volume * 100.0)
        }
    }
}

#[cfg(target_os = "windows")]
fn get_process_name(process_id: u32) -> Option<String> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::ProcessStatus::K32GetProcessImageFileNameW;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    unsafe {
        let process_handle =
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()?;
        if process_handle.is_invalid() {
            return None;
        }

        let mut buffer = vec![0u16; 1024];
        let len = K32GetProcessImageFileNameW(process_handle, &mut buffer);
        CloseHandle(process_handle);

        if len == 0 {
            return None;
        }

        let path = String::from_utf16_lossy(&buffer[..len as usize]);

        // Extract just the executable name from the full path
        path.split('\\').last().map(|s| s.to_string())
    }
}

impl Default for WindowsAudioManager {
    fn default() -> Self {
        Self::new()
    }
}
