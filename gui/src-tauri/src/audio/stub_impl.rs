use crate::audio::AudioManager;
use crate::types::AudioSession;
use anyhow::Result;

pub struct StubAudioManager;

impl StubAudioManager {
    pub fn new() -> Self {
        Self
    }
}

impl AudioManager for StubAudioManager {
    fn get_audio_sessions(&self) -> Result<Vec<AudioSession>> {
        // Return mock data for testing on non-Windows platforms
        Ok(vec![
            // Master Volume as special entry with process_id: 0
            AudioSession {
                process_id: 0,
                process_name: "Master".to_string(),
                display_name: "Master Volume".to_string(),
                volume: 75.0,
                is_muted: false,
            },
            // Common applications - using macOS/Linux process names
            AudioSession {
                process_id: 1234,
                process_name: "Google Chrome".to_string(),
                display_name: "Google Chrome".to_string(),
                volume: 50.0,
                is_muted: false,
            },
            AudioSession {
                process_id: 5678,
                process_name: "Spotify".to_string(),
                display_name: "Spotify".to_string(),
                volume: 65.0,
                is_muted: false,
            },
            AudioSession {
                process_id: 9012,
                process_name: "Discord".to_string(),
                display_name: "Discord".to_string(),
                volume: 80.0,
                is_muted: false,
            },
            AudioSession {
                process_id: 3456,
                process_name: "Firefox".to_string(),
                display_name: "Mozilla Firefox".to_string(),
                volume: 45.0,
                is_muted: false,
            },
            AudioSession {
                process_id: 7890,
                process_name: "VLC".to_string(),
                display_name: "VLC Media Player".to_string(),
                volume: 90.0,
                is_muted: false,
            },
        ])
    }

    fn set_app_volume(&self, process_id: u32, volume: f32) -> Result<()> {
        println!(
            "Stub: Setting volume for process {} to {}%",
            process_id, volume
        );
        Ok(())
    }

    fn set_master_volume(&self, volume: f32) -> Result<()> {
        println!("Stub: Setting master volume to {}%", volume);
        Ok(())
    }

    fn get_master_volume(&self) -> Result<f32> {
        Ok(50.0)
    }
}

impl Default for StubAudioManager {
    fn default() -> Self {
        Self::new()
    }
}
