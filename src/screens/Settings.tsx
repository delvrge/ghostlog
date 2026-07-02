/**
 * Settings — watched folder, trigger toggles, extension status,
 * launch-at-login, about, and the presentational-only "GHLG Pro" section.
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
      className={`relative w-10 h-5.5 rounded-full transition-colors shrink-0 ${
        disabled ? "bg-edge cursor-not-allowed opacity-50" : checked ? "bg-accent" : "bg-edge-strong"
      }`}
    >
      <span
        className={`absolute top-0.5 h-4.5 w-4.5 rounded-full bg-fg transition-transform ${
          checked ? "translate-x-[1.15rem]" : "translate-x-0.5"
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

  useEffect(() => {
    invoke<boolean>("is_git_hook_enabled").then(setGitHook).catch(() => {});
    autostartEnabled().then(setLaunchAtLogin).catch(() => {});
    invoke<string>("get_extension_status").then((s) => setExtensionStatus(s as "connected" | "disconnected"));
    getVersion().then(setVersion);
  }, []);

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
          GHLG Pro
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
