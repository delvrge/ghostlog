//! Tauri commands — the ONLY channel between the React frontend and the
//! backend (Tauri's built-in IPC; no HTTP, no ports).

use crate::state::{AppState, LastEvent, WatchState};
use crate::watcher;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn start_watching(app: AppHandle) -> Result<(), String> {
    watcher::start(&app)
}

#[tauri::command]
pub fn stop_watching(app: AppHandle) {
    watcher::stop(&app);
}

#[tauri::command]
pub fn get_watch_state(state: State<AppState>) -> WatchState {
    state.watch_state.lock().unwrap().state
}

#[tauri::command]
pub fn get_last_event(state: State<AppState>) -> Option<LastEvent> {
    state.last_event.lock().unwrap().clone()
}

/// Set the single watched folder (free tier = exactly one).
/// Validated here in Rust: must exist and be a directory.
#[tauri::command]
pub fn set_watched_folder(app: AppHandle, path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let canonical = p
        .canonicalize()
        .map_err(|e| format!("Cannot access folder: {e}"))?;
    if !canonical.is_dir() {
        return Err("Selected path is not a directory".into());
    }
    let state = app.state::<AppState>();
    *state.watched_path.lock().unwrap() = Some(canonical);
    Ok(())
}

#[tauri::command]
pub fn get_watched_folder(state: State<AppState>) -> Option<String> {
    state
        .watched_path
        .lock()
        .unwrap()
        .as_ref()
        .map(|p| p.display().to_string())
}

/// Manual "Log this now" trigger.
/// MOCK CAPTURE: records the event only; git diff/log context capture and
/// session-file writing land with the data layer (delivery step 3).
#[tauri::command]
pub fn manual_capture(app: AppHandle, note: Option<String>) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state.watched_path.lock().unwrap().is_none() {
        return Err("No watched folder configured".into());
    }
    let detail = note.unwrap_or_else(|| "manual capture".to_string());
    watcher::record_event(&app, "manual", detail);
    Ok(())
}
