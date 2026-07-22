/**
 * Compile view: turn captured entries into a document, and optionally
 * export it as a real file — the only place the user chooses where
 * Ghostlog writes to (Settings > Output folder).
 * Scopes: this session / this day / this week (the 7 days ending on the
 * session's date).
 */
import { useState } from "react";
import ReactMarkdown from "react-markdown";
import { invoke } from "@tauri-apps/api/core";
import { compileEntries } from "../lib/ai-stub";
import { readSession, listSessions, listDates } from "../lib/session";

type Scope = "session" | "day" | "week";

/**
 * The 7 calendar days ending on `date`, as an inclusive YYYY-MM-DD lower
 * bound. All-UTC arithmetic so no timezone can shift the day.
 */
function weekStart(date: string): string {
  const d = new Date(`${date}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() - 6);
  return d.toISOString().slice(0, 10);
}

export default function Compile({
  project,
  date,
  sessionId,
  onBack,
}: {
  project: string;
  date: string;
  sessionId: string;
  onBack: () => void;
}) {
  const [doc, setDoc] = useState<string | null>(null);
  const [scope, setScope] = useState<Scope>("session");
  const [busy, setBusy] = useState(false);
  const [exportResult, setExportResult] = useState<string | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);

  async function compile(nextScope: Scope) {
    setBusy(true);
    setExportResult(null);
    setExportError(null);
    try {
      const markdowns: string[] = [];
      if (nextScope === "session") {
        const entries = await readSession(project, date, sessionId);
        markdowns.push(...entries.map((e) => `## ${e.title}\n\n${e.summary}`));
      } else if (nextScope === "day") {
        for (const s of await listSessions(project, date)) {
          const entries = await readSession(project, date, s.sessionId);
          markdowns.push(...entries.map((e) => `## ${e.title}\n\n${e.summary}`));
        }
      } else {
        // Week = the 7 calendar days ending on this session's date. Titles
        // carry their date so the model can keep the chronology straight
        // when entries span several days.
        const from = weekStart(date);
        const weekDates = (await listDates(project))
          .filter((d) => d >= from && d <= date)
          .sort();
        for (const d of weekDates) {
          for (const s of await listSessions(project, d)) {
            const entries = await readSession(project, d, s.sessionId);
            markdowns.push(
              ...entries.map((e) => `## [${d}] ${e.title}\n\n${e.summary}`),
            );
          }
        }
      }
      setScope(nextScope);
      setDoc(await compileEntries(markdowns));
    } finally {
      setBusy(false);
    }
  }

  async function exportDoc() {
    if (!doc) return;
    setExportError(null);
    setExportResult(null);
    const filename = `ghostlog-${scope}-${date}-${sessionId}.md`;
    try {
      const path = await invoke<string>("export_document", { filename, content: doc });
      setExportResult(path);
    } catch (e) {
      setExportError(String(e));
    }
  }

  return (
    <div className="space-y-4">
      <header className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <button onClick={onBack} className="text-fg-muted hover:text-fg text-sm transition-colors">
            ← session
          </button>
          <h2 className="font-mono text-sm">
            compile: {date} / {sessionId}
          </h2>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => compile("session")}
            disabled={busy}
            className="text-sm bg-accent hover:bg-accent-dim disabled:opacity-50 text-white px-3 py-1.5 rounded-md transition-colors"
          >
            This session
          </button>
          <button
            onClick={() => compile("day")}
            disabled={busy}
            className="text-sm border border-edge-strong hover:border-fg-muted disabled:opacity-50 text-fg-muted hover:text-fg px-3 py-1.5 rounded-md transition-colors"
          >
            This day
          </button>
          <button
            onClick={() => compile("week")}
            disabled={busy}
            title="The 7 days ending on this session's date"
            className="text-sm border border-edge-strong hover:border-fg-muted disabled:opacity-50 text-fg-muted hover:text-fg px-3 py-1.5 rounded-md transition-colors"
          >
            This week
          </button>
        </div>
      </header>

      {doc ? (
        <>
          <article className="bg-panel border border-edge rounded-lg p-6 prose-ghlg">
            <ReactMarkdown>{doc}</ReactMarkdown>
          </article>
          <div className="flex items-center gap-3">
            <button
              onClick={exportDoc}
              className="text-sm border border-edge-strong hover:border-fg-muted text-fg-muted hover:text-fg px-3 py-1.5 rounded-md transition-colors"
            >
              Export as .md
            </button>
            {exportResult && <span className="text-xs text-fg-faint font-mono truncate">Saved to {exportResult}</span>}
            {exportError && <span className="text-xs text-accent font-mono truncate">{exportError}</span>}
          </div>
        </>
      ) : (
        <p className="text-sm text-fg-faint">
          {busy ? "Compiling…" : "Pick a scope to compile the captured entries into a document."}
        </p>
      )}
    </div>
  );
}
