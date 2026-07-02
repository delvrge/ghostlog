/**
 * First-run onboarding: pick the ONE watched folder (free tier), confirm the
 * exact path, then connect the browser extension.
 * The folder choice is validated and persisted on the Rust side.
 */
import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

type Step = "pick" | "confirm" | "extension";

export default function Onboarding({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState<Step>("pick");
  const [picked, setPicked] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function pickFolder() {
    setError(null);
    const selection = await open({ directory: true, multiple: false, title: "Choose the project folder Ghostlog will watch" });
    if (typeof selection === "string") {
      setPicked(selection);
      setStep("confirm");
    }
  }

  async function confirmFolder() {
    if (!picked) return;
    try {
      await invoke("set_watched_folder", { path: picked });
      setStep("extension");
    } catch (e) {
      setError(String(e));
      setStep("pick");
    }
  }

  return (
    <main className="min-h-screen bg-ink text-fg font-sans flex items-center justify-center p-8">
      <div className="w-full max-w-lg space-y-6">
        <header className="space-y-1">
          <h1 className="text-xl font-semibold tracking-tight">
            Welcome to Ghostlog<span className="text-accent">.</span>
          </h1>
          <p className="text-fg-muted text-sm">Quiet capture while you build. Review whenever you want.</p>
        </header>

        {step === "pick" && (
          <section className="bg-panel border border-edge rounded-lg p-6 space-y-4">
            <h2 className="font-medium">Choose your project folder</h2>
            <p className="text-sm text-fg-muted">
              Ghostlog only reads inside this folder.{" "}
              <span className="text-fg">Nothing else on your machine is accessed.</span>
            </p>
            {error && <p className="text-sm text-accent font-mono">{error}</p>}
            <button
              onClick={pickFolder}
              className="bg-accent hover:bg-accent-dim text-white text-sm font-medium px-4 py-2 rounded-md transition-colors"
            >
              Select folder…
            </button>
          </section>
        )}

        {step === "confirm" && picked && (
          <section className="bg-panel border border-edge rounded-lg p-6 space-y-4">
            <h2 className="font-medium">Confirm the watched folder</h2>
            <p className="text-sm text-fg-muted">Ghostlog will watch exactly this path — and nothing outside it:</p>
            <code className="block bg-ink border border-edge rounded-md px-3 py-2 font-mono text-sm break-all">
              {picked}
            </code>
            <div className="flex gap-3">
              <button
                onClick={confirmFolder}
                className="bg-accent hover:bg-accent-dim text-white text-sm font-medium px-4 py-2 rounded-md transition-colors"
              >
                Yes, watch this folder
              </button>
              <button
                onClick={() => setStep("pick")}
                className="border border-edge-strong hover:border-fg-muted text-fg-muted hover:text-fg text-sm px-4 py-2 rounded-md transition-colors"
              >
                Choose a different one
              </button>
            </div>
          </section>
        )}

        {step === "extension" && (
          <section className="bg-panel border border-edge rounded-lg p-6 space-y-4">
            <h2 className="font-medium">Connect the browser extension</h2>
            <p className="text-sm text-fg-muted">
              The Ghostlog extension captures screenshots of <span className="font-mono text-fg">localhost</span> while
              you test. Its permissions are limited to{" "}
              <span className="font-mono text-fg">localhost / 127.0.0.1</span> — it cannot see any other site, and it
              talks to Ghostlog through the browser's native messaging channel, never over the network.
            </p>
            <p className="text-sm text-fg-faint">
              Installation instructions will appear here once the extension build lands. You can connect it later from
              Settings.
            </p>
            <button
              onClick={onDone}
              className="bg-accent hover:bg-accent-dim text-white text-sm font-medium px-4 py-2 rounded-md transition-colors"
            >
              Finish setup
            </button>
          </section>
        )}

        <footer className="flex gap-2 justify-center">
          {(["pick", "confirm", "extension"] as Step[]).map((s) => (
            <span
              key={s}
              className={`h-1.5 w-6 rounded-full ${s === step ? "bg-accent" : "bg-edge-strong"}`}
            />
          ))}
        </footer>
      </div>
    </main>
  );
}
