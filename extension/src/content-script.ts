/**
 * GHLG content script — runs ONLY on http://localhost/* and http://127.0.0.1/*
 * (enforced by manifest host_permissions + content_scripts matches).
 *
 * Triggers:
 * - manual hotkey (Cmd/Ctrl+Shift+G), mirroring the desktop "Log this now"
 * - automatic error detection: uncaught JS errors and unhandled promise
 *   rejections on the page fire a capture with the error text as the note
 *   hint. Rate-limited, because a page stuck in an error loop can throw
 *   hundreds of times per second and each capture spawns a native-host
 *   subprocess + an AI call — over-capturing one entry per minute is plenty
 *   (Curate exists to discard noise, but not thousands of duplicates).
 *
 * Deliberately has NO import/export statements: MV3 content scripts (unlike
 * the background service worker) are always loaded as classic, non-module
 * scripts, and TypeScript synthesizes a trailing `export {};` for any file
 * with a type-only import once that import is erased — which is a syntax
 * error in a classic script context and silently kills the entire file.
 * Learned this the hard way: this file never ran at all, from step 7
 * onward, until this comment was added. Duplicate the tiny shape below
 * instead of importing it from protocol.ts.
 */
type CaptureTrigger = "hotkey" | "console-error";

const ERROR_CAPTURE_COOLDOWN_MS = 60_000;
let lastErrorCaptureAt = 0;

function sendCapture(trigger: CaptureTrigger, note?: string): void {
  const request = {
    kind: "capture",
    trigger,
    url: location.href,
    note,
  };
  chrome.runtime.sendMessage(request, () => {
    if (chrome.runtime.lastError) {
      console.log("[GHLG] capture failed:", chrome.runtime.lastError.message);
    }
  });
}

function sendErrorCapture(description: string): void {
  const now = Date.now();
  if (now - lastErrorCaptureAt < ERROR_CAPTURE_COOLDOWN_MS) return;
  lastErrorCaptureAt = now;
  sendCapture("console-error", `browser error on ${location.href}: ${description}`);
}

window.addEventListener("keydown", (e) => {
  const isHotkey = e.shiftKey && (e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "g";
  if (isHotkey) {
    e.preventDefault();
    sendCapture("hotkey");
  }
});

window.addEventListener("error", (e) => {
  const where = e.filename ? ` (${e.filename}:${e.lineno})` : "";
  sendErrorCapture(`${e.message}${where}`);
});

window.addEventListener("unhandledrejection", (e) => {
  sendErrorCapture(`unhandled promise rejection: ${String(e.reason)}`);
});
