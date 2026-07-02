/**
 * GHLG review window — shell + view routing (plain React state, no router
 * dependency). Onboarding shows when no folder is configured or when the
 * user chooses to change it.
 */
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Onboarding from "./screens/Onboarding";
import Home from "./screens/Home";
import Archive from "./screens/Archive";
import SessionDetail from "./screens/SessionDetail";
import Curate from "./screens/Curate";
import Compile from "./screens/Compile";
import Settings from "./screens/Settings";

type View =
  | { name: "home" }
  | { name: "archive" }
  | { name: "session"; date: string; sessionId: string }
  | { name: "curate"; date: string; sessionId: string }
  | { name: "compile"; date: string; sessionId: string }
  | { name: "settings" };

export default function App() {
  const [watchedFolder, setWatchedFolder] = useState<string | null | undefined>(undefined);
  const [changingFolder, setChangingFolder] = useState(false);
  const [view, setView] = useState<View>({ name: "home" });

  async function refresh() {
    setWatchedFolder(await invoke<string | null>("get_watched_folder"));
    setChangingFolder(false);
  }

  useEffect(() => {
    refresh();
  }, []);

  if (watchedFolder === undefined) return <main className="min-h-screen bg-ink" />;

  if (watchedFolder === null || changingFolder) {
    return <Onboarding onDone={refresh} />;
  }

  const nav = [
    { key: "home", label: "Home" },
    { key: "archive", label: "Archive" },
    { key: "settings", label: "Settings" },
  ] as const;
  const activeKey =
    view.name === "home" ? "home" : view.name === "settings" ? "settings" : "archive";

  return (
    <div className="min-h-screen bg-ink text-fg font-sans flex">
      <nav className="w-44 shrink-0 border-r border-edge p-4 flex flex-col gap-1">
        <p className="font-semibold tracking-tight px-3 pb-3">
          GHLG<span className="text-accent">.</span>
        </p>
        {nav.map((n) => (
          <button
            key={n.key}
            onClick={() => setView({ name: n.key } as View)}
            className={`text-left text-sm px-3 py-2 rounded-md transition-colors ${
              activeKey === n.key
                ? "bg-panel text-fg"
                : "text-fg-muted hover:text-fg hover:bg-panel"
            }`}
          >
            {n.label}
          </button>
        ))}
      </nav>

      <main className="flex-1 p-6 overflow-y-auto">
        {view.name === "home" && (
          <Home watchedFolder={watchedFolder} onChangeFolder={() => setChangingFolder(true)} />
        )}
        {view.name === "archive" && (
          <Archive
            onOpenSession={(date, sessionId) => setView({ name: "session", date, sessionId })}
          />
        )}
        {view.name === "session" && (
          <SessionDetail
            date={view.date}
            sessionId={view.sessionId}
            onBack={() => setView({ name: "archive" })}
            onCurate={() => setView({ name: "curate", date: view.date, sessionId: view.sessionId })}
            onCompile={() =>
              setView({ name: "compile", date: view.date, sessionId: view.sessionId })
            }
          />
        )}
        {view.name === "curate" && (
          <Curate
            date={view.date}
            sessionId={view.sessionId}
            onDone={() => setView({ name: "session", date: view.date, sessionId: view.sessionId })}
          />
        )}
        {view.name === "compile" && (
          <Compile
            date={view.date}
            sessionId={view.sessionId}
            onBack={() => setView({ name: "session", date: view.date, sessionId: view.sessionId })}
          />
        )}
        {view.name === "settings" && (
          <Settings watchedFolder={watchedFolder} onChangeFolder={() => setChangingFolder(true)} />
        )}
      </main>
    </div>
  );
}
