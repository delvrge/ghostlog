/**
 * ai-stub.ts — the ONLY frontend place AI calls live.
 *
 * compileEntries is wired to a real local model: it calls the `ai_compile`
 * Tauri command, which talks to a local llama.cpp server (`llama-server`,
 * OpenAI-compatible API) at the endpoint configured in Settings > AI
 * provider. If no endpoint is set, or the call fails, the Rust side
 * returns a clearly-labeled mock document instead — this file never needs
 * its own fallback logic.
 *
 * summarizeDiff/summarizeScreenshot below are still pure stubs: nothing in
 * the frontend calls them yet. Manual capture, git-commit capture, and
 * error-event screenshot analysis (ai.rs::describe_screenshot, fed by the
 * browser extension via the native-host path) all run entirely in Rust via
 * src-tauri/src/ai.rs, since those paths have no webview at all. They're
 * kept here as the frontend swap point for whenever a UI-driven flow needs
 * its own diff/screenshot summary.
 */
import { invoke } from "@tauri-apps/api/core";

export interface EntryDraft {
  /** Auto-tag guess — see SessionEntry["tag"] in src/lib/session.ts for the full set. */
  tag: import("./session").SessionEntry["tag"];
  title: string;
  summary: string;
}

/** STUB: replace with a real call (see src-tauri/src/ai.rs) once a UI flow needs it. */
export async function summarizeDiff(diff: string): Promise<EntryDraft> {
  void diff;
  return {
    tag: "bugfix",
    title: "Fixed webhook signature validation",
    summary:
      "Mock summary: the staged diff touches webhook handling; signature " +
      "check was comparing against the raw body after JSON parsing.",
  };
}

/** STUB: replace with a real call (see src-tauri/src/ai.rs) once a UI flow needs it. */
export async function summarizeScreenshot(pngPath: string): Promise<EntryDraft> {
  void pngPath;
  return {
    tag: "update",
    title: "UI state captured on localhost",
    summary: "Mock summary: screenshot shows the dashboard-free review UI mid-session.",
  };
}

/** Real: routes to the local model server via src-tauri/src/ai.rs (mock fallback lives there). */
export async function compileEntries(entryMarkdown: string[]): Promise<string> {
  return invoke("ai_compile", { entries: entryMarkdown });
}
