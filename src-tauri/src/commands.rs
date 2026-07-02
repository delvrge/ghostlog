//! Tauri commands — the ONLY channel between the React frontend and the
//! backend (Tauri's built-in IPC; no HTTP, no ports).

use crate::state::{AppState, LastEvent, WatchState};
use crate::storage;
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
    *state.watched_path.lock().unwrap() = Some(canonical.clone());
    storage::save_config(&canonical)?;
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

/// Manual "Log this now" trigger — writes a real entry file into the
/// current session (creating one if needed).
/// STUB: title/summary are placeholder text; the real flow will feed the
/// note + recent git diff/log through the local model (ai-stub → Ollama).
#[tauri::command]
pub fn manual_capture(app: AppHandle, note: Option<String>) -> Result<storage::SessionEntry, String> {
    let state = app.state::<AppState>();
    let project = {
        let watched = state.watched_path.lock().unwrap();
        let root = watched.as_ref().ok_or("No watched folder configured")?;
        storage::project_name(root)?
    };

    // Reuse the active session or lazily create one (manual capture must
    // work even when not actively watching).
    let (date, session_id) = {
        let mut cur = state.current_session.lock().unwrap();
        match cur.clone() {
            Some(s) => s,
            None => {
                let s = storage::create_session(&project)?;
                *cur = Some(s.clone());
                s
            }
        }
    };

    let note_text = note.unwrap_or_else(|| "manual capture".to_string());
    let entry = storage::write_entry(
        &project,
        &date,
        &session_id,
        "update",
        &note_text,
        "Placeholder summary — recent git diff/log context will be summarized \
         by the local model here once one is wired in.",
    )?;
    watcher::record_event(&app, "manual", note_text);
    Ok(entry)
}

// ---- Session archive (read/browse any past date) ----

fn current_project(state: &State<AppState>) -> Result<String, String> {
    let watched = state.watched_path.lock().unwrap();
    let root = watched.as_ref().ok_or("No watched folder configured")?;
    storage::project_name(root)
}

#[tauri::command]
pub fn list_session_dates(state: State<AppState>) -> Result<Vec<String>, String> {
    storage::list_dates(&current_project(&state)?)
}

#[tauri::command]
pub fn list_sessions(state: State<AppState>, date: String) -> Result<Vec<storage::SessionMeta>, String> {
    storage::list_sessions(&current_project(&state)?, &date)
}

#[tauri::command]
pub fn read_session(
    state: State<AppState>,
    date: String,
    session_id: String,
) -> Result<Vec<storage::SessionEntry>, String> {
    storage::read_session(&current_project(&state)?, &date, &session_id)
}

#[tauri::command]
pub fn update_entry(
    state: State<AppState>,
    date: String,
    session_id: String,
    entry_id: String,
    tag: String,
    title: String,
    summary: String,
) -> Result<(), String> {
    storage::update_entry(&current_project(&state)?, &date, &session_id, &entry_id, &tag, &title, &summary)
}

#[tauri::command]
pub fn delete_entry(
    state: State<AppState>,
    date: String,
    session_id: String,
    entry_id: String,
) -> Result<(), String> {
    storage::delete_entry(&current_project(&state)?, &date, &session_id, &entry_id)
}
