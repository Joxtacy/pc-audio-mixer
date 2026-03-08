mod audio;
mod config;
mod serial;
mod types;

use audio::{AudioManager, PlatformAudioManager};
use serial::SerialManager;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use types::{AudioSession, ConnectionStatus, MixerChannel, PotMapping, SerialPortInfo};

// Constants for magic numbers
const AUDIO_SESSION_POLL_INTERVAL_MS: u64 = 500;
const AUTO_RECONNECT_INTERVAL_SECS: u64 = 2;

/// Shared helper: starts the serial reading pipeline after a successful connect.
/// Aborts any existing reading task before starting a new one.
async fn start_serial_reading(
    serial_manager: &Arc<SerialManager>,
    app_handle: &AppHandle,
    audio_manager: &Arc<dyn AudioManager>,
    pot_mappings: &Arc<RwLock<Vec<PotMapping>>>,
    last_sessions: &Arc<RwLock<Vec<AudioSession>>>,
    reading_task: &Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
) -> Result<(), String> {
    // Abort existing reading task if present
    {
        let mut task_guard = reading_task.write().await;
        if let Some(handle) = task_guard.take() {
            handle.abort();
        }
    }

    let (tx, mut rx) = mpsc::channel(100);

    serial_manager
        .start_reading(tx)
        .await
        .map_err(|e| e.to_string())?;

    let app_handle_clone = app_handle.clone();
    let audio_manager = audio_manager.clone();
    let pot_mappings = pot_mappings.clone();
    let last_sessions = last_sessions.clone();

    let handle = tokio::spawn(async move {
        let mut last_applied: std::collections::HashMap<u32, f32> =
            std::collections::HashMap::new();

        while let Some(data) = rx.recv().await {
            if let Err(e) = app_handle_clone.emit("pot-data", &data) {
                log::error!("Failed to emit pot-data event: {}", e);
            }

            let percentages = data.to_percentages();
            let mappings = pot_mappings.read().await.clone();
            let sessions = last_sessions.read().await.clone();

            let mut volume_batch: Vec<(u32, f32)> = Vec::new();
            for mapping in mappings.iter() {
                let idx = (mapping.pot_index as usize).saturating_sub(1);
                if idx >= percentages.len() {
                    continue;
                }
                let volume = percentages[idx];

                let pid = if mapping.process_name.eq_ignore_ascii_case("master") {
                    Some(0u32)
                } else {
                    sessions
                        .iter()
                        .find(|s| s.process_name.eq_ignore_ascii_case(&mapping.process_name))
                        .map(|s| s.process_id)
                };

                if let Some(pid) = pid {
                    if last_applied.get(&pid) != Some(&volume) {
                        volume_batch.push((pid, volume));
                    }
                }
            }

            if !volume_batch.is_empty() && audio_manager.set_volumes_batch(&volume_batch).is_ok() {
                for (pid, vol) in &volume_batch {
                    last_applied.insert(*pid, *vol);
                }
            }
        }
    });

    // Store the new task handle
    {
        let mut task_guard = reading_task.write().await;
        *task_guard = Some(handle);
    }

    Ok(())
}

struct AppState {
    serial_manager: Arc<SerialManager>,
    audio_manager: Arc<dyn AudioManager>,
    cancellation_token: CancellationToken,
    last_audio_sessions: Arc<RwLock<Vec<AudioSession>>>,
    pot_mappings: Arc<RwLock<Vec<PotMapping>>>,
    app_handle: AppHandle,
    is_reconnecting: Arc<AtomicBool>,
    auto_reconnect_enabled: Arc<AtomicBool>,
    reading_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    #[cfg(desktop)]
    menu_items: Arc<std::sync::RwLock<Option<MenuItems>>>,
}

#[cfg(desktop)]
struct MenuItems {
    tray_connect: tauri::menu::MenuItem<tauri::Wry>,
    tray_disconnect: tauri::menu::MenuItem<tauri::Wry>,
    menu_connect: tauri::menu::MenuItem<tauri::Wry>,
    menu_disconnect: tauri::menu::MenuItem<tauri::Wry>,
}

#[cfg(desktop)]
fn update_menu_states(menu_items_lock: &std::sync::RwLock<Option<MenuItems>>, connected: bool) {
    if let Ok(guard) = menu_items_lock.read() {
        if let Some(ref items) = *guard {
            let _ = items.tray_connect.set_enabled(!connected);
            let _ = items.tray_disconnect.set_enabled(connected);
            let _ = items.menu_connect.set_enabled(!connected);
            let _ = items.menu_disconnect.set_enabled(connected);
        }
    }
}

/// Shared helper for connect actions (used by command, tray, menu, and auto-reconnect).
/// Handles the is_reconnecting guard, connect, start reading, event emission, and menu updates.
#[allow(clippy::too_many_arguments)]
async fn handle_connect_action(
    serial_manager: &Arc<SerialManager>,
    app_handle: &AppHandle,
    audio_manager: &Arc<dyn AudioManager>,
    pot_mappings: &Arc<RwLock<Vec<PotMapping>>>,
    last_sessions: &Arc<RwLock<Vec<AudioSession>>>,
    reading_task: &Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    is_reconnecting: &Arc<AtomicBool>,
    auto_reconnect_enabled: &Arc<AtomicBool>,
    #[cfg(desktop)] menu_items: &Arc<std::sync::RwLock<Option<MenuItems>>>,
    port: Option<String>,
) -> Result<ConnectionStatus, String> {
    // Guard against concurrent connect attempts
    if is_reconnecting
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(serial_manager.get_status());
    }
    // Ensure is_reconnecting is reset on ALL exit paths (F1 fix)
    let is_reconnecting_guard = is_reconnecting.clone();
    let _guard = scopeguard::guard((), move |_| {
        is_reconnecting_guard.store(false, Ordering::SeqCst);
    });

    // Re-enable auto-reconnect on manual connect
    auto_reconnect_enabled.store(true, Ordering::SeqCst);

    let status = serial_manager.connect(port).map_err(|e| e.to_string())?;

    if status.connected {
        start_serial_reading(
            serial_manager,
            app_handle,
            audio_manager,
            pot_mappings,
            last_sessions,
            reading_task,
        )
        .await?;
    }

    // Emit connection-status event (F3 fix)
    if let Err(e) = app_handle.emit("connection-status", &status) {
        log::error!("Failed to emit connection-status event: {}", e);
    }

    #[cfg(desktop)]
    update_menu_states(menu_items, status.connected);

    Ok(status)
}

/// Shared helper for disconnect actions (used by command, tray, and menu).
async fn handle_disconnect_action(
    serial_manager: &Arc<SerialManager>,
    app_handle: &AppHandle,
    reading_task: &Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    auto_reconnect_enabled: &Arc<AtomicBool>,
    #[cfg(desktop)] menu_items: &Arc<std::sync::RwLock<Option<MenuItems>>>,
) {
    // Abort reading task
    {
        let mut task_guard = reading_task.write().await;
        if let Some(handle) = task_guard.take() {
            handle.abort();
        }
    }

    serial_manager.disconnect();

    // Disable auto-reconnect on manual disconnect (F2 fix)
    auto_reconnect_enabled.store(false, Ordering::SeqCst);

    let status = serial_manager.get_status();
    if let Err(e) = app_handle.emit("connection-status", &status) {
        log::error!("Failed to emit connection-status event: {}", e);
    }

    #[cfg(desktop)]
    update_menu_states(menu_items, false);
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
    handle_connect_action(
        &state.serial_manager,
        &app_handle,
        &state.audio_manager,
        &state.pot_mappings,
        &state.last_audio_sessions,
        &state.reading_task,
        &state.is_reconnecting,
        &state.auto_reconnect_enabled,
        #[cfg(desktop)]
        &state.menu_items,
        port,
    )
    .await
}

#[tauri::command]
async fn disconnect_serial(state: State<'_, AppState>) -> Result<(), String> {
    handle_disconnect_action(
        &state.serial_manager,
        &state.app_handle,
        &state.reading_task,
        &state.auto_reconnect_enabled,
        #[cfg(desktop)]
        &state.menu_items,
    )
    .await;
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

            let is_reconnecting = Arc::new(AtomicBool::new(false));
            let auto_reconnect_enabled = Arc::new(AtomicBool::new(true));
            let reading_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>> =
                Arc::new(RwLock::new(None));

            #[cfg(desktop)]
            let menu_items_arc: Arc<std::sync::RwLock<Option<MenuItems>>> =
                Arc::new(std::sync::RwLock::new(None));

            let app_state = AppState {
                serial_manager: Arc::new(SerialManager::new()),
                audio_manager: Arc::new(PlatformAudioManager::new()),
                cancellation_token: CancellationToken::new(),
                last_audio_sessions: Arc::new(RwLock::new(Vec::new())),
                pot_mappings: Arc::new(RwLock::new(saved_mappings)),
                app_handle: app_handle.clone(),
                is_reconnecting: is_reconnecting.clone(),
                auto_reconnect_enabled: auto_reconnect_enabled.clone(),
                reading_task: reading_task.clone(),
                #[cfg(desktop)]
                menu_items: menu_items_arc.clone(),
            };

            app.manage(app_state);

            // Setup window close handler to minimize to tray
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(debug_assertions)]
                {
                    window.open_devtools();
                    log::info!("DevTools enabled for debugging");
                }

                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                });
            }

            // Setup system tray and app menu
            #[cfg(desktop)]
            {
                use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
                use tauri::tray::TrayIconBuilder;

                // Tray menu items
                let tray_connect =
                    MenuItem::with_id(app, "tray_connect", "Connect", true, None::<&str>)?;
                let tray_disconnect =
                    MenuItem::with_id(app, "tray_disconnect", "Disconnect", false, None::<&str>)?;
                let separator = PredefinedMenuItem::separator(app)?;
                let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
                let hide = MenuItem::with_id(app, "hide", "Hide", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

                let tray_menu = Menu::with_items(
                    app,
                    &[
                        &tray_connect,
                        &tray_disconnect,
                        &separator,
                        &show,
                        &hide,
                        &quit,
                    ],
                )?;

                // Clone state refs for tray event handler
                let state_for_tray = app.state::<AppState>();
                let sm_tray = state_for_tray.serial_manager.clone();
                let am_tray = state_for_tray.audio_manager.clone();
                let pm_tray = state_for_tray.pot_mappings.clone();
                let ls_tray = state_for_tray.last_audio_sessions.clone();
                let rt_tray = state_for_tray.reading_task.clone();
                let ir_tray = state_for_tray.is_reconnecting.clone();
                let ar_tray = state_for_tray.auto_reconnect_enabled.clone();
                let mi_tray = state_for_tray.menu_items.clone();

                let _tray = TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .icon_as_template(true)
                    .menu(&tray_menu)
                    .tooltip("PC Audio Mixer")
                    .on_menu_event(move |app, event| {
                        match event.id.as_ref() {
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
                            "tray_connect" => {
                                let sm = sm_tray.clone();
                                let am = am_tray.clone();
                                let pm = pm_tray.clone();
                                let ls = ls_tray.clone();
                                let rt = rt_tray.clone();
                                let ir = ir_tray.clone();
                                let ar = ar_tray.clone();
                                let mi = mi_tray.clone();
                                let ah = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    let _ = handle_connect_action(
                                        &sm, &ah, &am, &pm, &ls, &rt, &ir, &ar,
                                        #[cfg(desktop)]
                                        &mi,
                                        None,
                                    )
                                    .await;
                                });
                            }
                            "tray_disconnect" => {
                                let sm = sm_tray.clone();
                                let rt = rt_tray.clone();
                                let ar = ar_tray.clone();
                                let mi = mi_tray.clone();
                                let ah = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    handle_disconnect_action(
                                        &sm, &ah, &rt, &ar,
                                        #[cfg(desktop)]
                                        &mi,
                                    )
                                    .await;
                                });
                            }
                            _ => {}
                        }
                    })
                    .build(app)?;

                // App menu bar with Connection submenu
                let menu_connect =
                    MenuItem::with_id(app, "menu_connect", "Connect", true, None::<&str>)?;
                let menu_disconnect =
                    MenuItem::with_id(app, "menu_disconnect", "Disconnect", false, None::<&str>)?;

                let connection_submenu = Submenu::with_items(
                    app,
                    "Connection",
                    true,
                    &[&menu_connect, &menu_disconnect],
                )?;

                // App submenu with standard Quit (Cmd+Q on macOS)
                let app_submenu = Submenu::with_items(
                    app,
                    "PC Audio Mixer",
                    true,
                    &[&PredefinedMenuItem::quit(app, Some("Quit"))?],
                )?;

                let app_menu =
                    Menu::with_items(app, &[&app_submenu, &connection_submenu])?;
                app.set_menu(app_menu)?;

                // Store menu item refs synchronously (F4 fix — no fire-and-forget spawn)
                {
                    let mut guard = menu_items_arc.write().unwrap();
                    *guard = Some(MenuItems {
                        tray_connect: tray_connect.clone(),
                        tray_disconnect: tray_disconnect.clone(),
                        menu_connect: menu_connect.clone(),
                        menu_disconnect: menu_disconnect.clone(),
                    });
                }

                // App menu event handler
                let state_for_menu = app.state::<AppState>();
                let sm_menu = state_for_menu.serial_manager.clone();
                let am_menu = state_for_menu.audio_manager.clone();
                let pm_menu = state_for_menu.pot_mappings.clone();
                let ls_menu = state_for_menu.last_audio_sessions.clone();
                let rt_menu = state_for_menu.reading_task.clone();
                let ir_menu = state_for_menu.is_reconnecting.clone();
                let ar_menu = state_for_menu.auto_reconnect_enabled.clone();
                let mi_menu = state_for_menu.menu_items.clone();

                app.on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "menu_connect" => {
                            let sm = sm_menu.clone();
                            let am = am_menu.clone();
                            let pm = pm_menu.clone();
                            let ls = ls_menu.clone();
                            let rt = rt_menu.clone();
                            let ir = ir_menu.clone();
                            let ar = ar_menu.clone();
                            let mi = mi_menu.clone();
                            let ah = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = handle_connect_action(
                                    &sm, &ah, &am, &pm, &ls, &rt, &ir, &ar,
                                    #[cfg(desktop)]
                                    &mi,
                                    None,
                                )
                                .await;
                            });
                        }
                        "menu_disconnect" => {
                            let sm = sm_menu.clone();
                            let rt = rt_menu.clone();
                            let ar = ar_menu.clone();
                            let mi = mi_menu.clone();
                            let ah = app.clone();
                            tauri::async_runtime::spawn(async move {
                                handle_disconnect_action(
                                    &sm, &ah, &rt, &ar,
                                    #[cfg(desktop)]
                                    &mi,
                                )
                                .await;
                            });
                        }
                        _ => {}
                    }
                });
            }

            // Auto-reconnect loop (replaces one-shot auto-connect)
            let state = app.state::<AppState>();
            let sm_reconnect = state.serial_manager.clone();
            let am_reconnect = state.audio_manager.clone();
            let pm_reconnect = state.pot_mappings.clone();
            let ls_reconnect = state.last_audio_sessions.clone();
            let rt_reconnect = state.reading_task.clone();
            let ir_reconnect = state.is_reconnecting.clone();
            let ar_reconnect = state.auto_reconnect_enabled.clone();
            let ct_reconnect = state.cancellation_token.clone();
            let ah_reconnect = app_handle.clone();
            #[cfg(desktop)]
            let mi_reconnect = state.menu_items.clone();

            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::select! {
                        _ = ct_reconnect.cancelled() => {
                            log::info!("Auto-reconnect task cancelled");
                            break;
                        }
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(AUTO_RECONNECT_INTERVAL_SECS)) => {
                            // Skip if auto-reconnect disabled (user manually disconnected)
                            if !ar_reconnect.load(Ordering::SeqCst) {
                                continue;
                            }

                            if sm_reconnect.is_connected() {
                                continue;
                            }

                            // Use the shared connect helper (handles guard, reading, events, menus)
                            let _ = handle_connect_action(
                                &sm_reconnect, &ah_reconnect, &am_reconnect,
                                &pm_reconnect, &ls_reconnect, &rt_reconnect,
                                &ir_reconnect, &ar_reconnect,
                                #[cfg(desktop)]
                                &mi_reconnect,
                                None,
                            ).await;
                        }
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
