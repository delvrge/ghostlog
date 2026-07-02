//! GHLG native backend: tray, watcher, session storage, Tauri IPC commands.
//! Zero network ports by design — all frontend communication is Tauri IPC,
//! all extension communication will be Native Messaging (stdio).

mod commands;
mod state;
mod storage;
mod tray;
mod watcher;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            tray::init(app.handle())?;
            // Restore the previously chosen watched folder, if any.
            storage::load_config(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the review window hides it; the app lives in the tray.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_watching,
            commands::stop_watching,
            commands::get_watch_state,
            commands::get_last_event,
            commands::set_watched_folder,
            commands::get_watched_folder,
            commands::manual_capture,
            commands::list_session_dates,
            commands::list_sessions,
            commands::read_session,
            commands::update_entry,
            commands::delete_entry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
