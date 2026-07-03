/**
 * Session archive: browse ANY past date, not just recent ones.
 * Date list (searchable) → sessions of that date → opens session detail.
 * The search box above the session list searches entry CONTENT (title, tag,
 * summary) across the whole archive; results link straight into the session
 * that holds the matching entry.
 */
import { useEffect, useState } from "react";
import {
  listDates,
  listSessions,
  searchEntries,
  type SearchHit,
  type SessionMeta,
} from "../lib/session";
import TagBadge from "../components/TagBadge";

const SEARCH_DEBOUNCE_MS = 250;

export default function Archive({
  onOpenSession,
}: {
  onOpenSession: (date: string, sessionId: string) => void;
}) {
  const [dates, setDates] = useState<string[]>([]);
  const [filter, setFilter] = useState("");
  const [selectedDate, setSelectedDate] = useState<string | null>(null);
  const [sessions, setSessions] = useState<SessionMeta[]>([]);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [searching, setSearching] = useState(false);

  useEffect(() => {
    listDates().then((d) => {
      setDates(d);
      if (d.length > 0) setSelectedDate(d[0]);
    });
  }, []);

  useEffect(() => {
    if (selectedDate) listSessions(selectedDate).then(setSessions);
  }, [selectedDate]);

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
      searchEntries(q)
        .then(setHits)
        .finally(() => setSearching(false));
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [query]);

  const visibleDates = dates.filter((d) => d.includes(filter));
  const searchActive = query.trim().length > 0;

  return (
    <div className="flex gap-6 h-full">
      <aside className="w-56 shrink-0 space-y-3">
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
            <button
              key={d}
              onClick={() => setSelectedDate(d)}
              className={`w-full text-left font-mono text-sm px-3 py-2 rounded-md transition-colors ${
                d === selectedDate
                  ? "bg-panel-raised text-fg border border-edge-strong"
                  : "text-fg-muted hover:text-fg hover:bg-panel"
              }`}
            >
              {d}
            </button>
          ))}
        </div>
      </aside>

      <section className="flex-1 space-y-3 min-w-0">
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search all entries… (title, tag, summary)"
          className="w-full bg-ink border border-edge rounded-md px-3 py-2 text-sm placeholder:text-fg-faint focus:outline-none focus:border-accent"
        />

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
                onClick={() => onOpenSession(h.date, h.sessionId)}
                className="w-full bg-panel hover:bg-panel-raised border border-edge rounded-lg px-4 py-3 transition-colors text-left space-y-1"
              >
                <div className="flex items-center gap-2">
                  <TagBadge tag={h.entry.tag} />
                  <span className="text-sm font-medium truncate">
                    {h.entry.title}
                  </span>
                  <span className="ml-auto shrink-0 font-mono text-xs text-fg-muted">
                    {h.date} · {h.sessionId}
                  </span>
                </div>
                <p className="text-xs text-fg-muted line-clamp-2">
                  {h.entry.summary}
                </p>
              </button>
            ))}
          </div>
        ) : (
          <div className="space-y-2">
            {selectedDate && sessions.length === 0 && (
              <p className="text-sm text-fg-faint">
                No sessions on {selectedDate}.
              </p>
            )}
            {sessions.map((s) => (
              <button
                key={s.sessionId}
                onClick={() => onOpenSession(s.date, s.sessionId)}
                className="w-full flex items-center justify-between bg-panel hover:bg-panel-raised border border-edge rounded-lg px-4 py-3 transition-colors text-left"
              >
                <span className="font-mono text-sm">{s.sessionId}</span>
                <span className="text-xs text-fg-muted">
                  {s.entryCount} {s.entryCount === 1 ? "entry" : "entries"}
                </span>
              </button>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
