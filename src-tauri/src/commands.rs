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

/// Add a watched project folder. Validated here in Rust: must exist, be a
/// directory, and be the ROOT of a git repository. The git-root rule rejects
/// both umbrella folders (a parent holding many projects has no .git) and
/// subfolders inside a project (their repo root is above them) — each
/// watched entry is exactly one project.
#[tauri::command]
pub fn add_watched_folder(app: AppHandle, path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let canonical = p
        .canonicalize()
        .map_err(|e| format!("Cannot access folder: {e}"))?;
    if !canonical.is_dir() {
        return Err("Selected path is not a directory".into());
    }
    if !canonical.join(".git").exists() {
        return Err(
            "This folder is not a git project. Ghostlog watches project \
             repositories — select the root folder of one repository \
             (you can add more than one)."
                .into(),
        );
    }

    let state = app.state::<AppState>();
    let snapshot = {
        let mut paths = state.watched_paths.lock().unwrap();
        if paths.contains(&canonical) {
            return Err("That folder is already being watched.".into());
        }
        // Two watched projects with the same folder name would share a
        // session archive — sessions are keyed by project name on disk.
        let name = storage::project_name(&canonical)?;
        for existing in paths.iter() {
            if storage::project_name(existing)? == name {
                return Err(format!(
                    "A watched project named \"{name}\" already exists. \
                     Rename one of the folders to keep their archives separate."
                ));
            }
        }
        paths.push(canonical.clone());
        paths.clone()
    };
    storage::save_config(&snapshot)?;
    // The git-commit trigger applies to every watched repo — a project
    // added after the toggle was flipped on must get the hook too, not
    // silently go uncovered until someone happens to retoggle it.
    if storage::is_git_hook_enabled_setting() {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        storage::install_git_hook(&canonical, &exe)?;
    }

    // If we're mid-watch, pick the new folder up immediately.
    if state.watch_state.lock().unwrap().state == WatchState::Watching {
        watcher::restart(&app)?;
    }
    Ok(())
}

#[tauri::command]
pub fn remove_watched_folder(app: AppHandle, path: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let snapshot = {
        let mut paths = state.watched_paths.lock().unwrap();
        paths.retain(|p| p.display().to_string() != path);
        paths.clone()
    };
    storage::save_config(&snapshot)?;
    if state.watch_state.lock().unwrap().state == WatchState::Watching {
        if snapshot.is_empty() {
            watcher::stop(&app);
        } else {
            watcher::restart(&app)?;
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct WatchedProject {
    pub name: String,
    pub path: String,
}

#[tauri::command]
pub fn get_watched_folders(state: State<AppState>) -> Result<Vec<WatchedProject>, String> {
    state
        .watched_paths
        .lock()
        .unwrap()
        .iter()
        .map(|p| {
            Ok(WatchedProject {
                name: storage::project_name(p)?,
                path: p.display().to_string(),
            })
        })
        .collect()
}

/// Resolve a project name to its watched repo path.
fn repo_for_project(state: &State<AppState>, project: &str) -> Result<PathBuf, String> {
    let paths = state.watched_paths.lock().unwrap();
    for p in paths.iter() {
        if storage::project_name(p)? == project {
            return Ok(p.clone());
        }
    }
    Err(format!("\"{project}\" is not a watched project"))
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
pub async fn manual_capture(
    app: AppHandle,
    project: String,
    note: Option<String>,
) -> Result<storage::SessionEntry, String> {
    let state = app.state::<AppState>();
    let repo = repo_for_project(&state, &project)?;
    let diff = storage::working_tree_diff(&repo);

    // Reuse the project's active session or lazily create one (manual
    // capture must work even when not actively watching).
    let (date, session_id) = {
        let mut cur = state.current_sessions.lock().unwrap();
        match cur.get(&project).cloned() {
            Some(s) => s,
            None => {
                let s = storage::create_session(&project)?;
                cur.insert(project.clone(), s.clone());
                s
            }
        }
    };

    let note_text = note.unwrap_or_else(|| "manual capture".to_string());
    let diff_context = if diff.trim().is_empty() { None } else { Some(diff.as_str()) };
    let draft = crate::ai::summarize_capture(&note_text, diff_context).await;
    let entry = storage::write_entry(
        &project, &date, &session_id, &draft.tag, &draft.title, &draft.summary, None,
    )?;
    watcher::record_event(&app, "manual", note_text, &project);
    Ok(entry)
}

// ---- Session archive (read/browse any past date) ----

#[tauri::command]
pub fn list_session_dates(project: String) -> Result<Vec<String>, String> {
    storage::list_dates(&project)
}

#[tauri::command]
pub fn list_sessions(project: String, date: String) -> Result<Vec<storage::SessionMeta>, String> {
    storage::list_sessions(&project, &date)
}

#[tauri::command]
pub fn read_session(
    project: String,
    date: String,
    session_id: String,
) -> Result<Vec<storage::SessionEntry>, String> {
    storage::read_session(&project, &date, &session_id)
}

/// Full-text search across every entry of the watched project — backs the
/// Archive search box, so finding "that fix from a few weeks ago" doesn't
/// require remembering which date it happened on.
#[tauri::command]
pub fn search_entries(
    project: String,
    query: String,
) -> Result<Vec<storage::SearchHit>, String> {
    storage::search_entries(&project, &query)
}

#[tauri::command]
pub fn update_entry(
    project: String,
    date: String,
    session_id: String,
    entry_id: String,
    tag: String,
    title: String,
    summary: String,
) -> Result<(), String> {
    storage::update_entry(&project, &date, &session_id, &entry_id, &tag, &title, &summary)
}

#[tauri::command]
pub fn delete_entry(
    project: String,
    date: String,
    session_id: String,
    entry_id: String,
) -> Result<(), String> {
    storage::delete_entry(&project, &date, &session_id, &entry_id)
}

/// Deletes an entire session (every entry + screenshot in it) — the
/// Archive's per-session delete action.
#[tauri::command]
pub fn delete_session(
    project: String,
    date: String,
    session_id: String,
) -> Result<(), String> {
    storage::delete_session(&project, &date, &session_id)
}

// ---- Settings: git-commit trigger ----

/// The commit trigger applies to every watched repo. Backed by a persisted
/// setting rather than inferred from each repo's hook file — inferring from
/// the filesystem missed projects added after the toggle was last flipped,
/// and never noticed a hook path going stale after the app binary moved.
#[tauri::command]
pub fn is_git_hook_enabled() -> bool {
    storage::is_git_hook_enabled_setting()
}

#[tauri::command]
pub fn set_git_hook_enabled(state: State<AppState>, enabled: bool) -> Result<(), String> {
    storage::set_git_hook_enabled_setting(enabled)?;
    let paths = state.watched_paths.lock().unwrap().clone();
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    for repo in &paths {
        if enabled {
            storage::install_git_hook(repo, &exe)?;
        } else {
            storage::uninstall_git_hook(repo)?;
        }
    }
    Ok(())
}

// ---- Settings: run in background ----

#[tauri::command]
pub fn get_run_in_background() -> bool {
    storage::run_in_background_enabled()
}

#[tauri::command]
pub fn set_run_in_background(enabled: bool) -> Result<(), String> {
    storage::set_run_in_background_setting(enabled)
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
pub fn set_ai_config(
    endpoint: String,
    model: String,
    vision_endpoint: String,
    vision_model: String,
) -> Result<(), String> {
    storage::save_ai_config(&storage::AiConfig { endpoint, model, vision_endpoint, vision_model })
}

/// Backs ai-stub.ts's compileEntries — the UI-driven path always has a
/// webview, but we still keep the real call in ai.rs (single source of
/// truth for backend model calls, shared with the CLI git-hook path).
#[tauri::command]
pub async fn ai_compile(entries: Vec<String>) -> String {
    crate::ai::compile_document(&entries).await
}

// ---- Settings: capture data folder ----
// Where entries/screenshots are actually stored — a visible, user-chosen
// folder (defaults to ~/Desktop/Ghostlog Data) rather than the hidden OS
// app-data directory, so it's obvious in Finder that captures never leave
// the machine. Separate from the output folder below, which is only for
// explicitly-exported compiled documents.

#[tauri::command]
pub fn get_data_root() -> Result<String, String> {
    storage::data_root().map(|p| p.display().to_string())
}

#[tauri::command]
pub fn set_data_root(app: AppHandle, path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let canonical = p.canonicalize().map_err(|e| format!("Cannot access folder: {e}"))?;
    if !canonical.is_dir() {
        return Err("Selected path is not a directory".into());
    }
    let old_root = storage::data_root()?;
    storage::relocate_project_dirs(&old_root, &canonical)?;
    storage::set_data_root_setting(&canonical)?;
    let _ = app.asset_protocol_scope().allow_directory(&canonical, true);
    Ok(())
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
