import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";

export function ClipGlyph({ size = 20 }: { size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" className="pglyph">
      <rect x="5" y="4.5" width="14" height="16.5" rx="2.2" />
      <path d="M9 4.5V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v.5M8.5 10h7M8.5 13.5h7M8.5 17h4" />
    </svg>
  );
}

/** Clipboard history: watching runs only while this cell exists, and history lives in memory only. */
export default function Clipboard({ settings }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  useEffect(() => {
    invoke("clip_watch", { on: true }).catch(() => {});
    return () => {
      invoke("clip_watch", { on: false }).catch(() => {});
    };
  }, []);
  return (
    <button className="cell projects" title="Clipboard history" onClick={(e) => openAt("clipboard", e, vertical)}>
      <ClipGlyph />
    </button>
  );
}
