mod audio;
mod config;
mod serial;
mod types;

use audio::{AudioManager, WindowsAudioManager};
use serial::SerialManager;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;
use types::{AudioSession, ChannelMapping, ConnectionStatus, MixerChannel, SerialPortInfo};

struct AppState {
    serial_manager: Arc<SerialManager>,
    audio_manager: Arc<dyn AudioManager>,
    channel_mappings: Arc<Mutex<Vec<ChannelMapping>>>,
}

#[tauri::command]
async fn list_serial_ports() -> Result<Vec<SerialPortInfo>, String> {
    SerialManager::list_ports().map_err(|e| e.to_string())
}

#[tauri::command]
async fn connect_serial(
    state: State<'_, AppState>,
    port: Option<String>,
    app_handle: AppHandle,
) -> Result<ConnectionStatus, String> {
    let status = state
        .serial_manager
        .connect(port)
        .map_err(|e| e.to_string())?;

    if status.connected {
        // Start reading data and emitting events
        let (tx, mut rx) = mpsc::channel(100);

        let serial_manager = state.serial_manager.clone();
        serial_manager
            .start_reading(tx)
            .await
            .map_err(|e| e.to_string())?;

        // Spawn task to emit pot data events
        let app_handle_clone = app_handle.clone();
        let channel_mappings = state.channel_mappings.clone();
        let audio_manager = state.audio_manager.clone();

        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                // Emit raw pot data
                let _ = app_handle_clone.emit("pot-data", &data);

                // Apply volume changes based on mappings
                let mappings = channel_mappings.lock().unwrap().clone();
                let (pot1, pot2, pot3) = data.to_percentages();
                let pot_values = vec![pot1, pot2, pot3];

                for (idx, pot_value) in pot_values.iter().enumerate() {
                    if let Some(mapping) = mappings.iter().find(|m| m.channel_id == idx + 1) {
                        if mapping.is_master {
                            let _ = audio_manager.set_master_volume(*pot_value);
                        } else if let Some(process_id) = mapping.process_id {
                            let _ = audio_manager.set_app_volume(process_id, *pot_value);
                        }
                    }
                }
            }
        });
    }

    Ok(status)
}

#[tauri::command]
async fn disconnect_serial(state: State<'_, AppState>) -> Result<(), String> {
    state.serial_manager.disconnect();
    Ok(())
}

#[tauri::command]
async fn get_serial_status(state: State<'_, AppState>) -> Result<ConnectionStatus, String> {
    Ok(state.serial_manager.get_status())
}

#[tauri::command]
async fn get_audio_sessions(state: State<'_, AppState>) -> Result<Vec<AudioSession>, String> {
    state
        .audio_manager
        .get_audio_sessions()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_app_volume(
    state: State<'_, AppState>,
    process_id: u32,
    volume: f32,
) -> Result<(), String> {
    state
        .audio_manager
        .set_app_volume(process_id, volume)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_master_volume(state: State<'_, AppState>, volume: f32) -> Result<(), String> {
    state
        .audio_manager
        .set_master_volume(volume)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_master_volume(state: State<'_, AppState>) -> Result<f32, String> {
    state
        .audio_manager
        .get_master_volume()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_channel_mapping(
    state: State<'_, AppState>,
    mapping: ChannelMapping,
    app_handle: AppHandle,
) -> Result<(), String> {
    let mut mappings = state.channel_mappings.lock().unwrap();

    // Remove existing mapping for this channel
    mappings.retain(|m| m.channel_id != mapping.channel_id);

    // Add new mapping
    mappings.push(mapping.clone());

    // Save to config
    config::save_channel_mappings(&mappings, &app_handle).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn clear_channel_mapping(
    state: State<'_, AppState>,
    channel_id: usize,
    app_handle: AppHandle,
) -> Result<(), String> {
    let mut mappings = state.channel_mappings.lock().unwrap();
    mappings.retain(|m| m.channel_id != channel_id);

    // Save to config
    config::save_channel_mappings(&mappings, &app_handle).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn get_channel_mappings(state: State<'_, AppState>) -> Result<Vec<ChannelMapping>, String> {
    Ok(state.channel_mappings.lock().unwrap().clone())
}

#[tauri::command]
async fn get_mixer_channels(state: State<'_, AppState>) -> Result<Vec<MixerChannel>, String> {
    let mappings = state.channel_mappings.lock().unwrap().clone();
    let mut channels = Vec::new();

    for i in 1..=8 {
        let mapping = mappings.iter().find(|m| m.channel_id == i);
        channels.push(MixerChannel {
            id: i,
            value: 0.0,
            is_physical: i <= 3,
            mapped_app: mapping.and_then(|m| m.process_name.clone()),
            app_process_id: mapping.and_then(|m| m.process_id),
        });
    }

    Ok(channels)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Load saved channel mappings
            let channel_mappings = config::load_channel_mappings(&app_handle).unwrap_or_default();

            let app_state = AppState {
                serial_manager: Arc::new(SerialManager::new()),
                audio_manager: Arc::new(WindowsAudioManager::new()),
                channel_mappings: Arc::new(Mutex::new(channel_mappings)),
            };

            app.manage(app_state);

            // Setup system tray
            #[cfg(desktop)]
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::{TrayIconBuilder, TrayIconEvent};

                let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
                let hide = MenuItem::with_id(app, "hide", "Hide", true, None::<&str>)?;

                let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

                let tray = TrayIconBuilder::new()
                    .menu(&menu)
                    .tooltip("PC Audio Mixer")
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "quit" => {
                            app.exit(0);
                        }
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "hide" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.hide();
                            }
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click { .. } = event {
                            if let Some(app) = tray.app_handle().get_webview_window("main") {
                                if app.is_visible().unwrap_or(false) {
                                    let _ = app.hide();
                                } else {
                                    let _ = app.show();
                                    let _ = app.set_focus();
                                }
                            }
                        }
                    })
                    .build(app)?;
            }

            // Auto-connect to Pico on startup
            let state = app.state::<AppState>();
            let serial_manager = state.serial_manager.clone();
            let app_handle_clone = app_handle.clone();

            tauri::async_runtime::spawn(async move {
                // Wait a bit for the UI to be ready
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                // Try auto-connect
                if let Ok(status) = serial_manager.connect(None) {
                    let _ = app_handle_clone.emit("connection-status", &status);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_serial_ports,
            connect_serial,
            disconnect_serial,
            get_serial_status,
            get_audio_sessions,
            set_app_volume,
            set_master_volume,
            get_master_volume,
            save_channel_mapping,
            clear_channel_mapping,
            get_channel_mappings,
            get_mixer_channels,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
