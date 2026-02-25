use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::types::{AppConfig, ChannelMapping};

const CONFIG_FILE_NAME: &str = "config.json";

fn get_config_path(app_handle: &AppHandle) -> Result<PathBuf> {
    let config_dir = app_handle.path().app_config_dir()?;

    // Ensure directory exists
    fs::create_dir_all(&config_dir)?;

    Ok(config_dir.join(CONFIG_FILE_NAME))
}

pub fn load_config(app_handle: &AppHandle) -> Result<AppConfig> {
    let config_path = get_config_path(app_handle)?;

    if !config_path.exists() {
        // Return default config if file doesn't exist
        return Ok(AppConfig {
            channel_mappings: Vec::new(),
            start_with_windows: false,
            minimize_to_tray: true,
            auto_connect: true,
            theme: "dark".to_string(),
        });
    }

    let config_str = fs::read_to_string(config_path)?;
    let config: AppConfig = serde_json::from_str(&config_str)?;

    Ok(config)
}

pub fn save_config(app_handle: &AppHandle, config: &AppConfig) -> Result<()> {
    let config_path = get_config_path(app_handle)?;
    let config_str = serde_json::to_string_pretty(config)?;
    fs::write(config_path, config_str)?;

    Ok(())
}

pub fn load_channel_mappings(app_handle: &AppHandle) -> Result<Vec<ChannelMapping>> {
    let config = load_config(app_handle)?;
    Ok(config.channel_mappings)
}

pub fn save_channel_mappings(
    mappings: &[ChannelMapping],
    app_handle: &AppHandle,
) -> Result<()> {
    let mut config = load_config(app_handle)?;
    config.channel_mappings = mappings.to_vec();
    save_config(app_handle, &config)?;

    Ok(())
}

pub fn update_settings(
    app_handle: &AppHandle,
    start_with_windows: Option<bool>,
    minimize_to_tray: Option<bool>,
    auto_connect: Option<bool>,
    theme: Option<String>,
) -> Result<()> {
    let mut config = load_config(app_handle)?;

    if let Some(value) = start_with_windows {
        config.start_with_windows = value;
    }

    if let Some(value) = minimize_to_tray {
        config.minimize_to_tray = value;
    }

    if let Some(value) = auto_connect {
        config.auto_connect = value;
    }

    if let Some(value) = theme {
        config.theme = value;
    }

    save_config(app_handle, &config)?;

    Ok(())
}