/**
 * Settings — watched folder, trigger toggles, extension status,
 * launch-at-login, about, and the presentational-only "Ghostlog Pro" section.
 *
 * The Pro section below is disabled and purely visual per CLAUDE.md: it
 * must never import from src/pro-stub/ or wire up real logic.
 */
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { isEnabled as autostartEnabled, enable as autostartEnable, disable as autostartDisable } from "@tauri-apps/plugin-autostart";

function Toggle({ checked, onChange, disabled }: { checked: boolean; onChange: () => void; disabled?: boolean }) {
  return (
    <button
      onClick={onChange}
      disabled={disabled}
      className={`relative w-10 h-6 rounded-full transition-colors shrink-0 ${
        disabled ? "bg-edge cursor-not-allowed opacity-50" : checked ? "bg-accent" : "bg-edge-strong"
      }`}
    >
      <span
        className={`absolute top-1 left-1 h-4 w-4 rounded-full bg-fg transition-transform ${
          checked ? "translate-x-4" : "translate-x-0"
        }`}
      />
    </button>
  );
}

function Row({ title, description, children }: { title: string; description?: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="min-w-0">
        <p className="text-sm">{title}</p>
        {description && <p className="text-xs text-fg-muted mt-0.5">{description}</p>}
      </div>
      {children}
    </div>
  );
}

export default function Settings({
  watchedFolder,
  onChangeFolder,
}: {
  watchedFolder: string;
  onChangeFolder: () => void;
}) {
  const [manualTrigger, setManualTrigger] = useState(true);
  const [gitHook, setGitHook] = useState(false);
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [extensionStatus, setExtensionStatus] = useState<"connected" | "disconnected">("disconnected");
  const [version, setVersion] = useState("");
  const [error, setError] = useState<string | null>(null);

  const [aiEndpoint, setAiEndpoint] = useState("");
  const [aiModel, setAiModel] = useState("");
  const [aiSaved, setAiSaved] = useState(false);

  useEffect(() => {
    invoke<boolean>("is_git_hook_enabled").then(setGitHook).catch(() => {});
    autostartEnabled().then(setLaunchAtLogin).catch(() => {});
    invoke<string>("get_extension_status").then((s) => setExtensionStatus(s as "connected" | "disconnected"));
    getVersion().then(setVersion);
    invoke<{ endpoint: string; model: string }>("get_ai_config").then((cfg) => {
      setAiEndpoint(cfg.endpoint);
      setAiModel(cfg.model);
    });
  }, []);

  async function saveAiConfig() {
    setError(null);
    try {
      await invoke("set_ai_config", { endpoint: aiEndpoint.trim(), model: aiModel.trim() });
      setAiSaved(true);
      setTimeout(() => setAiSaved(false), 1500);
    } catch (e) {
      setError(String(e));
    }
  }

  async function toggleGitHook() {
    setError(null);
    const next = !gitHook;
    try {
      await invoke("set_git_hook_enabled", { enabled: next });
      setGitHook(next);
    } catch (e) {
      setError(String(e));
    }
  }

  async function toggleLaunchAtLogin() {
    setError(null);
    try {
      if (launchAtLogin) await autostartDisable();
      else await autostartEnable();
      setLaunchAtLogin(!launchAtLogin);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="max-w-xl space-y-8">
      <section>
        <h2 className="text-xs text-fg-faint uppercase tracking-wide mb-2">Watched folder</h2>
        <div className="bg-panel border border-edge rounded-lg px-4 py-3 flex items-center justify-between gap-4">
          <span className="font-mono text-sm break-all">{watchedFolder}</span>
          <button
            onClick={onChangeFolder}
            className="shrink-0 text-xs text-fg-muted hover:text-fg border border-edge-strong hover:border-fg-muted px-3 py-1.5 rounded-md transition-colors"
          >
            Change
          </button>
        </div>
      </section>

      <section>
        <h2 className="text-xs text-fg-faint uppercase tracking-wide mb-1">Triggers</h2>
        <div className="bg-panel border border-edge rounded-lg px-4 divide-y divide-edge">
          <Row title="Manual trigger" description='"Log this now" from the home view'>
            <Toggle checked={manualTrigger} onChange={() => setManualTrigger((v) => !v)} />
          </Row>
          <Row title="Git commit trigger" description="Capture automatically on every commit in this project">
            <Toggle checked={gitHook} onChange={toggleGitHook} />
          </Row>
          <Row title="Automatic error detection" description="Coming soon">
            <Toggle checked={false} onChange={() => {}} disabled />
          </Row>
        </div>
      </section>

      <section>
        <h2 className="text-xs text-fg-faint uppercase tracking-wide mb-1">Browser extension</h2>
        <div className="bg-panel border border-edge rounded-lg px-4 py-3 flex items-center gap-2">
          <span
            className={`h-2 w-2 rounded-full ${extensionStatus === "connected" ? "bg-accent" : "bg-fg-faint"}`}
          />
          <span className="text-sm text-fg-muted capitalize">{extensionStatus}</span>
        </div>
      </section>

      <section>
        <h2 className="text-xs text-fg-faint uppercase tracking-wide mb-1">AI provider</h2>
        <div className="bg-panel border border-edge rounded-lg px-4 py-4 space-y-3">
          <p className="text-xs text-fg-muted">
            Ghostlog summarizes captures with a model you point it at — nothing is sent anywhere until you set this.
            Works with any local or self-hosted OpenAI/Ollama-compatible endpoint. Leave blank to keep using mock
            summaries.
          </p>
          <div className="grid grid-cols-[1fr_auto] gap-3">
            <input
              value={aiEndpoint}
              onChange={(e) => setAiEndpoint(e.target.value)}
              placeholder="http://localhost:11434"
              className="bg-ink border border-edge rounded-md px-3 py-2 text-sm font-mono placeholder:text-fg-faint focus:outline-none focus:border-accent"
            />
            <input
              value={aiModel}
              onChange={(e) => setAiModel(e.target.value)}
              placeholder="model name"
              className="w-40 bg-ink border border-edge rounded-md px-3 py-2 text-sm font-mono placeholder:text-fg-faint focus:outline-none focus:border-accent"
            />
          </div>
          <button
            onClick={saveAiConfig}
            className="text-sm bg-accent hover:bg-accent-dim text-white px-4 py-2 rounded-md transition-colors"
          >
            {aiSaved ? "Saved" : "Save"}
          </button>
          {/* Presentational only — no working preset list, no pro-stub import. */}
          <p className="text-xs text-fg-faint pt-1 border-t border-edge">
            Ready-made provider presets are available in Ghostlog Pro.
          </p>
        </div>
      </section>

      <section>
        <h2 className="text-xs text-fg-faint uppercase tracking-wide mb-1">General</h2>
        <div className="bg-panel border border-edge rounded-lg px-4 divide-y divide-edge">
          <Row title="Launch at login">
            <Toggle checked={launchAtLogin} onChange={toggleLaunchAtLogin} />
          </Row>
          <Row title="Version">
            <span className="text-sm font-mono text-fg-muted">{version}</span>
          </Row>
        </div>
      </section>

      {error && <p className="text-sm text-accent font-mono">{error}</p>}

      {/* Presentational only — no imports from src/pro-stub/, no working logic. */}
      <section className="opacity-50">
        <h2 className="text-xs text-fg-faint uppercase tracking-wide mb-1 flex items-center gap-2">
          Ghostlog Pro
        </h2>
        <div className="bg-panel border border-edge rounded-lg px-4 divide-y divide-edge">
          <Row title="Multi-project management" description="Coming soon">
            <span className="text-xs text-fg-faint border border-edge-strong rounded px-2 py-0.5">Pro</span>
          </Row>
          <Row title="License" description="Coming soon">
            <span className="text-xs text-fg-faint border border-edge-strong rounded px-2 py-0.5">Pro</span>
          </Row>
          <Row title="Dashboard" description="Coming soon">
            <span className="text-xs text-fg-faint border border-edge-strong rounded px-2 py-0.5">Pro</span>
          </Row>
        </div>
      </section>
    </div>
  );
}
