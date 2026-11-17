import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";

/** Calculator glyph (our own drawing). */
function CalcGlyph() {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" className="pglyph">
      <rect x="5" y="3" width="14" height="18" rx="3" />
      <path d="M8.5 7.5h7M8.5 12h.01M12 12h.01M15.5 12h.01M8.5 16h.01M12 16h.01M15.5 16h.01" strokeWidth="2.2" />
    </svg>
  );
}

/** Calculator: opens a small calculator flyout. */
export default function CalcCell({ settings }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  return (
    <button className="cell projects calc" title="Calculator" onClick={(e) => openAt("calc", e, vertical)}>
      <CalcGlyph />
    </button>
  );
}
