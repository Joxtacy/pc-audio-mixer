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
use types::{AudioSession, ConnectionStatus, MixerChannel, SerialPortInfo};

// Constants for magic numbers
const AUDIO_SESSION_POLL_INTERVAL_SECS: u64 = 5;
const MASTER_VOLUME_PROCESS_ID: u32 = 0;

struct AppState {
    serial_manager: Arc<SerialManager>,
    audio_manager: Arc<dyn AudioManager>,
    cancellation_token: CancellationToken,
    last_audio_sessions: Arc<RwLock<Vec<AudioSession>>>,
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
        let audio_manager = state.audio_manager.clone();

        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                // Emit raw pot data
                if let Err(e) = app_handle_clone.emit("pot-data", &data) {
                    log::error!("Failed to emit pot-data event: {}", e);
                }

                // Use pot1 to control master volume directly
                let (pot1, _pot2, _pot3) = data.to_percentages();
                let _ = audio_manager.set_master_volume(pot1);
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
async fn get_mixer_channels(_state: State<'_, AppState>) -> Result<Vec<MixerChannel>, String> {
    let mut channels = Vec::new();

    // Only return 3 physical channels
    for i in 1..=3 {
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

            let app_state = AppState {
                serial_manager: Arc::new(SerialManager::new()),
                audio_manager: Arc::new(WindowsAudioManager::new()),
                cancellation_token: CancellationToken::new(),
                last_audio_sessions: Arc::new(RwLock::new(Vec::new())),
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

                let _tray = TrayIconBuilder::new()
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
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(AUDIO_SESSION_POLL_INTERVAL_SECS)) => {
                            // Get current audio sessions
                            match audio_manager.get_audio_sessions() {
                                Ok(current_sessions) => {
                                    // Use RwLock for thread-safe comparison and update
                                    let mut should_emit = false;
                                    {
                                        let last = last_sessions_state.read().await;
                                        if *last != current_sessions {
                                            should_emit = true;
                                        }
                                    }

                                    if should_emit {
                                        // Update stored sessions atomically
                                        {
                                            let mut last = last_sessions_state.write().await;
                                            *last = current_sessions.clone();
                                        }

                                        // Emit update event with error handling
                                        if let Err(e) = app_handle_clone2.emit("audio-sessions-updated", &current_sessions) {
                                            log::error!("Failed to emit audio-sessions-updated event: {}", e);
                                        }
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
