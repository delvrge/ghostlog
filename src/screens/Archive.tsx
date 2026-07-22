/**
 * Session archive: browse ANY past date, not just recent ones, across one
 * project or all of them at once. Date list (searchable) → entries of that
 * date, flattened across sessions (sessions are an implementation detail
 * the user never needs to see). Clicking an entry expands it in place — no
 * separate page/window — with inline edit and delete, and Curate/Compile
 * are reachable straight from here. The search box searches entry CONTENT
 * (title, tag, summary); results jump to that entry's date and expand it.
 */
import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  cleanupEmpty,
  deleteDate,
  deleteEntry,
  listDates,
  listSessions,
  readSession,
  searchEntries,
  updateEntry,
  type SearchHit,
  type SessionEntry,
} from "../lib/session";
import type { WatchedProject } from "../lib/watcher";
import TagBadge from "../components/TagBadge";

const SEARCH_DEBOUNCE_MS = 250;
/** How long a first click's "Confirm?" state stays armed before resetting. */
const CONFIRM_MS = 3000;
/** Sentinel project value meaning "every watched project", not a real name. */
const ALL_PROJECTS = "__all__";

const TAGS: SessionEntry["tag"][] = [
  "bugfix",
  "feature",
  "refactor",
  "performance",
  "ui",
  "configuration",
  "experiment",
  "decision",
  "question",
  "note",
  "update",
];

interface DateEntry {
  project: string;
  sessionId: string;
  entry: SessionEntry;
}

/** A search hit tagged with which project it came from — `searchEntries`
 * is called once per scoped project, so that has to be attached here
 * rather than assumed. */
type ProjectSearchHit = SearchHit & { project: string };

export default function Archive({
  folders,
  project,
  onOpenCurate,
  onOpenCompile,
}: {
  folders: WatchedProject[];
  project: string;
  onOpenCurate: (project: string, date: string, sessionId: string) => void;
  onOpenCompile: (project: string, date: string, sessionId: string) => void;
}) {
  // The project filter lives here (not the nav) and adds an "All projects"
  // option the nav's single-select never had. Defaults to whatever project
  // is globally selected, but browsing the archive doesn't need to change
  // that global selection, so this is its own local state.
  const [scope, setScope] = useState<string>(project);
  const [dates, setDates] = useState<string[]>([]);
  const [filter, setFilter] = useState("");
  const [selectedDate, setSelectedDate] = useState<string | null>(null);
  const [dateEntries, setDateEntries] = useState<DateEntry[]>([]);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ProjectSearchHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState({ tag: "update" as SessionEntry["tag"], title: "", summary: "" });
  const [confirmingDeleteEntry, setConfirmingDeleteEntry] = useState<string | null>(null);
  const [dateMenuOpen, setDateMenuOpen] = useState<string | null>(null);
  const [confirmingDeleteDate, setConfirmingDeleteDate] = useState<string | null>(null);
  const [confirmingFlush, setConfirmingFlush] = useState(false);
  const [flushResult, setFlushResult] = useState<string | null>(null);

  const scopedProjects = scope === ALL_PROJECTS ? folders.map((f) => f.name) : [scope];

  async function loadDates() {
    const perProject = await Promise.all(scopedProjects.map((p) => listDates(p)));
    const union = Array.from(new Set(perProject.flat())).sort().reverse();
    setDates(union);
    setSelectedDate((prev) => (prev && union.includes(prev) ? prev : union[0] ?? null));
  }

  useEffect(() => {
    loadDates();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scope]);

  async function loadDateEntries(date: string) {
    const perProject = await Promise.all(
      scopedProjects.map(async (p) => {
        const sessions = await listSessions(p, date);
        const perSession = await Promise.all(
          sessions.map(async (s) => ({ s, entries: await readSession(p, date, s.sessionId) })),
        );
        return perSession
          .filter((x) => x.entries.length > 0)
          .flatMap((x) => x.entries.map((entry) => ({ project: p, sessionId: x.s.sessionId, entry })));
      }),
    );
    // Grouped by project (in watched-folder order) with each group sorted
    // newest-first internally — "All projects" separates by a title per
    // project rather than interleaving everything by timestamp.
    setDateEntries(perProject.flat().sort((a, b) => {
      if (a.project !== b.project) return scopedProjects.indexOf(a.project) - scopedProjects.indexOf(b.project);
      return b.entry.timestamp.localeCompare(a.entry.timestamp);
    }));
  }

  useEffect(() => {
    if (selectedDate) loadDateEntries(selectedDate);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedDate, scope]);

  // Debounced content search — one backend walk per pause in typing, not
  // one per keystroke.
  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setHits([]);
      setSearching(false);
      return;
    }
    setSearching(true);
    const t = setTimeout(() => {
      Promise.all(
        scopedProjects.map(async (p) => (await searchEntries(p, q)).map((h) => ({ ...h, project: p }))),
      )
        .then((perProject) => setHits(perProject.flat()))
        .finally(() => setSearching(false));
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, scope]);

  // Reset armed "Confirm?" states after a few seconds of inaction, so a
  // stray click much later can't land on an already-armed delete button.
  useEffect(() => {
    if (!confirmingDeleteEntry) return;
    const t = setTimeout(() => setConfirmingDeleteEntry(null), CONFIRM_MS);
    return () => clearTimeout(t);
  }, [confirmingDeleteEntry]);
  useEffect(() => {
    if (!confirmingDeleteDate) return;
    const t = setTimeout(() => setConfirmingDeleteDate(null), CONFIRM_MS);
    return () => clearTimeout(t);
  }, [confirmingDeleteDate]);
  useEffect(() => {
    if (!confirmingFlush) return;
    const t = setTimeout(() => setConfirmingFlush(false), CONFIRM_MS);
    return () => clearTimeout(t);
  }, [confirmingFlush]);

  function toggleExpanded(entry: SessionEntry) {
    setEditing(null);
    setExpanded((prev) => (prev === entry.markdownPath ? null : entry.markdownPath));
  }

  function beginEdit(entry: SessionEntry) {
    setEditing(entry.id);
    setDraft({ tag: entry.tag, title: entry.title, summary: entry.summary });
  }

  async function saveEdit(entryProject: string, sessionId: string, entryId: string) {
    await updateEntry(entryProject, selectedDate!, sessionId, entryId, draft);
    setEditing(null);
    loadDateEntries(selectedDate!);
  }

  async function handleDeleteEntry(entryProject: string, sessionId: string, entryId: string) {
    if (confirmingDeleteEntry !== entryId) {
      setConfirmingDeleteEntry(entryId);
      return;
    }
    setConfirmingDeleteEntry(null);
    await deleteEntry(entryProject, selectedDate!, sessionId, entryId);
    loadDateEntries(selectedDate!);
  }

  async function handleDeleteDate(date: string) {
    if (confirmingDeleteDate !== date) {
      setConfirmingDeleteDate(date);
      return;
    }
    setConfirmingDeleteDate(null);
    setDateMenuOpen(null);
    // A specific project must be selected to know which project's date
    // folder to remove — the menu action is disabled in "All projects".
    await deleteDate(scope, date);
    await loadDates();
  }

  async function handleFlushEmpty() {
    if (!confirmingFlush) {
      setConfirmingFlush(true);
      return;
    }
    setConfirmingFlush(false);
    setFlushResult(null);
    const perProject = await Promise.all(scopedProjects.map((p) => cleanupEmpty(p)));
    const total = perProject.reduce((a, b) => a + b, 0);
    setFlushResult(total === 0 ? "Nothing to flush" : `Flushed ${total} empty folder${total === 1 ? "" : "s"}`);
    await loadDates();
    if (selectedDate) await loadDateEntries(selectedDate);
  }

  async function openSearchHit(hit: ProjectSearchHit) {
    setQuery("");
    setSelectedDate(hit.date);
    await loadDateEntries(hit.date);
    setExpanded(hit.entry.markdownPath);
  }

  const visibleDates = dates.filter((d) => d.includes(filter));
  const searchActive = query.trim().length > 0;
  // Curate/Compile need one concrete project + session — meaningless
  // (and disabled) in "All projects".
  const activeEntry = scope !== ALL_PROJECTS ? dateEntries[0] : undefined;

  return (
    <div className="flex gap-6 h-full">
      <aside className="w-56 shrink-0 space-y-3">
        <select
          value={scope}
          onChange={(e) => setScope(e.target.value)}
          className="w-full bg-panel-raised border border-edge-strong rounded-md px-3 py-2 text-sm focus:outline-none"
        >
          <option value={ALL_PROJECTS}>All projects</option>
          {folders.map((f) => (
            <option key={f.name} value={f.name}>
              {f.name}
            </option>
          ))}
        </select>
        <input
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="Filter dates… (2026-07)"
          className="w-full bg-ink border border-edge rounded-md px-3 py-2 text-sm font-mono placeholder:text-fg-faint focus:outline-none focus:border-accent"
        />
        <div className="space-y-1 overflow-y-auto">
          {visibleDates.length === 0 && (
            <p className="text-sm text-fg-faint px-1">No sessions yet.</p>
          )}
          {visibleDates.map((d) => (
            <div key={d} className="relative group">
              <button
                onClick={() => setSelectedDate(d)}
                className={`w-full text-left font-mono text-sm pl-3 pr-8 py-2 rounded-md transition-colors ${
                  d === selectedDate
                    ? "bg-panel-raised text-fg border border-edge-strong"
                    : "text-fg-muted hover:text-fg hover:bg-panel"
                }`}
              >
                {d}
              </button>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setDateMenuOpen((prev) => (prev === d ? null : d));
                }}
                title="Date actions"
                className="absolute right-1 top-1/2 -translate-y-1/2 px-1.5 py-1 rounded text-fg-faint hover:text-fg hover:bg-panel-raised opacity-0 group-hover:opacity-100 transition-opacity"
              >
                ⋮
              </button>
              {dateMenuOpen === d && (
                <div className="absolute right-0 top-full mt-1 z-10 bg-panel-raised border border-edge-strong rounded-md shadow-lg overflow-hidden">
                  <button
                    onClick={() => handleDeleteDate(d)}
                    disabled={scope === ALL_PROJECTS}
                    title={scope === ALL_PROJECTS ? "Pick a single project to delete a date" : undefined}
                    className="block w-full text-left px-3 py-2 text-xs text-fg-muted hover:text-accent hover:bg-panel disabled:opacity-40 disabled:hover:text-fg-muted transition-colors whitespace-nowrap"
                  >
                    {confirmingDeleteDate === d ? "Confirm delete?" : "Delete this date"}
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>
      </aside>

      <section className="flex-1 space-y-3 min-w-0">
        <div className="flex items-center gap-3">
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search all entries… (title, tag, summary)"
            className="flex-1 bg-ink border border-edge rounded-md px-3 py-2 text-sm placeholder:text-fg-faint focus:outline-none focus:border-accent"
          />
          <button
            onClick={() => activeEntry && onOpenCurate(activeEntry.project, selectedDate!, activeEntry.sessionId)}
            disabled={!activeEntry}
            title={scope === ALL_PROJECTS ? "Pick a single project to curate" : undefined}
            className="shrink-0 text-sm border border-edge-strong hover:border-fg-muted disabled:opacity-40 disabled:hover:border-edge-strong text-fg-muted hover:text-fg px-3 py-2 rounded-md transition-colors"
          >
            Curate
          </button>
          <button
            onClick={() => activeEntry && onOpenCompile(activeEntry.project, selectedDate!, activeEntry.sessionId)}
            disabled={!activeEntry}
            title={scope === ALL_PROJECTS ? "Pick a single project to compile" : undefined}
            className="shrink-0 text-sm bg-accent hover:bg-accent-dim disabled:opacity-40 disabled:hover:bg-accent text-white px-3 py-2 rounded-md transition-colors"
          >
            Compile
          </button>
          <button
            onClick={handleFlushEmpty}
            title="Delete every empty session/date folder — entries with content are never touched"
            className={`shrink-0 text-sm border rounded-md px-3 py-2 transition-colors ${
              confirmingFlush
                ? "border-accent text-accent"
                : "border-edge-strong hover:border-fg-muted text-fg-muted hover:text-fg"
            }`}
          >
            {confirmingFlush ? "Confirm flush?" : "Flush empty"}
          </button>
        </div>
        {flushResult && <p className="text-xs text-fg-faint">{flushResult}</p>}

        {searchActive ? (
          <div className="space-y-2">
            {searching && <p className="text-sm text-fg-faint">Searching…</p>}
            {!searching && hits.length === 0 && (
              <p className="text-sm text-fg-faint">
                No entries match “{query.trim()}”.
              </p>
            )}
            {hits.map((h) => (
              <button
                key={h.entry.markdownPath}
                onClick={() => openSearchHit(h)}
                className="w-full bg-panel hover:bg-panel-raised border border-edge rounded-lg px-4 py-3 transition-colors text-left space-y-1"
              >
                <div className="flex items-center gap-2">
                  <TagBadge tag={h.entry.tag} />
                  <span className="text-sm font-medium truncate">
                    {h.entry.title}
                  </span>
                  <span className="ml-auto shrink-0 font-mono text-xs text-fg-muted">
                    {scope === ALL_PROJECTS ? `${h.project} · ${h.date}` : h.date}
                  </span>
                </div>
                <p className="text-xs text-fg-muted line-clamp-2">
                  {h.entry.summary}
                </p>
              </button>
            ))}
          </div>
        ) : (
          <div className="space-y-4">
            {selectedDate && dateEntries.length === 0 && (
              <p className="text-sm text-fg-faint">
                No entries on {selectedDate}.
              </p>
            )}
            {scopedProjects.map((p) => {
              const entries = dateEntries.filter((d) => d.project === p);
              if (entries.length === 0) return null;
              return (
                <div key={p} className="space-y-2">
                  {scope === ALL_PROJECTS && (
                    <h3 className="text-xs text-fg-faint uppercase tracking-wide pt-1">{p}</h3>
                  )}
                  {entries.map(({ sessionId, entry }) => {
                    const isExpanded = expanded === entry.markdownPath;
                    const isEditing = editing === entry.id;
                    return (
                      <div
                        key={entry.markdownPath}
                        className="bg-panel border border-edge rounded-lg transition-colors"
                      >
                        <button
                          onClick={() => toggleExpanded(entry)}
                          className="w-full flex items-center gap-3 text-left px-4 py-3 hover:bg-panel-raised rounded-lg transition-colors"
                        >
                          <TagBadge tag={entry.tag} />
                          <span className="text-sm font-medium truncate">{entry.title}</span>
                          <span className="ml-auto shrink-0 font-mono text-xs text-fg-muted">
                            {entry.timestamp.slice(11, 16)}
                          </span>
                        </button>

                        {isExpanded && (
                          <div className="px-4 pb-4 pt-1 border-t border-edge">
                            {isEditing ? (
                              <div className="space-y-3 pt-3">
                                <div className="flex flex-wrap gap-2">
                                  {TAGS.map((t) => (
                                    <button
                                      key={t}
                                      onClick={() => setDraft({ ...draft, tag: t })}
                                      className={`text-xs font-mono px-2 py-1 rounded border transition-colors ${
                                        draft.tag === t
                                          ? "border-accent text-accent"
                                          : "border-edge-strong text-fg-muted hover:text-fg"
                                      }`}
                                    >
                                      {t}
                                    </button>
                                  ))}
                                </div>
                                <input
                                  value={draft.title}
                                  onChange={(ev) => setDraft({ ...draft, title: ev.target.value })}
                                  className="w-full bg-ink border border-edge rounded-md px-3 py-2 text-sm focus:outline-none focus:border-accent"
                                />
                                <textarea
                                  value={draft.summary}
                                  onChange={(ev) => setDraft({ ...draft, summary: ev.target.value })}
                                  rows={5}
                                  className="w-full bg-ink border border-edge rounded-md px-3 py-2 text-sm font-mono focus:outline-none focus:border-accent"
                                />
                                <div className="flex gap-2">
                                  <button
                                    onClick={() => saveEdit(p, sessionId, entry.id)}
                                    className="text-sm bg-accent hover:bg-accent-dim text-white px-3 py-1.5 rounded-md transition-colors"
                                  >
                                    Save
                                  </button>
                                  <button
                                    onClick={() => setEditing(null)}
                                    className="text-sm text-fg-muted hover:text-fg px-3 py-1.5 transition-colors"
                                  >
                                    Cancel
                                  </button>
                                </div>
                              </div>
                            ) : (
                              <div className="flex gap-4 pt-3">
                                <div className="flex-1 min-w-0 space-y-2">
                                  <p className="text-sm text-fg-muted whitespace-pre-wrap">{entry.summary}</p>
                                  <div className="flex gap-3 pt-1">
                                    <button
                                      onClick={() => beginEdit(entry)}
                                      className="text-xs text-fg-muted hover:text-fg transition-colors"
                                    >
                                      Edit
                                    </button>
                                    <button
                                      onClick={() => handleDeleteEntry(p, sessionId, entry.id)}
                                      className={`text-xs transition-colors ${
                                        confirmingDeleteEntry === entry.id
                                          ? "text-accent"
                                          : "text-fg-muted hover:text-accent"
                                      }`}
                                    >
                                      {confirmingDeleteEntry === entry.id ? "Confirm delete?" : "Delete"}
                                    </button>
                                  </div>
                                </div>
                                {entry.screenshotPath && (
                                  <img
                                    src={convertFileSrc(entry.screenshotPath)}
                                    alt=""
                                    className="w-32 h-20 object-cover rounded border border-edge shrink-0"
                                  />
                                )}
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}
