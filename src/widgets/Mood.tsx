import { invoke } from "@tauri-apps/api/core";
import { useRef } from "react";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { activeMood } from "../lib/moods";

/** Mood switcher: the active mood's icon (and name, if the option is on). Click lists the moods;
 *  scrolling over it steps through them. */
export default function MoodCell({ settings, options }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  const m = activeMood(settings);
  const last = useRef(0);
  return (
    <button
      className="cell mood"
      title={`Mood: ${m?.name ?? ""} (click to switch, scroll to step through)`}
      aria-label={`Mood: ${m?.name ?? ""}. Switch mood`}
      onClick={(e) => openAt("moods", e, vertical)}
      onWheel={(e) => {
        // One step per notch, at most every 250 ms, so a fast scroll doesn't skip moods.
        if (Date.now() - last.current < 250 || e.deltaY === 0) return;
        last.current = Date.now();
        invoke("cycle_mood", { step: e.deltaY > 0 ? 1 : -1 }).catch(() => {});
      }}
    >
      <span aria-hidden="true">{m?.icon || "\u25cf"}</span>
      {!!options.name && <b>{m?.name}</b>}
    </button>
  );
}
