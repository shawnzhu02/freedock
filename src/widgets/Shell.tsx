import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { isActive, onWake } from "../lib/active";
import type { WidgetProps } from "./registry";

// Small buttons for Windows' own shell features (they press Windows' shortcuts; see shell_key).

/** Show desktop: a thin strip, best at the end of the dock like the classic corner. Click again to
 *  bring the windows back. */
export function ShowDesktop() {
  return <button className="cell showdesk" title="Show desktop" aria-label="Show desktop" onClick={() => invoke("shell_key", { which: "desktop" })} />;
}

export function TaskView() {
  return (
    <button className="cell shellbtn" title="Task View" aria-label="Task View" onClick={() => invoke("shell_key", { which: "taskview" })}>
      <svg viewBox="0 0 20 20" width="19" height="19" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.6">
        <rect x="2" y="4" width="11" height="9" rx="2" />
        <path d="M6 16.5h9.5a2 2 0 0 0 2-2V7.5" />
      </svg>
    </button>
  );
}

export function Search() {
  return (
    <button className="cell shellbtn" title="Search" aria-label="Search" onClick={() => invoke("shell_key", { which: "search" })}>
      <svg viewBox="0 0 20 20" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8">
        <circle cx="8.5" cy="8.5" r="5.5" />
        <path d="M12.6 12.6L17 17" strokeLinecap="round" />
      </svg>
    </button>
  );
}

/** Keyboard language ("EN"); click for the next layout. Hidden while only one layout is installed. */
export function Keyboard(_: WidgetProps) {
  const [k, setK] = useState<[string, number] | null>(null);
  useEffect(() => {
    const get = () => invoke<[string, number]>("keyboard_layout").then(setK).catch(() => {});
    get();
    // The layout belongs to the window in front, so it changes as the user switches windows.
    const t = window.setInterval(() => isActive() && get(), 1500);
    const off = onWake(get);
    return () => {
      clearInterval(t);
      off();
    };
  }, []);
  if (!k || k[1] < 2) return null;
  return (
    <button
      className="cell kbd"
      title="Keyboard language (click or Win+Space for the next)"
      aria-label={`Keyboard language ${k[0]}. Switch`}
      onClick={() => {
        invoke("shell_key", { which: "language" });
        setTimeout(() => invoke<[string, number]>("keyboard_layout").then(setK).catch(() => {}), 350);
      }}
    >
      {k[0]}
    </button>
  );
}
