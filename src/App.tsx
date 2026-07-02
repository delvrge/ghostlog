/**
 * GHLG review window — root component.
 * Routes to onboarding when no watched folder is configured yet;
 * the full review screens (home/archive/curation/compile) land in step 5.
 */
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Onboarding from "./screens/Onboarding";

export default function App() {
  const [watchedFolder, setWatchedFolder] = useState<string | null | undefined>(undefined);

  async function refresh() {
    setWatchedFolder(await invoke<string | null>("get_watched_folder"));
  }

  useEffect(() => {
    refresh();
  }, []);

  // Still loading initial state.
  if (watchedFolder === undefined) {
    return <main className="min-h-screen bg-ink" />;
  }

  if (watchedFolder === null) {
    return <Onboarding onDone={refresh} />;
  }

  return (
    <main className="min-h-screen bg-ink text-fg font-sans flex items-center justify-center">
      <div className="text-center space-y-2">
        <h1 className="text-2xl font-semibold tracking-tight">
          GHLG<span className="text-accent">.</span>
        </h1>
        <p className="text-fg-muted font-mono text-sm break-all px-8">watching: {watchedFolder}</p>
        <p className="text-fg-faint text-sm">review screens land in step 5</p>
      </div>
    </main>
  );
}
