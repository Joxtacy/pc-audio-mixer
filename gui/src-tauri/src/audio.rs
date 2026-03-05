use crate::types::AudioSession;
use anyhow::Result;

pub trait AudioManager: Send + Sync {
    fn get_audio_sessions(&self) -> Result<Vec<AudioSession>>;
    fn set_app_volume(&self, process_id: u32, volume: f32) -> Result<()>;
    fn set_master_volume(&self, volume: f32) -> Result<()>;
    fn get_master_volume(&self) -> Result<f32>;
    /// Set volumes for multiple processes in a single COM enumeration pass.
    /// Each tuple is (process_id, volume). process_id 0 means master volume.
    fn set_volumes_batch(&self, volumes: &[(u32, f32)]) -> Result<()>;
}

#[cfg(target_os = "windows")]
pub mod windows_impl;

#[cfg(not(target_os = "windows"))]
pub mod stub_impl;

// Platform-specific type aliases
#[cfg(target_os = "windows")]
pub type PlatformAudioManager = windows_impl::WindowsAudioManager;

#[cfg(not(target_os = "windows"))]
pub type PlatformAudioManager = stub_impl::StubAudioManager;

// Keep backward compatibility
pub use PlatformAudioManager as WindowsAudioManager;
