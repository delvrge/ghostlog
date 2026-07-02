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

    // Free tier watches exactly ONE project, enforced structurally:
    // the folder must be the ROOT of a git repository. This rejects both
    // umbrella folders (a parent holding many projects has no .git) and
    // subfolders inside a project (their repo root is above them).
    if !canonical.join(".git").exists() {
        return Err(
            "This folder is not a git project. Ghostlog (free) watches a single \
             project — select the root folder of one repository. Watching \
             multiple projects at once is a Ghostlog Pro feature."
                .into(),
        );
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
/// current session (creating one if needed). The note is just a hint for
/// the model, NOT the documentation itself: the real material is the
/// working-tree git diff (staged + unstaged), which the model uses to
/// reconstruct what actually changed, why, and how it was fixed. Title/
/// tag/summary come from the local model configured in Settings > AI
/// provider, via ai.rs; if none is configured (or the call fails), a
/// clearly-labeled mock draft is used instead — capture always succeeds.
#[tauri::command]
pub async fn manual_capture(app: AppHandle, note: Option<String>) -> Result<storage::SessionEntry, String> {
    let state = app.state::<AppState>();
    let (project, repo) = {
        let watched = state.watched_path.lock().unwrap();
        let root = watched.as_ref().ok_or("No watched folder configured")?;
        (storage::project_name(root)?, root.clone())
    };
    let diff = storage::working_tree_diff(&repo);

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
    let diff_context = if diff.trim().is_empty() { None } else { Some(diff.as_str()) };
    let draft = crate::ai::summarize_capture(&note_text, diff_context).await;
    let entry =
        storage::write_entry(&project, &date, &session_id, &draft.tag, &draft.title, &draft.summary)?;
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

// ---- Settings: git-commit trigger ----

fn watched_path(state: &State<AppState>) -> Result<std::path::PathBuf, String> {
    state
        .watched_path
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "No watched folder configured".to_string())
}

#[tauri::command]
pub fn is_git_hook_enabled(state: State<AppState>) -> Result<bool, String> {
    Ok(storage::is_git_hook_installed(&watched_path(&state)?))
}

#[tauri::command]
pub fn set_git_hook_enabled(state: State<AppState>, enabled: bool) -> Result<(), String> {
    let repo = watched_path(&state)?;
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        storage::install_git_hook(&repo, &exe)
    } else {
        storage::uninstall_git_hook(&repo)
    }
}

/// Extension connection status. Live handshake detection would need the
/// short-lived native-host subprocess to report back to this long-running
/// app, which isn't built — so this only reflects whether the host manifest
/// is registered with the browser, not whether a session is active right now.
#[tauri::command]
pub fn get_extension_status() -> &'static str {
    "disconnected"
}

#[tauri::command]
pub fn is_native_host_installed() -> bool {
    storage::is_native_host_installed()
}

#[tauri::command]
pub fn install_native_host() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    storage::install_native_host(&exe)
}

#[tauri::command]
pub fn uninstall_native_host() -> Result<(), String> {
    storage::uninstall_native_host()
}

/// "Guaranteed" auto-capture trigger: installs a small marked block into
/// the user's ~/.zshrc that fires on any nonzero-exit command, regardless
/// of whether a human or an AI coding tool ran it.
#[tauri::command]
pub fn is_shell_hook_installed() -> bool {
    storage::is_shell_hook_installed()
}

#[tauri::command]
pub fn install_shell_hook() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    storage::install_shell_hook(&exe)
}

#[tauri::command]
pub fn uninstall_shell_hook() -> Result<(), String> {
    storage::uninstall_shell_hook()
}

// ---- Settings: AI provider ----
// Free tier ships with no preset — the user points Ghostlog at their own
// local/self-hosted endpoint. ai-stub.ts reads this to decide whether to
// call a real model or keep returning mock data; storing the config here
// does NOT itself wire up any model call (see ai-stub.ts for that boundary).

#[tauri::command]
pub fn get_ai_config() -> storage::AiConfig {
    storage::load_ai_config()
}

#[tauri::command]
pub fn set_ai_config(endpoint: String, model: String) -> Result<(), String> {
    storage::save_ai_config(&storage::AiConfig { endpoint, model })
}

/// Backs ai-stub.ts's compileEntries — the UI-driven path always has a
/// webview, but we still keep the real call in ai.rs (single source of
/// truth for backend model calls, shared with the CLI git-hook path).
#[tauri::command]
pub async fn ai_compile(entries: Vec<String>) -> String {
    crate::ai::compile_document(&entries).await
}

// ---- Settings: output folder + export ----
// Separate from the watched (input) folder: this is the one place Ghostlog
// writes files the user asked for, chosen explicitly, never auto-written to.

#[tauri::command]
pub fn get_output_folder() -> Option<String> {
    storage::load_output_folder().map(|p| p.display().to_string())
}

#[tauri::command]
pub fn set_output_folder(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    let canonical = p.canonicalize().map_err(|e| format!("Cannot access folder: {e}"))?;
    if !canonical.is_dir() {
        return Err("Selected path is not a directory".into());
    }
    storage::save_output_folder(&canonical)
}

/// Writes a compiled document to the configured output folder. Returns the
/// full path written, so the UI can tell the user exactly where it went.
#[tauri::command]
pub fn export_document(filename: String, content: String) -> Result<String, String> {
    storage::export_document(&filename, &content)
}
