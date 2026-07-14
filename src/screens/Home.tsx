/**
 * Home — the project dashboard: one card per watched project (entries
 * captured today, per-project "Log now"), plus the global watch toggle and
 * the live last-captured-event line.
 */
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  startWatching,
  stopWatching,
  getWatchState,
  triggerManualCapture,
  type WatchState,
  type WatchedProject,
} from "../lib/watcher";

interface LastEvent {
  timestamp: string;
  kind: string;
  detail: string;
  project: string;
}

interface SessionMetaRaw {
  entryCount: number;
}

function todayStr(): string {
  const d = new Date();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

export default function Home({
  folders,
  selectedProject,
  onSelectProject,
  onOpenSettings,
}: {
  folders: WatchedProject[];
  selectedProject: string;
  onSelectProject: (name: string) => void;
  onOpenSettings: () => void;
}) {
  const [watchState, setWatchState] = useState<WatchState>("idle");
  const [lastEvent, setLastEvent] = useState<LastEvent | null>(null);
  const [note, setNote] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [capturedFor, setCapturedFor] = useState<string | null>(null);
  const [todayCounts, setTodayCounts] = useState<Record<string, number>>({});

  async function refreshCounts() {
    const date = todayStr();
    const counts: Record<string, number> = {};
    await Promise.all(
      folders.map(async (f) => {
        try {
          const sessions = await invoke<SessionMetaRaw[]>("list_sessions", {
            project: f.name,
            date,
          });
          counts[f.name] = sessions.reduce((n, s) => n + s.entryCount, 0);
        } catch {
          counts[f.name] = 0;
        }
      }),
    );
    setTodayCounts(counts);
  }

  useEffect(() => {
    getWatchState().then(setWatchState);
    invoke<LastEvent | null>("get_last_event").then(setLastEvent);
    refreshCounts();
    const unlisten = listen<LastEvent>("ghlg://capture", (e) => {
      setLastEvent(e.payload);
      refreshCounts();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [folders]);

  async function toggleWatching() {
    setError(null);
    try {
      if (watchState === "watching") {
        await stopWatching();
        setWatchState("idle");
      } else {
        await startWatching();
        setWatchState("watching");
      }
    } catch (e) {
      setError(String(e));
    }
  }

  async function logNow(project: string) {
    setError(null);
    try {
      await triggerManualCapture(project, note.trim() || undefined);
      setNote("");
      setCapturedFor(project);
      setTimeout(() => setCapturedFor(null), 2000);
      refreshCounts();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="space-y-6">
      {/* Global watch state */}
      <section className="bg-panel border border-edge rounded-lg p-5 flex items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <button
            onClick={toggleWatching}
            className={`text-sm font-medium px-4 py-2 rounded-md transition-colors ${
              watchState === "watching"
                ? "border border-accent text-accent hover:bg-accent/10"
                : "bg-accent hover:bg-accent-dim text-white"
            }`}
          >
            {watchState === "watching" ? "Stop watching" : "Start watching"}
          </button>
          <span className="flex items-center gap-2 text-sm text-fg-muted">
            <span
              className={`h-2 w-2 rounded-full ${
                watchState === "watching" ? "bg-accent animate-pulse" : "bg-fg-faint"
              }`}
            />
            {watchState === "watching"
              ? `capturing across ${folders.length} ${folders.length === 1 ? "project" : "projects"}`
              : "idle"}
          </span>
        </div>
        <button
          onClick={onOpenSettings}
          className="shrink-0 text-xs text-fg-muted hover:text-fg border border-edge-strong hover:border-fg-muted px-3 py-1.5 rounded-md transition-colors"
        >
          Manage projects
        </button>
      </section>

      {/* Project cards */}
      <section className="grid gap-4 sm:grid-cols-2">
        {folders.map((f) => {
          const selected = f.name === selectedProject;
          return (
            <div
              key={f.path}
              onClick={() => onSelectProject(f.name)}
              className={`bg-panel border rounded-lg p-4 space-y-3 cursor-pointer transition-colors ${
                selected ? "border-accent" : "border-edge hover:border-edge-strong"
              }`}
            >
              <div className="flex items-center justify-between gap-3">
                <p className="font-semibold tracking-tight truncate">
                  {f.name}
                  {selected && <span className="text-accent">.</span>}
                </p>
                <span
                  className={`h-2 w-2 rounded-full shrink-0 ${
                    watchState === "watching" ? "bg-accent" : "bg-fg-faint"
                  }`}
                />
              </div>
              <p className="font-mono text-xs text-fg-faint break-all">{f.path}</p>
              <div className="flex items-center justify-between pt-1">
                <span className="text-xs text-fg-muted">
                  <span className="font-mono text-fg">{todayCounts[f.name] ?? 0}</span>{" "}
                  {(todayCounts[f.name] ?? 0) === 1 ? "entry" : "entries"} today
                </span>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    logNow(f.name);
                  }}
                  className="text-xs bg-accent hover:bg-accent-dim text-white font-medium px-3 py-1.5 rounded-md transition-colors"
                >
                  {capturedFor === f.name ? "Captured" : "Log now"}
                </button>
              </div>
            </div>
          );
        })}
      </section>

      {/* Shared note for the next manual capture */}
      <section className="bg-panel border border-edge rounded-lg p-5 space-y-3">
        <p className="text-xs text-fg-faint uppercase tracking-wide">Log this now</p>
        <div className="flex gap-3">
          <input
            value={note}
            onChange={(e) => setNote(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && logNow(selectedProject)}
            placeholder="optional hint for the model (not required — the diff does the work)"
            className="flex-1 bg-ink border border-edge rounded-md px-3 py-2 text-sm font-mono placeholder:text-fg-faint focus:outline-none focus:border-accent"
          />
          <button
            onClick={() => logNow(selectedProject)}
            className="bg-accent hover:bg-accent-dim text-white text-sm font-medium px-5 py-2 rounded-md transition-colors"
          >
            {capturedFor === selectedProject ? "Captured" : "Log now"}
          </button>
        </div>
        <p className="text-xs text-fg-faint">
          Reads the selected project&apos;s uncommitted changes and reconstructs the problem, the
          fix, and the reasoning — your note is just a nudge, not the documentation.
        </p>
      </section>

      {/* Last captured event */}
      <section className="bg-panel border border-edge rounded-lg p-5">
        <p className="text-xs text-fg-faint uppercase tracking-wide mb-2">Last captured event</p>
        {lastEvent ? (
          <div className="flex items-baseline gap-3 text-sm">
            <span className="font-mono text-fg-faint">
              {new Date(lastEvent.timestamp).toLocaleTimeString()}
            </span>
            {lastEvent.project && (
              <span className="text-accent font-mono text-xs">{lastEvent.project}</span>
            )}
            <span className="text-fg-muted font-mono">{lastEvent.kind}</span>
            <span className="truncate">{lastEvent.detail}</span>
          </div>
        ) : (
          <p className="text-sm text-fg-faint">Nothing captured yet this run.</p>
        )}
      </section>

      {error && <p className="text-sm text-accent font-mono">{error}</p>}
    </div>
  );
}
