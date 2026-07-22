/**
 * GHLG review window — shell + view routing (plain React state, no router
 * dependency). Onboarding shows when no folder is configured yet. Multiple
 * projects can be watched at once; the sidebar switcher picks which one the
 * archive screens are scoped to.
 */
import { useEffect, useState, type ReactElement } from "react";
import { invoke } from "@tauri-apps/api/core";
import Onboarding from "./screens/Onboarding";
import Home from "./screens/Home";
import Archive from "./screens/Archive";
import Curate from "./screens/Curate";
import Compile from "./screens/Compile";
import Tasks from "./screens/Tasks";
import Settings from "./screens/Settings";
import { getWatchedFolders, type WatchedProject } from "./lib/watcher";

type View =
  | { name: "home" }
  | { name: "archive" }
  | { name: "curate"; project: string; date: string; sessionId: string }
  | { name: "compile"; project: string; date: string; sessionId: string }
  | { name: "tasks" }
  | { name: "settings" };

const NAV_COLLAPSED_KEY = "ghlg:navCollapsed";

const NAV_ICONS: Record<"home" | "archive" | "tasks" | "settings", ReactElement> = {
  home: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-6 h-6">
      <path strokeLinecap="round" strokeLinejoin="round" d="M3 11.5 12 4l9 7.5M5 10v9a1 1 0 0 0 1 1h4v-6h4v6h4a1 1 0 0 0 1-1v-9" />
    </svg>
  ),
  archive: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-6 h-6">
      <path strokeLinecap="round" strokeLinejoin="round" d="M4 7h16M4 7v12a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1V7M4 7l1.5-3h13L20 7M10 12h4" />
    </svg>
  ),
  tasks: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-6 h-6">
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path strokeLinecap="round" d="M8 8v8M13 8v5M18 8v11" />
    </svg>
  ),
  settings: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-6 h-6">
      <circle cx="12" cy="12" r="3" />
      <path strokeLinecap="round" strokeLinejoin="round" d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  ),
};

export default function App() {
  const [folders, setFolders] = useState<WatchedProject[] | undefined>(undefined);
  const [project, setProject] = useState<string>("");
  const [view, setView] = useState<View>({ name: "home" });
  const [navCollapsed, setNavCollapsed] = useState(
    () => localStorage.getItem(NAV_COLLAPSED_KEY) === "1"
  );

  function toggleNavCollapsed() {
    setNavCollapsed((prev) => {
      const next = !prev;
      localStorage.setItem(NAV_COLLAPSED_KEY, next ? "1" : "0");
      return next;
    });
  }

  async function refresh() {
    const list = await getWatchedFolders();
    setFolders(list);
    // Keep the selection if it still exists, else fall back to the first.
    setProject((prev) => (list.some((f) => f.name === prev) ? prev : (list[0]?.name ?? "")));
  }

  useEffect(() => {
    refresh();
  }, []);

  // Opt-in, best-effort: polls each watched project's session log for
  // whichever terminal AI tool is selected in Settings (if any) for new
  // human-typed prompts. No-ops instantly server-side when the source is
  // "none", so it's cheap to just always run this rather than thread the
  // setting through here too.
  useEffect(() => {
    if (!folders || folders.length === 0) return;
    const poll = () => {
      for (const f of folders) {
        invoke("poll_agent_prompts", { project: f.name }).catch(() => {});
      }
    };
    poll();
    const id = setInterval(poll, 20_000);
    return () => clearInterval(id);
  }, [folders]);

  function switchProject(name: string) {
    setProject(name);
    // Archive-family views hold data from the old project; go home.
    if (view.name !== "home" && view.name !== "settings") setView({ name: "archive" });
  }

  if (folders === undefined) return <main className="min-h-screen bg-ink" />;

  if (folders.length === 0) {
    return <Onboarding onDone={refresh} />;
  }

  const nav = [
    { key: "home", label: "Home" },
    { key: "archive", label: "Archive" },
    { key: "tasks", label: "Board" },
    { key: "settings", label: "Settings" },
  ] as const;
  const activeKey =
    view.name === "home"
      ? "home"
      : view.name === "settings"
        ? "settings"
        : view.name === "tasks"
          ? "tasks"
          : "archive";

  const navToggleButton = (
    <button
      onClick={toggleNavCollapsed}
      title={navCollapsed ? "Show sidebar" : "Hide sidebar"}
      className={`w-10 h-10 flex items-center justify-center rounded-md text-white/70 hover:text-white hover:bg-white/10 transition-colors shrink-0 ${
        navCollapsed ? "mx-auto" : ""
      }`}
    >
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        className={`w-6 h-6 transition-transform ${navCollapsed ? "rotate-180" : ""}`}
      >
        <rect x="3" y="3" width="18" height="18" rx="2" strokeLinecap="round" strokeLinejoin="round" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M9 3v18" />
        <path strokeLinecap="round" strokeLinejoin="round" d="m16 15-3-3 3-3" />
      </svg>
    </button>
  );

  return (
    <div className="min-h-screen bg-ink text-fg font-sans flex">
      <nav
        className={`shrink-0 sticky top-0 h-screen p-4 flex flex-col gap-1 transition-[width] duration-200 overflow-hidden bg-accent ${
          navCollapsed ? "w-16 px-2" : "w-44"
        }`}
      >
        <div className={`pb-3 ${navCollapsed ? "flex justify-center" : "px-3"}`}>
          {navCollapsed ? (
            <img src="/white-ghost.png" alt="Ghostlog" className="w-7 h-7 object-contain" />
          ) : (
            <p className="font-semibold text-2xl tracking-tight text-white flex items-center gap-2">
              <img src="/white-ghost.png" alt="" className="w-7 h-7 object-contain" />
              <span
                style={{
                  backgroundImage: "linear-gradient(to bottom, rgba(255,255,255,1) 0%, rgba(255,255,255,0.75) 100%)",
                  WebkitBackgroundClip: "text",
                  backgroundClip: "text",
                  color: "transparent",
                }}
              >
                Ghostlog<span className="text-black/60">.</span>
              </span>
            </p>
          )}
        </div>
        {nav.map((n) => (
          <button
            key={n.key}
            onClick={() => setView({ name: n.key } as View)}
            title={n.label}
            className={`flex items-center gap-2 text-sm rounded-md transition-colors ${
              navCollapsed ? "w-10 h-10 mx-auto justify-center" : "text-left px-3 py-2"
            } ${
              activeKey === n.key
                ? "bg-white/20 text-white"
                : "text-white/70 hover:text-white hover:bg-white/10"
            }`}
          >
            {NAV_ICONS[n.key]}
            {!navCollapsed && n.label}
          </button>
        ))}

        <div className="mt-auto pt-3">{navToggleButton}</div>
      </nav>

      <main className="flex-1 p-6 overflow-y-auto">
        {view.name === "home" && (
          <Home
            folders={folders}
            selectedProject={project}
            onSelectProject={switchProject}
            onOpenSettings={() => setView({ name: "settings" })}
          />
        )}
        {view.name === "archive" && (
          <Archive
            folders={folders}
            project={project}
            onOpenCurate={(p, date, sessionId) => setView({ name: "curate", project: p, date, sessionId })}
            onOpenCompile={(p, date, sessionId) => setView({ name: "compile", project: p, date, sessionId })}
          />
        )}
        {view.name === "curate" && (
          <Curate
            project={view.project}
            date={view.date}
            sessionId={view.sessionId}
            onDone={() => setView({ name: "archive" })}
          />
        )}
        {view.name === "compile" && (
          <Compile
            project={view.project}
            date={view.date}
            sessionId={view.sessionId}
            onBack={() => setView({ name: "archive" })}
          />
        )}
        {view.name === "tasks" && <Tasks selectedProject={project} />}
        {view.name === "settings" && <Settings folders={folders} onFoldersChanged={refresh} />}
      </main>
    </div>
  );
}
