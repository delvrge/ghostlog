/**
 * ai-stub.ts — the ONLY place AI calls live.
 *
 * STUB: every function here returns realistic mock data. Replace with real
 * local model calls (Ollama, or any endpoint the user configures in
 * Settings → AI Provider, read via get_ai_config) once a model is selected.
 * Swapping in the real implementation must touch this file only — nothing
 * else in the app may talk to a model directly.
 *
 * Settings already lets the user store an endpoint + model (free tier:
 * empty by default, bring-your-own; Pro: presets). That config is inert
 * until this file is wired to actually call it — storing it is not the
 * same as using it.
 */

export interface EntryDraft {
  /** e.g. "bugfix" | "update" | "feature" — auto-tag guess. */
  tag: "bugfix" | "update" | "feature";
  title: string;
  summary: string;
}

/** STUB: replace with real local model call (Ollama) once model is selected. */
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

/** STUB: replace with real local model call (Ollama) once model is selected. */
export async function summarizeScreenshot(pngPath: string): Promise<EntryDraft> {
  void pngPath;
  return {
    tag: "update",
    title: "UI state captured on localhost",
    summary: "Mock summary: screenshot shows the dashboard-free review UI mid-session.",
  };
}

/** STUB: replace with real local model call (Ollama) once model is selected. */
export async function compileEntries(entryMarkdown: string[]): Promise<string> {
  return [
    "# Session postmortem (mock)",
    "",
    `Compiled ${entryMarkdown.length} entries.`,
    "",
    "## What happened",
    "Mock compiled narrative goes here once a local model is wired in.",
  ].join("\n");
}
