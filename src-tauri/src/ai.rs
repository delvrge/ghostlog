//! The single place real local-model calls live on the backend.
//!
//! This exists alongside src/lib/ai-stub.ts on the frontend for one reason:
//! two capture paths never have a webview to call into —
//! the git post-commit hook (a bare CLI subprocess, see main.rs) and the
//! manual-capture command — so their AI calls must happen here in Rust,
//! not in TypeScript. The Compile view (UI-driven, always has a webview)
//! still goes through ai-stub.ts, which calls the same idea via the
//! `ai_compile` command below. Swapping providers means editing this file
//! (and ai-stub.ts) — nothing else in the app should need to change.
//!
//! Talks to a local llama.cpp server (`llama-server`), which speaks an
//! OpenAI-compatible chat API — NOT Ollama. Endpoint/model are whatever the
//! user configured in Settings > AI provider (default: none, e.g.
//! http://localhost:8080 once llama-server is running with a Qwen2.5-3B
//! GGUF model loaded). If no endpoint is set, or the call fails for any
//! reason, callers fall back to clearly-labeled mock text — a capture must
//! never be lost just because the model is unavailable.

use crate::storage::AiConfig;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

pub struct EntryDraft {
    pub tag: String,
    pub title: String,
    pub summary: String,
}

fn mock_draft(seed: &str) -> EntryDraft {
    EntryDraft {
        tag: "update".into(),
        title: seed.chars().take(60).collect(),
        summary: "**Problem:** (mock)\n\n**Fix:** (mock)\n\n**Reasoning:** set an endpoint in \
                   Settings > AI provider (a running llama.cpp server) to replace this with \
                   a real reconstruction of what changed and why."
            .into(),
    }
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

/// Calls a local llama.cpp server's OpenAI-compatible
/// `/v1/chat/completions` endpoint. `json_mode` requests a JSON object back
/// (`response_format: {type: "json_object"}`) for the structured entry
/// drafts; the compile flow asks for plain markdown instead.
async fn call_llama_cpp(cfg: &AiConfig, system: &str, user: &str, json_mode: bool) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(|e| e.to_string())?;

    let mut body = json!({
        "model": cfg.model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
        "temperature": 0.2,
    });
    if json_mode {
        body["response_format"] = json!({ "type": "json_object" });
    }

    let url = format!("{}/v1/chat/completions", cfg.endpoint.trim_end_matches('/'));
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("could not reach local model server: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("local model server returned {}", resp.status()));
    }

    let parsed: ChatCompletionResponse = resp.json().await.map_err(|e| e.to_string())?;
    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "local model server returned no choices".to_string())
}

/// Reconstructs a documentation entry from a git diff (plus an optional
/// short hint from the developer) into a structured entry draft.
///
/// The point of Ghostlog is NOT to store whatever one-liner the developer
/// typed — that's just a nudge for the model, not the documentation. The
/// diff is the real evidence: what files/lines actually changed. The model
/// is asked to reconstruct what the problem was, what the fix/change was,
/// and the likely reasoning behind it — the way a programmer would explain
/// it to themselves later, or in a postmortem. Falls back to a mock draft
/// on any error or when no endpoint is configured — capture always
/// succeeds either way.
pub async fn summarize_capture(hint: &str, diff: Option<&str>) -> EntryDraft {
    let cfg = crate::storage::load_ai_config();
    if cfg.endpoint.trim().is_empty() {
        return mock_draft(hint);
    }

    let system = "You are a documentation assistant for a solo developer. Given a code \
                  diff (and sometimes a short note from the developer), reconstruct what \
                  actually happened — do not just repeat the note verbatim, the diff is \
                  the real evidence. NEVER invent file names, function names, or specifics \
                  that are not visible in the diff or note below — if there is no diff and \
                  the note is thin or missing, say plainly that there isn't enough \
                  information, do not guess or fabricate a plausible-sounding story. Reply \
                  with ONLY a JSON object with exactly these keys:\n\
                  \"tag\": one of \"bugfix\", \"update\", \"feature\".\n\
                  \"title\": a short one-line title, under 60 characters.\n\
                  \"summary\": a markdown string with these sections, each 1-3 sentences, \
                  omitting a section only if it genuinely doesn't apply:\n\
                  \"**Problem:** what was broken or missing, inferred ONLY from the diff.\n\
                  \"**Fix:** what the diff actually changed to address it.\n\
                  \"**Reasoning:** the likely thought process behind that specific fix.\n\
                  \"**Suggestion:** (optional) one concrete follow-up idea, only if genuinely useful.\n\
                  No text outside the JSON object.";
    let user = format!(
        "Developer's note (a hint, not the full story): {}\n\n{}",
        if hint.trim().is_empty() { "(none given)" } else { hint },
        diff.map(|d| format!("Diff:\n```diff\n{d}\n```"))
            .unwrap_or_else(|| "No diff available — there is no code change to reason about. \
                Base the summary only on the note above; if the note is also thin, say so \
                plainly instead of inventing detail.".to_string())
    );

    match call_llama_cpp(&cfg, system, &user, true).await {
        Ok(raw) => parse_draft(&raw).unwrap_or_else(|| fallback_with_note(hint, &raw)),
        Err(e) => fallback_with_error(hint, &e),
    }
}

#[derive(Deserialize)]
struct RawDraft {
    tag: String,
    title: String,
    summary: String,
}

/// Small local models rarely produce perfectly strict JSON even when asked
/// (markdown code fences, YAML-style block scalars for multi-line string
/// values, etc). Strip the obvious wrapping first, try strict parsing, and
/// fall back to pulling the three fields out with plain string search
/// rather than discarding a perfectly good reconstruction over a syntax
/// slip.
fn parse_draft(raw: &str) -> Option<EntryDraft> {
    let cleaned = strip_code_fence(raw.trim());

    if let Ok(d) = serde_json::from_str::<RawDraft>(&cleaned) {
        return Some(normalize(d));
    }

    heuristic_extract(&cleaned)
}

fn strip_code_fence(s: &str) -> String {
    let s = s.trim();
    let Some(rest) = s.strip_prefix("```") else { return s.to_string() };
    let rest = rest.strip_prefix("json").unwrap_or(rest);
    rest.trim_start_matches('\n').trim_end().trim_end_matches("```").trim().to_string()
}

fn normalize(d: RawDraft) -> EntryDraft {
    let tag = match d.tag.as_str() {
        "bugfix" | "feature" => d.tag,
        _ => "update".to_string(),
    };
    EntryDraft { tag, title: d.title, summary: d.summary }
}

/// Pulls tag/title/summary out of a near-JSON reply that failed strict
/// parsing — e.g. a summary value written as a YAML block scalar (`>`)
/// instead of a quoted JSON string. Only used when serde_json gives up.
fn heuristic_extract(text: &str) -> Option<EntryDraft> {
    let tag = extract_quoted_value(text, "tag")
        .filter(|t| ["bugfix", "update", "feature"].contains(&t.as_str()))
        .unwrap_or_else(|| "update".to_string());
    let title = extract_quoted_value(text, "title")?;

    let summary_start = text.find("\"summary\"")? + "\"summary\"".len();
    let after_colon = text[summary_start..].trim_start().strip_prefix(':')?.trim_start();
    // Whatever follows (quoted string, YAML `>` block, or bare text) up to
    // the closing brace is the summary content — clean it up as markdown.
    let body = after_colon.trim_end_matches(['}', '`']).trim();
    let body = body.strip_prefix(['>', '|']).unwrap_or(body);
    let body = body.trim().trim_matches('"');
    let summary = body
        .lines()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if summary.is_empty() {
        return None;
    }

    Some(EntryDraft { tag, title, summary })
}

fn extract_quoted_value(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = text.find(&needle)? + needle.len();
    let rest = text[start..].trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn fallback_with_note(note: &str, raw: &str) -> EntryDraft {
    EntryDraft {
        tag: "update".into(),
        title: note.chars().take(60).collect(),
        summary: format!("Model reply could not be parsed as JSON; raw reply: {raw}"),
    }
}

fn fallback_with_error(note: &str, err: &str) -> EntryDraft {
    EntryDraft {
        tag: "update".into(),
        title: note.chars().take(60).collect(),
        summary: format!("Local model call failed ({err}); captured the note as-is."),
    }
}

/// Compile a batch of entry markdown into a single document. Falls back to
/// a simple mock document if no endpoint is configured or the call fails.
pub async fn compile_document(entry_markdown: &[String]) -> String {
    let cfg = crate::storage::load_ai_config();
    let joined = entry_markdown.join("\n\n");

    if cfg.endpoint.trim().is_empty() {
        return format!(
            "# Session postmortem (mock)\n\nCompiled {} entries.\n\n## What happened\n\
             Mock compiled narrative — set an endpoint in Settings > AI provider for a \
             real summary.",
            entry_markdown.len()
        );
    }

    let system = "Write a plain-language postmortem in markdown from the dev-log entries \
                  the user gives you (each already has a problem/fix/reasoning breakdown). \
                  Use a '# Session postmortem' heading, one subsection per entry, and a \
                  short closing 'What I'd do differently' section only if the entries \
                  actually suggest something. Do not invent details not present in the \
                  entries below.";

    match call_llama_cpp(&cfg, system, &joined, false).await {
        Ok(text) => text,
        Err(e) => format!(
            "# Session postmortem\n\n_Local model call failed ({e}) — showing raw entries \
             instead._\n\n{joined}"
        ),
    }
}
