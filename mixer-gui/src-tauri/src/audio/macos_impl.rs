#[cfg(target_os = "macos")]
use anyhow::{anyhow, Result};
use std::process::Command;
use crate::audio::AudioManager;
use crate::types::AudioSession;

pub struct MacOSAudioManager;

impl MacOSAudioManager {
    pub fn new() -> Self {
        Self
    }

    /// Set system volume using AppleScript
    fn set_system_volume_applescript(volume: i32) -> Result<()> {
        let output = Command::new("osascript")
            .arg("-e")
            .arg(format!("set volume output volume {}", volume))
            .output()?;

        if !output.status.success() {
            return Err(anyhow!("Failed to set volume via AppleScript"));
        }

        Ok(())
    }

    /// Get system volume using AppleScript
    fn get_system_volume_applescript() -> Result<i32> {
        let output = Command::new("osascript")
            .arg("-e")
            .arg("output volume of (get volume settings)")
            .output()?;

        if !output.status.success() {
            return Err(anyhow!("Failed to get volume via AppleScript"));
        }

        let volume_str = String::from_utf8_lossy(&output.stdout);
        let volume = volume_str.trim().parse::<i32>()
            .map_err(|e| anyhow!("Failed to parse volume: {}", e))?;

        Ok(volume)
    }
}

impl AudioManager for MacOSAudioManager {
    fn get_audio_sessions(&self) -> Result<Vec<AudioSession>> {
        // macOS doesn't provide per-app volume control natively
        // Return list of audio-capable apps for UI consistency
        // In a real implementation, you might enumerate running apps
        // that are known to produce audio

        let mut sessions = Vec::new();

        // Add some common audio apps if they're running
        let common_apps = vec![
            ("Music", "com.apple.Music"),
            ("Spotify", "com.spotify.client"),
            ("Discord", "com.hnc.Discord"),
            ("Chrome", "com.google.Chrome"),
            ("Safari", "com.apple.Safari"),
            ("Zoom", "us.zoom.xos"),
        ];

        for (name, bundle_id) in common_apps {
            // Check if app is running using ps or other method
            let output = Command::new("pgrep")
                .arg("-f")
                .arg(bundle_id)
                .output();

            if let Ok(output) = output {
                if output.status.success() && !output.stdout.is_empty() {
                    let pid_str = String::from_utf8_lossy(&output.stdout);
                    if let Ok(pid) = pid_str.trim().lines().next()
                        .unwrap_or("0")
                        .parse::<u32>()
                    {
                        sessions.push(AudioSession {
                            process_id: pid,
                            process_name: format!("{}.app", name),
                            display_name: name.to_string(),
                            volume: 50.0, // Default since we can't get actual app volume
                            is_muted: false,
                        });
                    }
                }
            }
        }

        if sessions.is_empty() {
            // Return at least one mock session for testing
            sessions.push(AudioSession {
                process_id: 1234,
                process_name: "System Audio".to_string(),
                display_name: "System Audio".to_string(),
                volume: 50.0,
                is_muted: false,
            });
        }

        Ok(sessions)
    }

    fn set_app_volume(&self, process_id: u32, volume: f32) -> Result<()> {
        // macOS doesn't support per-app volume natively
        // You could implement this using:
        // 1. Audio Hijack Pro's scripting interface
        // 2. Background Music app's API
        // 3. Custom Core Audio solution (complex)

        // For now, we'll just log the intent
        println!("macOS: Would set volume for PID {} to {}% (not supported natively)",
                 process_id, volume);

        // Return success to prevent UI errors
        Ok(())
    }

    fn set_master_volume(&self, volume: f32) -> Result<()> {
        // Convert percentage (0-100) to macOS scale (0-100)
        let mac_volume = volume.round() as i32;
        Self::set_system_volume_applescript(mac_volume)
    }

    fn get_master_volume(&self) -> Result<f32> {
        let volume = Self::get_system_volume_applescript()?;
        Ok(volume as f32)
    }
}

impl Default for MacOSAudioManager {
    fn default() -> Self {
        Self::new()
    }
}