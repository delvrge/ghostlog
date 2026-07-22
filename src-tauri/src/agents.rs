//! Opt-in capture from local terminal AI-coding-tool session logs.
//!
//! Ghostlog only *watches*: it never talks to these tools. Some of them
//! happen to write their own session transcripts to plain local files —
//! when they do, we can tail those files for the human-typed prompts and
//! log them as `note` entries, the same "did something happen" bar as any
//! other capture. This is inherently fragile: these are undocumented,
//! internal formats owned by each tool and can change without notice.
//! Every parser here fails soft (empty result, never a panic or a lost
//! capture) and is scoped to exactly one project directory at a time.
//!
//! Two tools wired up so far: Claude Code (project-slugged folders under
//! `~/.claude/projects/`) and Codex CLI (date-bucketed folders under
//! `~/.codex/sessions/`, matched to a project by the `cwd` each session
//! records in its own first line). Different enough shapes that they don't
//! share a lookup path, but both funnel into the same `poll_generic`
//! cursor/dedup logic at the bottom of this file.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Claude Code slugs a project's absolute path into its `~/.claude/projects`
/// folder name by replacing every non-alphanumeric character with `-`
/// (e.g. `/Volumes/SSD/4_Code/GHLG` -> `-Volumes-SSD-4-Code-GHLG`).
fn claude_code_project_slug(root: &Path) -> String {
    root.display()
        .to_string()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn claude_code_projects_dir(root: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let slug = claude_code_project_slug(root);
    let dir = home.join(".claude/projects").join(slug);
    dir.is_dir().then_some(dir)
}

/// Most-recently-modified session transcript for this project, if any.
fn latest_session_file(root: &Path) -> Option<PathBuf> {
    let dir = claude_code_projects_dir(root)?;
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .max_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
}

/// Known synthetic/system-injected prefixes that show up as `type: "user"`
/// entries but were never typed by a human — continuation summaries,
/// local-command output wrappers, etc. Best-effort filter, not exhaustive.
const SYNTHETIC_PREFIXES: &[&str] = &[
    "This session is being continued",
    "<local-command-caveat>",
    "<command-name>",
    "<system-reminder>",
];

/// Pulls plain-text human prompts out of one JSONL transcript, starting
/// after `skip_bytes` (so re-polling the same growing file only returns
/// what's new). Returns the new end-of-file byte offset alongside the
/// prompts found, so the caller can persist it as the next cursor.
fn extract_new_prompts_from(raw: &str, skip_bytes: usize) -> (Vec<String>, u64) {
    let slice = &raw[skip_bytes.min(raw.len())..];
    let mut prompts = Vec::new();

    for line in slice.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if v.get("type").and_then(Value::as_str) != Some("user") {
            continue;
        }
        if v.get("isMeta").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        if v.get("isSidechain").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        // Real human turns store `message.content` as a plain string;
        // tool results and multi-part turns use an array instead — those
        // are not something the user typed, skip them.
        let Some(text) = v.get("message").and_then(|m| m.get("content")).and_then(Value::as_str)
        else {
            continue;
        };
        let text = text.trim();
        if text.is_empty() || SYNTHETIC_PREFIXES.iter().any(|p| text.starts_with(p)) {
            continue;
        }
        prompts.push(text.to_string());
    }

    (prompts, raw.len() as u64)
}

/// Reads (and advances) the persisted cursor for one project + source file,
/// then returns whatever new human prompts have shown up since.
/// `cursor_key` scopes the offset in config.json so multiple watched
/// projects don't stomp on each other's progress.
pub fn poll_claude_code(project: &str, root: &Path) -> Result<Vec<String>, String> {
    let Some(file) = latest_session_file(root) else { return Ok(vec![]) };
    poll_generic(project, "claudeCode", &file, |raw, skip| {
        extract_new_prompts_from(raw, skip)
    })
}

// ---- Codex CLI ----
//
// Unlike Claude Code, Codex doesn't slug the project path into a folder
// name — every session (any project) lands under
// ~/.codex/sessions/<year>/<month>/<day>/rollout-*.jsonl, keyed only by
// when it happened. Matching a session to a project means opening
// candidate files and checking their own recorded `cwd`. Only the first
// line of each file (the `session_meta` record) needs reading for that,
// so this stays cheap even with a lot of history.

fn codex_sessions_root() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".codex/sessions"))
}

/// Recursively collects `.jsonl` files under `dir`, bounded so a huge
/// session history can't turn a routine poll into a slow directory walk.
fn collect_jsonl_files(dir: &Path, out: &mut Vec<PathBuf>, limit: usize) {
    if out.len() >= limit {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.filter_map(|e| e.ok()) {
        if out.len() >= limit {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, out, limit);
        } else if path.extension().is_some_and(|x| x == "jsonl") {
            out.push(path);
        }
    }
}

/// The first line of a Codex rollout file is a `session_meta` record
/// carrying the working directory that session ran in.
fn codex_session_cwd(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let first_line = std::io::BufRead::lines(std::io::BufReader::new(file)).next()?.ok()?;
    let v: Value = serde_json::from_str(&first_line).ok()?;
    v.get("payload")?.get("cwd")?.as_str().map(str::to_string)
}

/// Most-recently-modified Codex session file whose recorded cwd matches
/// this project's watched root, if any.
fn latest_codex_session_file(root: &Path) -> Option<PathBuf> {
    let base = codex_sessions_root()?;
    let mut files = Vec::new();
    // 500 most-recently-touched files is generous headroom for "any
    // session from the last while" without scanning years of history on
    // every 20s poll.
    collect_jsonl_files(&base, &mut files, 500);
    let target = root.to_string_lossy().to_string();

    files
        .into_iter()
        .filter(|p| codex_session_cwd(p).as_deref() == Some(target.as_str()))
        .max_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
}

/// Codex marks genuine human input distinctly from everything else in the
/// transcript (tool calls, model output, injected environment context) via
/// `event_msg` records of kind `user_message` — no heuristic filtering
/// needed the way Claude Code's overloaded `"type": "user"` requires.
fn extract_new_codex_prompts(raw: &str, skip_bytes: usize) -> (Vec<String>, u64) {
    let slice = &raw[skip_bytes.min(raw.len())..];
    let mut prompts = Vec::new();

    for line in slice.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if v.get("type").and_then(Value::as_str) != Some("event_msg") {
            continue;
        }
        let payload = v.get("payload");
        if payload.and_then(|p| p.get("type")).and_then(Value::as_str) != Some("user_message") {
            continue;
        }
        let Some(text) = payload.and_then(|p| p.get("message")).and_then(Value::as_str) else {
            continue;
        };
        let text = text.trim();
        if !text.is_empty() {
            prompts.push(text.to_string());
        }
    }

    (prompts, raw.len() as u64)
}

pub fn poll_codex(project: &str, root: &Path) -> Result<Vec<String>, String> {
    let Some(file) = latest_codex_session_file(root) else { return Ok(vec![]) };
    poll_generic(project, "codex", &file, extract_new_codex_prompts)
}

/// Shared cursor bookkeeping for both sources: persists which file was
/// last read and how far into it, so re-polling only surfaces genuinely
/// new prompts and a session rollover restarts cleanly from the top of the
/// new file.
fn poll_generic(
    project: &str,
    source: &str,
    file: &Path,
    extract: impl Fn(&str, usize) -> (Vec<String>, u64),
) -> Result<Vec<String>, String> {
    let cursor_key = format!("{source}Cursor::{project}");
    let file_key = format!("{source}File::{project}");

    let last_file = crate::storage::read_config_string(&file_key);
    let last_offset = if last_file.as_deref() == Some(&*file.to_string_lossy()) {
        crate::storage::read_config_u64(&cursor_key).unwrap_or(0)
    } else {
        0
    };

    let Ok(raw) = fs::read_to_string(file) else { return Ok(vec![]) };
    let skip = if (raw.len() as u64) < last_offset { 0 } else { last_offset as usize };
    let (prompts, new_offset) = extract(&raw, skip);

    crate::storage::write_config_string(&file_key, &file.to_string_lossy())?;
    crate::storage::write_config_u64(&cursor_key, new_offset)?;
    Ok(prompts)
}
