mod audio;
mod config;
mod serial;
mod types;

use audio::{AudioManager, WindowsAudioManager};
use serial::SerialManager;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use types::{AudioSession, ConnectionStatus, MixerChannel, PotMapping, SerialPortInfo};

// Constants for magic numbers
const AUDIO_SESSION_POLL_INTERVAL_MS: u64 = 500;

struct AppState {
    serial_manager: Arc<SerialManager>,
    audio_manager: Arc<dyn AudioManager>,
    cancellation_token: CancellationToken,
    last_audio_sessions: Arc<RwLock<Vec<AudioSession>>>,
    pot_mappings: Arc<RwLock<Vec<PotMapping>>>,
    app_handle: AppHandle,
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

        // Spawn task to emit pot data events and apply mapped volumes
        let app_handle_clone = app_handle.clone();
        let audio_manager = state.audio_manager.clone();
        let pot_mappings = state.pot_mappings.clone();
        let last_sessions = state.last_audio_sessions.clone();

        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                // Emit raw pot data
                if let Err(e) = app_handle_clone.emit("pot-data", &data) {
                    log::error!("Failed to emit pot-data event: {}", e);
                }

                // Apply mapped volumes — clone data out of locks to avoid holding
                // them across potentially slow audio manager calls
                let percentages = data.to_percentages();
                let mappings = pot_mappings.read().await.clone();
                let sessions = last_sessions.read().await.clone();

                for mapping in mappings.iter() {
                    let idx = (mapping.pot_index as usize).saturating_sub(1);
                    if idx >= percentages.len() {
                        continue;
                    }
                    let volume = percentages[idx];

                    if mapping.process_name.eq_ignore_ascii_case("master") {
                        let _ = audio_manager.set_master_volume(volume);
                    } else {
                        // Find matching session by process name (case-insensitive)
                        if let Some(session) = sessions.iter().find(|s| {
                            s.process_name.eq_ignore_ascii_case(&mapping.process_name)
                        }) {
                            let _ =
                                audio_manager.set_app_volume(session.process_id, volume);
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
async fn get_pot_mappings(state: State<'_, AppState>) -> Result<Vec<PotMapping>, String> {
    let mappings = state.pot_mappings.read().await;
    Ok(mappings.clone())
}

#[tauri::command]
async fn set_pot_mapping(
    state: State<'_, AppState>,
    pot_index: u8,
    process_name: Option<String>,
) -> Result<Vec<PotMapping>, String> {
    // Validate pot_index range
    if !(1..=8).contains(&pot_index) {
        return Err(format!("Invalid pot_index: {}. Must be 1-8.", pot_index));
    }

    // Update in-memory state, then release lock before I/O
    let result = {
        let mut mappings = state.pot_mappings.write().await;

        // Remove existing mapping for this pot
        mappings.retain(|m| m.pot_index != pot_index);

        // Add new mapping if process_name provided
        if let Some(name) = process_name {
            // Also remove any existing mapping for this process (one app = one pot)
            mappings.retain(|m| !m.process_name.eq_ignore_ascii_case(&name));

            mappings.push(PotMapping {
                pot_index,
                process_name: name,
            });
        }

        mappings.clone()
    }; // write lock released here

    // Persist to config (outside of lock)
    if let Err(e) = config::save_pot_mappings(&state.app_handle, &result) {
        log::error!("Failed to save pot mappings: {}", e);
    }

    // Emit update event
    if let Err(e) = state.app_handle.emit("pot-mappings-updated", &result) {
        log::error!("Failed to emit pot-mappings-updated event: {}", e);
    }

    Ok(result)
}

#[tauri::command]
async fn get_mixer_channels(_state: State<'_, AppState>) -> Result<Vec<MixerChannel>, String> {
    let mut channels = Vec::new();

    // Return 8 physical channels
    for i in 1..=8 {
        channels.push(MixerChannel {
            id: i,
            value: 0.0,
            is_physical: true,
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

            // Load saved pot mappings from config
            let saved_mappings = config::load_pot_mappings(&app_handle).unwrap_or_default();

            let app_state = AppState {
                serial_manager: Arc::new(SerialManager::new()),
                audio_manager: Arc::new(WindowsAudioManager::new()),
                cancellation_token: CancellationToken::new(),
                last_audio_sessions: Arc::new(RwLock::new(Vec::new())),
                pot_mappings: Arc::new(RwLock::new(saved_mappings)),
                app_handle: app_handle.clone(),
            };

            app.manage(app_state);

            // Setup window close handler to minimize to tray
            if let Some(window) = app.get_webview_window("main") {
                // Enable DevTools in debug builds
                #[cfg(debug_assertions)]
                {
                    window.open_devtools();
                    log::info!("DevTools enabled for debugging");
                }

                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // Prevent the window from closing
                        api.prevent_close();
                        // Hide the window instead
                        let _ = window_clone.hide();
                    }
                });
            }

            // Setup system tray
            #[cfg(desktop)]
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::{TrayIconBuilder, TrayIconEvent};

                let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
                let hide = MenuItem::with_id(app, "hide", "Hide", true, None::<&str>)?;

                let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

                let _tray = TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .icon_as_template(true)
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
                    if let Err(e) = app_handle_clone.emit("connection-status", &status) {
                        log::error!("Failed to emit connection-status event: {}", e);
                    }
                }
            });

            // Start audio session polling with proper cancellation
            let audio_manager = state.audio_manager.clone();
            let app_handle_clone2 = app_handle.clone();
            let cancellation_token = state.cancellation_token.clone();
            let last_sessions_state = state.last_audio_sessions.clone();

            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::select! {
                        _ = cancellation_token.cancelled() => {
                            // Clean shutdown
                            log::info!("Audio session polling task cancelled");
                            break;
                        }
                        _ = tokio::time::sleep(tokio::time::Duration::from_millis(AUDIO_SESSION_POLL_INTERVAL_MS)) => {
                            // Get current audio sessions
                            match audio_manager.get_audio_sessions() {
                                Ok(current_sessions) => {
                                    // Always update and emit so volume level changes are reflected in real-time
                                    let mut last = last_sessions_state.write().await;
                                    *last = current_sessions.clone();
                                    drop(last);

                                    if let Err(e) = app_handle_clone2.emit("audio-sessions-updated", &current_sessions) {
                                        log::error!("Failed to emit audio-sessions-updated event: {}", e);
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to get audio sessions: {}", e);
                                }
                            }
                        }
                    }
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
            get_mixer_channels,
            get_pot_mappings,
            set_pot_mapping,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
