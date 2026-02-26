use crate::types::AudioSession;
use anyhow::Result;

pub trait AudioManager: Send + Sync {
    fn get_audio_sessions(&self) -> Result<Vec<AudioSession>>;
    fn set_app_volume(&self, process_id: u32, volume: f32) -> Result<()>;
    fn set_master_volume(&self, volume: f32) -> Result<()>;
    fn get_master_volume(&self) -> Result<f32>;
}

#[cfg(target_os = "windows")]
pub mod windows_impl;

#[cfg(target_os = "macos")]
pub mod macos_impl;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub mod stub_impl;

// Platform-specific type aliases
#[cfg(target_os = "windows")]
pub type PlatformAudioManager = windows_impl::WindowsAudioManager;

#[cfg(target_os = "macos")]
pub type PlatformAudioManager = macos_impl::MacOSAudioManager;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub type PlatformAudioManager = stub_impl::StubAudioManager;

// Keep backward compatibility
pub use PlatformAudioManager as WindowsAudioManager;
