import { invoke } from "@tauri-apps/api/core";
import type { WidgetProps } from "./registry";

/** Snip & ask: Windows' snipping overlay, then Quick Ask opens with the snip attached. */
export default function Snip(_: WidgetProps) {
  return (
    <button className="cell projects" title="Snip & ask: drag over part of the screen, then ask Claude about it" onClick={() => invoke("snip_start")}>
      <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" className="pglyph">
        <path d="M3.5 8V5.5a2 2 0 0 1 2-2H8M16 3.5h2.5a2 2 0 0 1 2 2V8M3.5 16v2.5a2 2 0 0 0 2 2H8" />
        <path d="M17 13.5l.9 2.1 2.1.9-2.1.9-.9 2.1-.9-2.1-2.1-.9 2.1-.9z" fill="currentColor" stroke="none" />
      </svg>
    </button>
  );
}
