//! Shared app state: watch status, the single watched folder, last capture.
//!
//! Free tier watches exactly ONE folder. The path is held here and every
//! watcher/capture operation validates against it — path scoping is enforced
//! in Rust, never trusted to the UI.

use notify::RecommendedWatcher;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WatchState {
    Idle,
    Watching,
}

#[derive(Clone, Serialize)]
pub struct LastEvent {
    /// ISO 8601 timestamp.
    pub timestamp: String,
    /// e.g. "file-change", "manual", "git-commit".
    pub kind: String,
    /// Human-readable one-liner shown in the review window home view.
    pub detail: String,
}

#[derive(Default)]
pub struct AppState {
    pub watch_state: Mutex<WatchStateHolder>,
    pub watched_path: Mutex<Option<PathBuf>>,
    pub last_event: Mutex<Option<LastEvent>>,
}

pub struct WatchStateHolder {
    pub state: WatchState,
    /// Dropping the watcher stops it; kept here so Stop Watching works.
    pub watcher: Option<RecommendedWatcher>,
}

impl Default for WatchStateHolder {
    fn default() -> Self {
        Self { state: WatchState::Idle, watcher: None }
    }
}
