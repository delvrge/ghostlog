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

/// config.json in the app-data root persists small local settings across
/// restarts: the single watched folder (free tier) and, optionally, the
/// user's own AI endpoint/model. Nothing is sent anywhere by this file
/// itself — it's just local key-value storage.
fn config_path() -> Result<PathBuf, String> {
    Ok(app_data_root()?.join("config.json"))
}

fn read_config() -> serde_json::Value {
    config_path()
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

fn write_config(cfg: &serde_json::Value) -> Result<(), String> {
    let root = app_data_root()?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    fs::write(config_path()?, serde_json::to_string_pretty(cfg).unwrap()).map_err(|e| e.to_string())
}

pub fn load_config(app: &tauri::AppHandle) {
    use tauri::Manager;
    let cfg = read_config();
    if let Some(path) = cfg.get("watchedFolder").and_then(|v| v.as_str()) {
        let p = PathBuf::from(path);
        if p.is_dir() {
            let state = app.state::<crate::state::AppState>();
            *state.watched_path.lock().unwrap() = Some(p);
        }
    }
}

pub fn save_config(watched: &Path) -> Result<(), String> {
    let mut cfg = read_config();
    cfg["watchedFolder"] = serde_json::json!(watched.display().to_string());
    write_config(&cfg)
}

/// Reads the persisted watched folder directly from config.json — used by
/// the native-host CLI subprocess (see below), which has no AppState/Tauri
/// runtime to read it from.
pub fn load_watched_folder() -> Option<PathBuf> {
    read_config()
        .get("watchedFolder")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

#[derive(Clone, Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    /// e.g. "http://localhost:11434" — any local/self-hosted OpenAI- or
    /// Ollama-compatible endpoint. Empty means: keep using ai-stub mocks.
    pub endpoint: String,
    /// e.g. "llama3.2" — model name as the endpoint expects it.
    pub model: String,
}

pub fn load_ai_config() -> AiConfig {
    read_config()
        .get("ai")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

pub fn save_ai_config(ai: &AiConfig) -> Result<(), String> {
    let mut cfg = read_config();
    cfg["ai"] = serde_json::to_value(ai).map_err(|e| e.to_string())?;
    write_config(&cfg)
}

// ---- Export ----
// The output folder is the ONLY place outside the app-data directory (and
// the watched repo's own .git/hooks, for the commit hook) that Ghostlog is
// allowed to write to — and only when the user explicitly exports a
// document, never automatically.

pub fn load_output_folder() -> Option<PathBuf> {
    read_config()
        .get("outputFolder")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

pub fn save_output_folder(path: &Path) -> Result<(), String> {
    let mut cfg = read_config();
    cfg["outputFolder"] = serde_json::json!(path.display().to_string());
    write_config(&cfg)
}

/// Writes `content` under the configured output folder. `filename` is
/// sanitized to a bare name — no path separators or traversal — so an
/// export can never land outside the folder the user chose.
pub fn export_document(filename: &str, content: &str) -> Result<String, String> {
    let folder = load_output_folder().ok_or("No output folder configured (Settings > Output folder)")?;
    let safe_name: String = filename
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':') { '-' } else { c })
        .collect();
    let safe_name = safe_name.trim_start_matches('.').to_string();
    if safe_name.is_empty() {
        return Err("Invalid export filename".into());
    }
    let path = folder.join(safe_name);
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
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

/// Cap on diff text handed to the local model — keeps a small-context
/// model (e.g. a 4k-context Qwen2.5-3B) from being flooded by a huge diff.
const MAX_DIFF_CHARS: usize = 6000;

fn truncate_diff(diff: String) -> String {
    if diff.len() <= MAX_DIFF_CHARS {
        diff
    } else {
        format!("{}\n… (diff truncated)", &diff[..MAX_DIFF_CHARS])
    }
}

/// The actual code change is the point — Ghostlog documents what happened
/// and why, not just whatever one-liner the developer typed while rushing
/// back to work. This is the diff of everything not yet committed
/// (staged + unstaged) in the watched repo, used as reasoning material for
/// the manual "Log this now" trigger.
pub fn working_tree_diff(repo: &Path) -> String {
    truncate_diff(run_git(repo, &["diff", "HEAD"]).unwrap_or_default())
}

/// Entry point for `ghlg --ghlg-git-commit <repo>`, run by the post-commit
/// hook as a short-lived subprocess — there is no webview here, so the AI
/// call happens directly through ai.rs rather than ai-stub.ts. Reads the
/// latest commit's full diff and summarizes it with the local model
/// configured in Settings > AI provider (falls back to a mock draft if
/// none is configured or the call fails).
pub async fn capture_from_git_commit(repo: &Path) -> Result<(), String> {
    let project = project_name(repo)?;
    let subject = run_git(repo, &["log", "-1", "--pretty=%s"])?;
    let diff = truncate_diff(run_git(repo, &["show", "--pretty=", "HEAD"]).unwrap_or_default());
    let note = if subject.is_empty() { "git commit".to_string() } else { subject };
    // An empty diff (e.g. --allow-empty, or a merge with nothing to show)
    // must not be handed to the model as if there were real content to
    // reason about — see ai.rs for why that matters.
    let diff_context = if diff.trim().is_empty() { None } else { Some(diff.as_str()) };

    let draft = crate::ai::summarize_capture(&note, diff_context).await;
    let (date, session_id) = create_session(&project)?;
    write_entry(&project, &date, &session_id, &draft.tag, &draft.title, &draft.summary)?;
    Ok(())
}

/// Entry point for `ghlg --ghlg-native-host`, launched by Chrome itself as a
/// short-lived stdio subprocess per `chrome.runtime.connectNative` call (see
/// main.rs for the protocol loop). No repo argument is available — Chrome
/// only passes its own extension origin — so the watched folder comes from
/// the same config.json the review window itself uses.
pub async fn capture_from_native_host(note: Option<String>) -> Result<(), String> {
    let repo = load_watched_folder().ok_or("No watched folder configured")?;
    let project = project_name(&repo)?;
    let diff = working_tree_diff(&repo);
    let note_text = note.unwrap_or_else(|| "browser capture".to_string());
    let diff_context = if diff.trim().is_empty() { None } else { Some(diff.as_str()) };

    let draft = crate::ai::summarize_capture(&note_text, diff_context).await;
    let (date, session_id) = create_session(&project)?;
    write_entry(&project, &date, &session_id, &draft.tag, &draft.title, &draft.summary)?;
    Ok(())
}

// ---- Native Messaging host registration ----
// Chrome (and Chromium-based browsers) locate a native messaging host by a
// small JSON manifest file in a fixed, browser-specific directory — NOT by
// anything the extension itself can specify at runtime. Registering here
// just writes that manifest; it does not open any port or start any process
// (Chrome launches the host on demand, once per connectNative() call).

const NATIVE_HOST_NAME: &str = "com.ghostlog.native";
// Fixed via extension/manifest.json's "key" field, so the extension's ID
// never changes across reloads — otherwise every unpacked reload would
// require re-registering the host with a new allowed_origins entry.
const EXTENSION_ID: &str = "gmlnlhknokpiignefikdlpilogkfcldn";

#[cfg(target_os = "macos")]
fn native_host_manifest_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot resolve home directory")?;
    Ok(home
        .join("Library/Application Support/Google/Chrome/NativeMessagingHosts")
        .join(format!("{NATIVE_HOST_NAME}.json")))
}

#[cfg(target_os = "linux")]
fn native_host_manifest_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot resolve home directory")?;
    Ok(home
        .join(".config/google-chrome/NativeMessagingHosts")
        .join(format!("{NATIVE_HOST_NAME}.json")))
}

#[cfg(target_os = "windows")]
fn native_host_manifest_path() -> Result<PathBuf, String> {
    // Windows resolves native messaging hosts via a registry key rather than
    // a fixed directory; out of scope until Windows packaging is tackled.
    Err("Native Messaging host registration isn't implemented for Windows yet".to_string())
}

pub fn is_native_host_installed() -> bool {
    native_host_manifest_path().is_ok_and(|p| p.is_file())
}

pub fn install_native_host(exe_path: &Path) -> Result<(), String> {
    let path = native_host_manifest_path()?;
    let dir = path.parent().ok_or("Invalid native host manifest path")?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let manifest = serde_json::json!({
        "name": NATIVE_HOST_NAME,
        "description": "Ghostlog native messaging host",
        "path": exe_path.display().to_string(),
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{EXTENSION_ID}/")]
    });
    fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).map_err(|e| e.to_string())
}

pub fn uninstall_native_host() -> Result<(), String> {
    let path = native_host_manifest_path()?;
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---- Shell error trigger ----
// A "guaranteed" auto-capture trigger that doesn't depend on remembering to
// type a wrapper command: a small marked block in the user's shell startup
// file (~/.zshrc) runs on EVERY command, regardless of whether a human or
// an AI coding tool typed it. On a nonzero exit code it shells out to the
// GHLG binary in the background, same short-lived-subprocess pattern as the
// git post-commit hook. v1 signal is just "the last command failed" (exit
// code != 0) — no output-pattern scanning, which would be far more fragile.
// Over-capturing is fine: the Curate screen already exists for discarding
// low-value entries.

const SHELL_HOOK_MARKER: &str = "# ghlg-managed-shell-hook";

fn shell_rc_path() -> Result<PathBuf, String> {
    Ok(dirs::home_dir().ok_or("Cannot resolve home directory")?.join(".zshrc"))
}

pub fn is_shell_hook_installed() -> bool {
    shell_rc_path()
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .is_some_and(|s| s.contains(SHELL_HOOK_MARKER))
}

pub fn install_shell_hook(exe_path: &Path) -> Result<(), String> {
    if is_shell_hook_installed() {
        return Ok(());
    }
    let path = shell_rc_path()?;
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let exe = exe_path.display();
    let block = format!(
        "\n{SHELL_HOOK_MARKER}\n\
ghlg_precmd() {{\n\
  local exit_code=$?\n\
  if [ $exit_code -ne 0 ]; then\n\
    \"{exe}\" --ghlg-shell-error \"$(fc -ln -1)\" \"$exit_code\" &>/dev/null &\n\
  fi\n\
}}\n\
autoload -Uz add-zsh-hook\n\
add-zsh-hook precmd ghlg_precmd\n\
{SHELL_HOOK_MARKER}\n"
    );
    fs::write(&path, existing + &block).map_err(|e| e.to_string())
}

/// Only removes the block GHLG installed (between its two matching marker
/// lines) — never touches anything else the user has in their shell config.
pub fn uninstall_shell_hook() -> Result<(), String> {
    let path = shell_rc_path()?;
    let Ok(content) = fs::read_to_string(&path) else { return Ok(()) };
    let markers: Vec<usize> = content
        .match_indices(SHELL_HOOK_MARKER)
        .map(|(i, _)| i)
        .collect();
    if markers.len() < 2 {
        return Ok(());
    }
    // Trim the exact leading/trailing newline install() added around the
    // block, so uninstall restores the file byte-for-byte instead of
    // leaving stray blank lines behind.
    let mut start = markers[0];
    if start > 0 && content.as_bytes()[start - 1] == b'\n' {
        start -= 1;
    }
    let mut end = markers[1] + SHELL_HOOK_MARKER.len();
    if content.as_bytes().get(end) == Some(&b'\n') {
        end += 1;
    }
    let mut cleaned = content[..start].to_string();
    cleaned.push_str(&content[end..]);
    fs::write(&path, cleaned).map_err(|e| e.to_string())
}

/// Entry point for `ghlg --ghlg-shell-error <command> <exit_code>`, invoked
/// by the shell hook above as a short-lived subprocess. Same reasoning as
/// `capture_from_native_host`: no repo argument available, so the watched
/// folder comes from the persisted config.
pub async fn capture_from_shell_error(command: &str, exit_code: &str) -> Result<(), String> {
    let repo = load_watched_folder().ok_or("No watched folder configured")?;
    let project = project_name(&repo)?;
    let diff = working_tree_diff(&repo);
    let note = format!("shell command failed (exit {exit_code}): {command}");
    let diff_context = if diff.trim().is_empty() { None } else { Some(diff.as_str()) };

    let draft = crate::ai::summarize_capture(&note, diff_context).await;
    let (date, session_id) = create_session(&project)?;
    write_entry(&project, &date, &session_id, &draft.tag, &draft.title, &draft.summary)?;
    Ok(())
}

fn run_git(repo: &Path, args: &[&str]) -> Result<String, String> {
    std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| e.to_string())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
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
