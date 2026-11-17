import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";

/** Opens the capture flyout: type a to-do and it lands on your list. */
export default function Capture({ settings, options }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  return (
    <button className="cell projects" title="Add a to-do" onClick={(e) => openAt("capture", e, vertical)}>
      <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" className="pglyph">
        <rect x="4" y="4" width="16" height="16" rx="4" />
        <path d="M12 8.5v7M8.5 12h7" />
      </svg>
      {!vertical && options.label !== false && <span>To-do</span>}
    </button>
  );
}
