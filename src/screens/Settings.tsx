/**
 * Settings — watched folder, trigger toggles, extension status,
 * launch-at-login, and about.
 */
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/plugin-dialog";
import { isEnabled as autostartEnabled, enable as autostartEnable, disable as autostartDisable } from "@tauri-apps/plugin-autostart";
import { applyTheme, getTheme, type Theme } from "../lib/theme";
import { addWatchedFolder, removeWatchedFolder, type WatchedProject } from "../lib/watcher";

function Toggle({ checked, onChange, disabled }: { checked: boolean; onChange: () => void; disabled?: boolean }) {
  return (
    <button
      onClick={onChange}
      disabled={disabled}
      className={`relative w-10 h-6 rounded-full transition-colors shrink-0 ${
        disabled
          ? "bg-edge cursor-not-allowed opacity-50"
          : checked
            ? "bg-accent"
            : "bg-edge-strong"
      }`}
    >
      {/* Fixed white knob with a soft shadow reads correctly against both the
          red "on" track and the neutral "off" track, in either theme. */}
      <span
        className={`absolute top-1 left-1 h-4 w-4 rounded-full bg-white shadow transition-transform ${
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
  folders,
  onFoldersChanged,
}: {
  folders: WatchedProject[];
  onFoldersChanged: () => void;
}) {
  const [manualTrigger, setManualTrigger] = useState(true);
  const [gitHook, setGitHook] = useState(false);
  const [shellHook, setShellHook] = useState(false);
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [runInBackground, setRunInBackgroundState] = useState(true);
  const [theme, setTheme] = useState<Theme>(getTheme());
  const [extensionStatus, setExtensionStatus] = useState<"connected" | "disconnected">("disconnected");
  const [nativeHostInstalled, setNativeHostInstalled] = useState(false);
  const [version, setVersion] = useState("");
  const [error, setError] = useState<string | null>(null);

  const [aiEndpoint, setAiEndpoint] = useState("");
  const [aiModel, setAiModel] = useState("");
  const [aiVisionEndpoint, setAiVisionEndpoint] = useState("");
  const [aiVisionModel, setAiVisionModel] = useState("");
  const [aiSaved, setAiSaved] = useState(false);

  const [outputFolder, setOutputFolder] = useState<string | null>(null);
  const [dataRoot, setDataRoot] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>("is_git_hook_enabled").then(setGitHook).catch(() => {});
    invoke<boolean>("is_shell_hook_installed").then(setShellHook).catch(() => {});
    invoke<boolean>("get_run_in_background").then(setRunInBackgroundState).catch(() => {});
    invoke<string>("get_data_root").then(setDataRoot).catch(() => {});
    autostartEnabled().then(setLaunchAtLogin).catch(() => {});
    invoke<string>("get_extension_status").then((s) => setExtensionStatus(s as "connected" | "disconnected"));
    invoke<boolean>("is_native_host_installed").then(setNativeHostInstalled).catch(() => {});
    getVersion().then(setVersion);
    invoke<{ endpoint: string; model: string; visionEndpoint: string; visionModel: string }>(
      "get_ai_config",
    ).then((cfg) => {
      setAiEndpoint(cfg.endpoint);
      setAiModel(cfg.model);
      setAiVisionEndpoint(cfg.visionEndpoint);
      setAiVisionModel(cfg.visionModel);
    });
    invoke<string | null>("get_output_folder").then(setOutputFolder);
  }, []);

  async function chooseDataRoot() {
    setError(null);
    const selection = await open({ directory: true, multiple: false, title: "Choose where Ghostlog stores captured entries and screenshots" });
    if (typeof selection !== "string") return;
    try {
      await invoke("set_data_root", { path: selection });
      setDataRoot(selection);
    } catch (e) {
      setError(String(e));
    }
  }

  async function chooseOutputFolder() {
    setError(null);
    const selection = await open({ directory: true, multiple: false, title: "Choose where Ghostlog exports documents" });
    if (typeof selection !== "string") return;
    try {
      await invoke("set_output_folder", { path: selection });
      setOutputFolder(selection);
    } catch (e) {
      setError(String(e));
    }
  }

  async function saveAiConfig() {
    setError(null);
    try {
      await invoke("set_ai_config", {
        endpoint: aiEndpoint.trim(),
        model: aiModel.trim(),
        visionEndpoint: aiVisionEndpoint.trim(),
        visionModel: aiVisionModel.trim(),
      });
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

  async function toggleNativeHost() {
    setError(null);
    try {
      if (nativeHostInstalled) await invoke("uninstall_native_host");
      else await invoke("install_native_host");
      setNativeHostInstalled(!nativeHostInstalled);
    } catch (e) {
      setError(String(e));
    }
  }

  async function toggleShellHook() {
    setError(null);
    const next = !shellHook;
    try {
      if (next) await invoke("install_shell_hook");
      else await invoke("uninstall_shell_hook");
      setShellHook(next);
    } catch (e) {
      setError(String(e));
    }
  }

  function toggleTheme() {
    const next: Theme = theme === "dark" ? "light" : "dark";
    applyTheme(next);
    setTheme(next);
  }

  async function toggleRunInBackground() {
    setError(null);
    const next = !runInBackground;
    try {
      await invoke("set_run_in_background", { enabled: next });
      setRunInBackgroundState(next);
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

  async function addFolder() {
    setError(null);
    const selection = await open({ directory: true, multiple: false, title: "Choose a project folder to watch" });
    if (typeof selection !== "string") return;
    try {
      await addWatchedFolder(selection);
      onFoldersChanged();
    } catch (e) {
      setError(String(e));
    }
  }

  async function stopWatchingFolder(path: string) {
    setError(null);
    try {
      await removeWatchedFolder(path);
      onFoldersChanged();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="max-w-xl space-y-8">
      <section>
        <h2 className="text-xs text-fg-faint uppercase tracking-wide mb-2">Watched projects</h2>
        <div className="bg-panel border border-edge rounded-lg px-4 divide-y divide-edge">
          {folders.map((f) => (
            <div key={f.path} className="flex items-center justify-between gap-4 py-3">
              <div className="min-w-0">
                <p className="text-sm">{f.name}</p>
                <p className="font-mono text-xs text-fg-faint break-all">{f.path}</p>
              </div>
              <button
                onClick={() => stopWatchingFolder(f.path)}
                className="shrink-0 text-xs text-fg-faint hover:text-accent hover:bg-accent/10 px-2 py-1 rounded-md transition-colors"
              >
                Remove
              </button>
            </div>
          ))}
          <div className="py-3">
            <button
              onClick={addFolder}
              className="text-xs text-fg-muted hover:text-fg border border-edge-strong hover:border-fg-muted px-3 py-1.5 rounded-md transition-colors"
            >
              Add project…
            </button>
          </div>
        </div>
        <p className="text-xs text-fg-faint mt-1.5">
          Each entry must be the root folder of a git repository. Sessions are archived per
          project; removing a project here stops watching it but keeps its archive.
        </p>
      </section>

      <section>
        <h2 className="text-xs text-fg-faint uppercase tracking-wide mb-2">Capture data folder</h2>
        <div className="bg-panel border border-edge rounded-lg px-4 py-3 flex items-center justify-between gap-4">
          <span className="font-mono text-sm break-all text-fg-muted">
            {dataRoot ?? "Loading…"}
          </span>
          <button
            onClick={chooseDataRoot}
            className="shrink-0 text-xs text-fg-muted hover:text-fg border border-edge-strong hover:border-fg-muted px-3 py-1.5 rounded-md transition-colors"
          >
            Change
          </button>
        </div>
        <p className="text-xs text-fg-faint mt-1.5">
          Every entry, note, and screenshot Ghostlog captures — for every project — lives here as
          plain files, organized project / date / session. Nothing here ever leaves this folder;
          changing it moves your existing history to the new location.
        </p>
      </section>

      <section>
        <h2 className="text-xs text-fg-faint uppercase tracking-wide mb-2">Output folder</h2>
        <div className="bg-panel border border-edge rounded-lg px-4 py-3 flex items-center justify-between gap-4">
          <span className="font-mono text-sm break-all text-fg-muted">
            {outputFolder ?? "Not set — choose where compiled documents get exported"}
          </span>
          <button
            onClick={chooseOutputFolder}
            className="shrink-0 text-xs text-fg-muted hover:text-fg border border-edge-strong hover:border-fg-muted px-3 py-1.5 rounded-md transition-colors"
          >
            {outputFolder ? "Change" : "Choose…"}
          </button>
        </div>
        <p className="text-xs text-fg-faint mt-1.5">
          The only place Ghostlog writes files you asked for — exporting a compiled document is the one thing it can
          save outside its own app data.
        </p>
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
          <Row
            title="Shell error trigger"
            description="Capture automatically when a terminal command fails — works no matter who runs the command, you or an AI tool"
          >
            <Toggle checked={shellHook} onChange={toggleShellHook} />
          </Row>
        </div>
      </section>

      <section>
        <h2 className="text-xs text-fg-faint uppercase tracking-wide mb-1">Browser extension</h2>
        <div className="bg-panel border border-edge rounded-lg px-4 divide-y divide-edge">
          <Row title="Native messaging host" description="Lets the Ghostlog extension trigger captures — no network ports involved">
            <button
              onClick={toggleNativeHost}
              className="shrink-0 text-xs text-fg-muted hover:text-fg border border-edge-strong hover:border-fg-muted px-3 py-1.5 rounded-md transition-colors"
            >
              {nativeHostInstalled ? "Uninstall" : "Install"}
            </button>
          </Row>
          <Row title="Connection">
            <div className="flex items-center gap-2">
              <span
                className={`h-2 w-2 rounded-full ${extensionStatus === "connected" ? "bg-accent" : "bg-fg-faint"}`}
              />
              <span className="text-sm text-fg-muted capitalize">{extensionStatus}</span>
            </div>
          </Row>
        </div>
      </section>

      <section>
        <h2 className="text-xs text-fg-faint uppercase tracking-wide mb-1">AI provider</h2>
        <div className="bg-panel border border-edge rounded-lg px-4 py-4 space-y-3">
          <p className="text-xs text-fg-muted">
            Ghostlog summarizes captures with a model you point it at — nothing is sent anywhere until you set this.
            Works with a local llama.cpp server (<span className="font-mono text-fg">llama-server</span>, OpenAI-compatible
            API). Leave blank to keep using mock summaries.
          </p>
          <div className="grid grid-cols-[1fr_auto] gap-3">
            <input
              value={aiEndpoint}
              onChange={(e) => setAiEndpoint(e.target.value)}
              placeholder="http://localhost:8080"
              className="bg-ink border border-edge rounded-md px-3 py-2 text-sm font-mono placeholder:text-fg-faint focus:outline-none focus:border-accent"
            />
            <input
              value={aiModel}
              onChange={(e) => setAiModel(e.target.value)}
              placeholder="model label (optional)"
              className="w-48 bg-ink border border-edge rounded-md px-3 py-2 text-sm font-mono placeholder:text-fg-faint focus:outline-none focus:border-accent"
            />
          </div>
          <p className="text-xs text-fg-muted pt-1">
            Vision model (optional) — a second, vision-capable endpoint used only to describe
            the screenshot taken when a browser error is captured. Leave blank to skip
            screenshot analysis; the screenshot itself is always saved.
          </p>
          <div className="grid grid-cols-[1fr_auto] gap-3">
            <input
              value={aiVisionEndpoint}
              onChange={(e) => setAiVisionEndpoint(e.target.value)}
              placeholder="http://localhost:8081"
              className="bg-ink border border-edge rounded-md px-3 py-2 text-sm font-mono placeholder:text-fg-faint focus:outline-none focus:border-accent"
            />
            <input
              value={aiVisionModel}
              onChange={(e) => setAiVisionModel(e.target.value)}
              placeholder="model label (optional)"
              className="w-48 bg-ink border border-edge rounded-md px-3 py-2 text-sm font-mono placeholder:text-fg-faint focus:outline-none focus:border-accent"
            />
          </div>
          <button
            onClick={saveAiConfig}
            className="text-sm bg-accent hover:bg-accent-dim text-white px-4 py-2 rounded-md transition-colors"
          >
            {aiSaved ? "Saved" : "Save"}
          </button>
        </div>
      </section>

      <section>
        <h2 className="text-xs text-fg-faint uppercase tracking-wide mb-1">General</h2>
        <div className="bg-panel border border-edge rounded-lg px-4 divide-y divide-edge">
          <Row title="Theme" description={theme === "dark" ? "Dark" : "Light"}>
            <button
              onClick={toggleTheme}
              className="text-sm border border-edge-strong hover:border-fg-muted text-fg-muted hover:text-fg px-3 py-1.5 rounded-md transition-colors"
            >
              Switch to {theme === "dark" ? "light" : "dark"}
            </button>
          </Row>
          <Row title="Launch at login">
            <Toggle checked={launchAtLogin} onChange={toggleLaunchAtLogin} />
          </Row>
          <Row
            title="Run in background"
            description="Closing the window keeps Ghostlog watching in the tray instead of quitting — use the tray menu's Quit to actually exit"
          >
            <Toggle checked={runInBackground} onChange={toggleRunInBackground} />
          </Row>
          <Row title="Version">
            <span className="text-sm font-mono text-fg-muted">{version}</span>
          </Row>
        </div>
      </section>

      {error && <p className="text-sm text-accent font-mono">{error}</p>}
    </div>
  );
}
