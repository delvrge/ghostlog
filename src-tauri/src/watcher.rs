//! File/git watcher, scoped strictly to the single user-selected folder.
//!
//! Pure filesystem access — no network anywhere. Capture logic is mock at
//! this stage: events update `last_event` and are emitted to the frontend;
//! real entry-writing lands with the data layer (delivery step 3).

use crate::state::{AppState, LastEvent, WatchState};
use notify::{RecursiveMode, Watcher};
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};

/// Directories inside the watched folder we never react to.
const IGNORED: &[&str] = &["node_modules", "target", ".git/objects", "dist", ".next"];

fn is_ignored(path: &Path) -> bool {
    let s = path.to_string_lossy();
    IGNORED.iter().any(|seg| s.contains(seg))
}

pub fn record_event(app: &AppHandle, kind: &str, detail: String) {
    let state = app.state::<AppState>();
    let event = LastEvent {
        timestamp: chrono::Local::now().to_rfc3339(),
        kind: kind.to_string(),
        detail,
    };
    *state.last_event.lock().unwrap() = Some(event.clone());
    // Review window (if open) updates live; ignore failure when no window.
    let _ = app.emit("ghlg://capture", &event);
}

/// Start watching the configured folder. Fails if no folder is set —
/// there is deliberately no way to watch an arbitrary path.
pub fn start(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();

    let root = state
        .watched_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No watched folder configured. Complete onboarding first.")?;
    if !root.is_dir() {
        return Err(format!("Watched folder no longer exists: {}", root.display()));
    }

    let mut holder = state.watch_state.lock().unwrap();
    if holder.state == WatchState::Watching {
        return Ok(());
    }

    let app_for_events = app.clone();
    let scope_root = root.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            // Defense in depth: drop anything outside the watched root even
            // though the watcher is only registered on that root.
            let relevant: Vec<_> = event
                .paths
                .iter()
                .filter(|p| p.starts_with(&scope_root) && !is_ignored(p))
                .collect();
            if let Some(path) = relevant.first() {
                let rel = path.strip_prefix(&scope_root).unwrap_or(path);
                // MOCK CAPTURE: real entry-writing (git diff context, session
                // files) lands in delivery step 3.
                record_event(
                    &app_for_events,
                    "file-change",
                    format!("changed: {}", rel.display()),
                );
            }
        }
    })
    .map_err(|e| e.to_string())?;

    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;

    holder.state = WatchState::Watching;
    holder.watcher = Some(watcher);
    drop(holder);

    crate::tray::sync(app);
    Ok(())
}

pub fn stop(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut holder = state.watch_state.lock().unwrap();
    holder.watcher = None; // dropping stops the watcher
    holder.state = WatchState::Idle;
    drop(holder);
    crate::tray::sync(app);
}
