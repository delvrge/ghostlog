//! Session storage — plain filesystem, no database.
//!
//! Layout (never inside the watched repo, so it can't be committed):
//!   <os-app-data>/GHLG/<project-name>/<YYYY-MM-DD>/session-NN/
//!     entry-001-bugfix.md
//!     screenshot-001.png
//!
//! Entries are markdown with a small front-matter block. Everything here is
//! pure local filesystem access; the review window reaches it only through
//! Tauri commands.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntry {
    pub id: String,
    pub timestamp: String,
    pub tag: String, // "bugfix" | "update" | "feature"
    pub title: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_path: Option<String>,
    pub markdown_path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub date: String,
    pub session_id: String,
    pub entry_count: usize,
}

/// OS-standard app-data root:
/// macOS ~/Library/Application Support/GHLG, Windows %APPDATA%/GHLG,
/// Linux ~/.local/share/ghlg.
pub fn app_data_root() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or("Cannot resolve OS app-data directory")?;
    let name = if cfg!(target_os = "linux") { "ghlg" } else { "GHLG" };
    Ok(base.join(name))
}

/// config.json in the app-data root persists the watched folder across
/// restarts (the single free-tier folder — nothing else is stored).
pub fn load_config(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Ok(root) = app_data_root() else { return };
    let Ok(raw) = fs::read_to_string(root.join("config.json")) else { return };
    let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&raw) else { return };
    if let Some(path) = cfg.get("watchedFolder").and_then(|v| v.as_str()) {
        let p = PathBuf::from(path);
        if p.is_dir() {
            let state = app.state::<crate::state::AppState>();
            *state.watched_path.lock().unwrap() = Some(p);
        }
    }
}

pub fn save_config(watched: &Path) -> Result<(), String> {
    let root = app_data_root()?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let cfg = serde_json::json!({ "watchedFolder": watched.display().to_string() });
    fs::write(root.join("config.json"), serde_json::to_string_pretty(&cfg).unwrap())
        .map_err(|e| e.to_string())
}

fn project_root(project: &str) -> Result<PathBuf, String> {
    Ok(app_data_root()?.join(project))
}

/// Project name = final component of the watched folder path.
pub fn project_name(watched: &Path) -> Result<String, String> {
    watched
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "Watched folder has no name".into())
}

fn is_date_dir(name: &str) -> bool {
    name.len() == 10 && chrono::NaiveDate::parse_from_str(name, "%Y-%m-%d").is_ok()
}

/// All dates that have at least one session, newest first.
/// This backs the full archive browser — any past date, not just today.
pub fn list_dates(project: &str) -> Result<Vec<String>, String> {
    let root = project_root(project)?;
    if !root.is_dir() {
        return Ok(vec![]);
    }
    let mut dates: Vec<String> = fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| is_date_dir(n))
        .collect();
    dates.sort();
    dates.reverse();
    Ok(dates)
}

pub fn list_sessions(project: &str, date: &str) -> Result<Vec<SessionMeta>, String> {
    let dir = project_root(project)?.join(date);
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut sessions: Vec<SessionMeta> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            if !id.starts_with("session-") {
                return None;
            }
            let entry_count = fs::read_dir(e.path())
                .map(|rd| {
                    rd.filter_map(|f| f.ok())
                        .filter(|f| {
                            f.file_name().to_string_lossy().starts_with("entry-")
                                && f.path().extension().is_some_and(|x| x == "md")
                        })
                        .count()
                })
                .unwrap_or(0);
            Some(SessionMeta { date: date.to_string(), session_id: id, entry_count })
        })
        .collect();
    sessions.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    Ok(sessions)
}

/// Create (or reuse) today's next session folder. Called when watching
/// starts; entries within one watch period share a session.
pub fn create_session(project: &str) -> Result<(String, String), String> {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let dir = project_root(project)?.join(&date);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let next = list_sessions(project, &date)?
        .iter()
        .filter_map(|s| s.session_id.strip_prefix("session-")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let session_id = format!("session-{next:02}");
    fs::create_dir_all(dir.join(&session_id)).map_err(|e| e.to_string())?;
    Ok((date, session_id))
}

fn session_dir(project: &str, date: &str, session_id: &str) -> Result<PathBuf, String> {
    // Reject path-traversal shaped inputs from the frontend.
    if !is_date_dir(date) || !session_id.starts_with("session-") || session_id.contains(['/', '\\'])
    {
        return Err("Invalid date or session id".into());
    }
    Ok(project_root(project)?.join(date).join(session_id))
}

pub fn write_entry(
    project: &str,
    date: &str,
    session_id: &str,
    tag: &str,
    title: &str,
    summary: &str,
) -> Result<SessionEntry, String> {
    let dir = session_dir(project, date, session_id)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let next = read_session(project, date, session_id)?
        .iter()
        .filter_map(|e| e.id.split('-').nth(1)?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let id = format!("entry-{next:03}-{tag}");
    let timestamp = chrono::Local::now().to_rfc3339();
    let path = dir.join(format!("{id}.md"));
    let content = format!(
        "---\nid: {id}\ntimestamp: {timestamp}\ntag: {tag}\ntitle: {title}\n---\n\n{summary}\n"
    );
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(SessionEntry {
        id,
        timestamp,
        tag: tag.to_string(),
        title: title.to_string(),
        summary: summary.to_string(),
        screenshot_path: None,
        markdown_path: path.display().to_string(),
    })
}

pub fn read_session(
    project: &str,
    date: &str,
    session_id: &str,
) -> Result<Vec<SessionEntry>, String> {
    let dir = session_dir(project, date, session_id)?;
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut entries: Vec<SessionEntry> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name().to_string_lossy().starts_with("entry-")
                && e.path().extension().is_some_and(|x| x == "md")
        })
        .filter_map(|e| parse_entry(&e.path()).ok())
        .collect();
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    // Attach screenshots referenced by front-matter or matching by number.
    Ok(entries)
}

fn parse_entry(path: &Path) -> Result<SessionEntry, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut id = String::new();
    let mut timestamp = String::new();
    let mut tag = String::new();
    let mut title = String::new();
    let mut screenshot: Option<String> = None;
    let mut body = String::new();

    let mut in_front = false;
    let mut front_done = false;
    for line in raw.lines() {
        if line.trim() == "---" {
            if !in_front && !front_done {
                in_front = true;
            } else if in_front {
                in_front = false;
                front_done = true;
            }
            continue;
        }
        if in_front {
            if let Some((k, v)) = line.split_once(':') {
                let v = v.trim();
                match k.trim() {
                    "id" => id = v.into(),
                    "timestamp" => timestamp = v.into(),
                    "tag" => tag = v.into(),
                    "title" => title = v.into(),
                    "screenshot" => screenshot = Some(v.into()),
                    _ => {}
                }
            }
        } else if front_done {
            body.push_str(line);
            body.push('\n');
        }
    }
    if id.is_empty() {
        id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
    }
    let screenshot_path =
        screenshot.map(|s| path.parent().unwrap_or(Path::new("")).join(s).display().to_string());
    Ok(SessionEntry {
        id,
        timestamp,
        tag,
        title,
        summary: body.trim().to_string(),
        screenshot_path,
        markdown_path: path.display().to_string(),
    })
}

/// Overwrite an entry's editable fields (tag, title, summary).
pub fn update_entry(
    project: &str,
    date: &str,
    session_id: &str,
    entry_id: &str,
    tag: &str,
    title: &str,
    summary: &str,
) -> Result<(), String> {
    let existing = find_entry(project, date, session_id, entry_id)?;
    let content = format!(
        "---\nid: {entry_id}\ntimestamp: {}\ntag: {tag}\ntitle: {title}\n---\n\n{summary}\n",
        existing.timestamp
    );
    fs::write(&existing.markdown_path, content).map_err(|e| e.to_string())
}

pub fn delete_entry(
    project: &str,
    date: &str,
    session_id: &str,
    entry_id: &str,
) -> Result<(), String> {
    let existing = find_entry(project, date, session_id, entry_id)?;
    fs::remove_file(&existing.markdown_path).map_err(|e| e.to_string())?;
    if let Some(shot) = existing.screenshot_path {
        let _ = fs::remove_file(shot); // screenshot may already be gone
    }
    Ok(())
}

// ---- Git commit hook ----
// The hook script shells out to the GHLG binary itself in a lightweight CLI
// mode (see main.rs), so a capture can be written even when the review
// window / tray process isn't running. Pure filesystem + git — no network.

const HOOK_MARKER: &str = "# ghlg-managed-hook";

fn hook_path(repo: &Path) -> PathBuf {
    repo.join(".git").join("hooks").join("post-commit")
}

pub fn is_git_hook_installed(repo: &Path) -> bool {
    fs::read_to_string(hook_path(repo)).is_ok_and(|s| s.contains(HOOK_MARKER))
}

pub fn install_git_hook(repo: &Path, exe_path: &Path) -> Result<(), String> {
    let path = hook_path(repo);
    let script = format!(
        "#!/bin/sh\n{HOOK_MARKER}\n\"{}\" --ghlg-git-commit \"$(pwd)\" &\n",
        exe_path.display()
    );
    fs::write(&path, script).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Only removes the hook if GHLG installed it — never touches a hook a
/// developer wrote themselves.
pub fn uninstall_git_hook(repo: &Path) -> Result<(), String> {
    if is_git_hook_installed(repo) {
        fs::remove_file(hook_path(repo)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Entry point for `ghlg --ghlg-git-commit <repo>`, run by the post-commit
/// hook as a short-lived subprocess. Reads the latest commit subject via
/// `git log` and writes it as an entry — no app instance needs to be
/// running for git-commit capture to work.
/// STUB: the summary is placeholder text; real diff summarization routes
/// through ai-stub.ts (Ollama) once wired into the review window flow.
pub fn capture_from_git_commit(repo: &Path) -> Result<(), String> {
    let project = project_name(repo)?;
    let subject = std::process::Command::new("git")
        .args(["log", "-1", "--pretty=%s"])
        .current_dir(repo)
        .output()
        .map_err(|e| e.to_string())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())?;
    let title = if subject.is_empty() { "git commit".to_string() } else { subject };

    let (date, session_id) = create_session(&project)?;
    write_entry(
        &project,
        &date,
        &session_id,
        "update",
        &title,
        "Captured from git commit hook. Placeholder summary — will be replaced \
         by a local-model diff summary once ai-stub.ts is wired to a real model.",
    )?;
    Ok(())
}

fn find_entry(
    project: &str,
    date: &str,
    session_id: &str,
    entry_id: &str,
) -> Result<SessionEntry, String> {
    read_session(project, date, session_id)?
        .into_iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| format!("Entry not found: {entry_id}"))
}
