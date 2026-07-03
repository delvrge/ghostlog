/**
 * Light/dark theme toggle. Purely a local UI preference (localStorage) —
 * unrelated to the AI/watch/output config that lives in Rust's config.json.
 * Applied via a `data-theme` attribute on <html>, which theme.css keys off.
 */
export type Theme = "dark" | "light";

const STORAGE_KEY = "ghlg-theme";

/** Light is the default look; once the user picks a theme it's remembered. */
export function getTheme(): Theme {
  const stored = localStorage.getItem(STORAGE_KEY);
  return stored === "dark" ? "dark" : "light";
}

export function applyTheme(theme: Theme): void {
  document.documentElement.setAttribute("data-theme", theme);
  localStorage.setItem(STORAGE_KEY, theme);
}
