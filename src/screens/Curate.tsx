/**
 * Curation checklist — the one screen touched regularly, kept fast:
 * checkbox per entry, "Keep selected" / "Discard rest".
 */
import { useEffect, useState } from "react";
import TagBadge from "../components/TagBadge";
import { readSession, deleteEntry, type SessionEntry } from "../lib/session";

export default function Curate({
  project,
  date,
  sessionId,
  onDone,
}: {
  project: string;
  date: string;
  sessionId: string;
  onDone: () => void;
}) {
  const [entries, setEntries] = useState<SessionEntry[]>([]);
  const [kept, setKept] = useState<Set<string>>(new Set());
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    readSession(project, date, sessionId).then((es) => {
      setEntries(es);
      setKept(new Set(es.map((e) => e.id))); // default: keep everything
    });
  }, [project, date, sessionId]);

  function toggle(id: string) {
    const next = new Set(kept);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setKept(next);
  }

  const discardCount = entries.length - kept.size;

  async function discardRest() {
    for (const e of entries) {
      if (!kept.has(e.id)) await deleteEntry(project, date, sessionId, e.id);
    }
    onDone();
  }

  return (
    <div className="space-y-4">
      <header className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <button onClick={onDone} className="text-fg-muted hover:text-fg text-sm transition-colors">
            ← session
          </button>
          <h2 className="font-mono text-sm">
            curate: {date} / {sessionId}
          </h2>
        </div>
        <div className="flex items-center gap-3">
          <span className="text-xs text-fg-muted">
            keeping {kept.size} / {entries.length}
          </span>
          {confirming ? (
            <>
              <button
                onClick={discardRest}
                className="text-sm bg-accent hover:bg-accent-dim text-white px-3 py-1.5 rounded-md transition-colors"
              >
                Confirm: discard {discardCount}
              </button>
              <button
                onClick={() => setConfirming(false)}
                className="text-sm text-fg-muted hover:text-fg px-2 transition-colors"
              >
                Cancel
              </button>
            </>
          ) : (
            <button
              onClick={() => (discardCount > 0 ? setConfirming(true) : onDone())}
              className="text-sm bg-accent hover:bg-accent-dim text-white px-3 py-1.5 rounded-md transition-colors"
            >
              Keep selected{discardCount > 0 ? `, discard ${discardCount}` : ""}
            </button>
          )}
        </div>
      </header>

      <ul className="space-y-1.5">
        {entries.map((e) => (
          <li key={e.id}>
            <label
              className={`flex items-center gap-3 bg-panel border rounded-lg px-4 py-2.5 cursor-pointer transition-colors ${
                kept.has(e.id) ? "border-edge" : "border-edge opacity-40"
              }`}
            >
              <input
                type="checkbox"
                checked={kept.has(e.id)}
                onChange={() => toggle(e.id)}
                className="accent-[#e63946]"
              />
              <TagBadge tag={e.tag} />
              <span className="text-sm truncate flex-1">{e.title}</span>
              <span className="font-mono text-xs text-fg-faint">
                {e.timestamp ? new Date(e.timestamp).toLocaleTimeString() : ""}
              </span>
            </label>
          </li>
        ))}
      </ul>
    </div>
  );
}
