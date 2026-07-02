//! GHLG native backend: tray, watcher, session storage, Tauri IPC commands.
//! Zero network ports by design — all frontend communication is Tauri IPC,
//! all extension communication will be Native Messaging (stdio).

mod commands;
mod state;
mod storage;
mod tray;
mod watcher;

use state::AppState;
use std::path::Path;

/// Entry point for the `--ghlg-git-commit <repo>` CLI mode (see main.rs).
pub fn capture_from_git_commit_cli(repo: &Path) -> Result<(), String> {
    let canonical = repo.canonicalize().map_err(|e| e.to_string())?;
    storage::capture_from_git_commit(&canonical)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
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
            commands::is_git_hook_enabled,
            commands::set_git_hook_enabled,
            commands::get_extension_status,
            commands::get_ai_config,
            commands::set_ai_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
