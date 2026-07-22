/**
 * Color-coded entry tag badge. Palette rule: red = bug/attention, solid
 * gray = neutral/update, outlined = feature-like, dashed = tentative
 * (experiment/question). No hues outside red/white/gray.
 */
const styles: Record<string, string> = {
  bugfix: "bg-accent/15 text-accent border border-accent/40",
  update: "bg-panel-raised text-fg-muted border border-edge-strong",
  feature: "bg-transparent text-fg border border-fg-muted",
  refactor: "bg-transparent text-fg-muted border border-edge",
  performance: "bg-accent/8 text-accent border border-accent/25",
  ui: "bg-panel text-fg border border-edge-strong",
  configuration: "bg-ink text-fg-faint border border-edge",
  experiment: "bg-transparent text-fg-muted border border-dashed border-fg-muted",
  decision: "bg-fg/10 text-fg border border-fg",
  question: "bg-transparent text-accent border border-dashed border-accent/50",
  note: "bg-transparent text-fg-faint border border-edge",
};

export default function TagBadge({ tag }: { tag: string }) {
  return (
    <span
      className={`inline-block px-2 py-0.5 rounded text-xs font-mono uppercase tracking-wide ${
        styles[tag] ?? styles.update
      }`}
    >
      {tag}
    </span>
  );
}
