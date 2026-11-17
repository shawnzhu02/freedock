import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";

/** A power glyph (our own drawing). */
export function PowerGlyph({ size = 18 }: { size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" className="pglyph">
      <path d="M12 3.5v8M7.2 6.3a7.5 7.5 0 1 0 9.6 0" />
    </svg>
  );
}

/** Power: opens lock, sleep, sign out, restart and shut down. */
export default function PowerCell({ settings }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  return (
    <button className="cell projects power" title="Power" onClick={(e) => openAt("power", e, vertical)}>
      <PowerGlyph />
    </button>
  );
}
