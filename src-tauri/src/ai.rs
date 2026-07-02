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
        summary: "Mock summary — set an endpoint in Settings > AI provider (a running \
                   llama.cpp server) to replace this with a real local-model summary."
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

/// Summarize a manual note or commit subject (plus optional git context)
/// into a structured entry draft. Falls back to a mock draft on any error
/// or when no endpoint is configured — capture always succeeds.
pub async fn summarize_capture(note: &str, git_context: Option<&str>) -> EntryDraft {
    let cfg = crate::storage::load_ai_config();
    if cfg.endpoint.trim().is_empty() {
        return mock_draft(note);
    }

    let system = "You are labeling a developer's dev-log entry. Reply with ONLY a JSON \
                  object with exactly these keys: \"tag\" (one of \"bugfix\", \"update\", \
                  \"feature\"), \"title\" (a short one-line title, under 60 characters), \
                  and \"summary\" (one or two plain sentences). No other text.";
    let user = format!(
        "Note from the developer: {note}\n{}",
        git_context.map(|c| format!("Recent git context:\n{c}")).unwrap_or_default()
    );

    match call_llama_cpp(&cfg, system, &user, true).await {
        Ok(raw) => parse_draft(&raw).unwrap_or_else(|| fallback_with_note(note, &raw)),
        Err(e) => fallback_with_error(note, &e),
    }
}

#[derive(Deserialize)]
struct RawDraft {
    tag: String,
    title: String,
    summary: String,
}

fn parse_draft(raw: &str) -> Option<EntryDraft> {
    let d: RawDraft = serde_json::from_str(raw.trim()).ok()?;
    let tag = match d.tag.as_str() {
        "bugfix" | "feature" => d.tag,
        _ => "update".to_string(),
    };
    Some(EntryDraft { tag, title: d.title, summary: d.summary })
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

    let system = "Write a short, plain-language postmortem in markdown from the dev-log \
                  entries the user gives you. Use a '# Session postmortem' heading and a \
                  few short sections. Do not invent details not present in the entries.";

    match call_llama_cpp(&cfg, system, &joined, false).await {
        Ok(text) => text,
        Err(e) => format!(
            "# Session postmortem\n\n_Local model call failed ({e}) — showing raw entries \
             instead._\n\n{joined}"
        ),
    }
}
