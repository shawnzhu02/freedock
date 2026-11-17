import { invoke } from "@tauri-apps/api/core";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";

/** Start: freedock's own Start menu, or Windows' (the widget's "Opens" option). Our own four-pane glyph. */
export default function Start({ settings, options }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  const windows = options.opens === "windows";
  return (
    <button
      className="cell start"
      title="Start"
      aria-label="Start"
      onClick={(e) => (windows ? invoke("start_menu") : openAt("start", e, vertical))}
    >
      <svg viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <rect x="1" y="1" width="8.2" height="8.2" rx="1.6" />
        <rect x="10.8" y="1" width="8.2" height="8.2" rx="1.6" />
        <rect x="1" y="10.8" width="8.2" height="8.2" rx="1.6" />
        <rect x="10.8" y="10.8" width="8.2" height="8.2" rx="1.6" />
      </svg>
    </button>
  );
}
