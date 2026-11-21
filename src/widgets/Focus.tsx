import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { clock } from "../lib/media";
import { pollActive } from "../lib/active";

export type Focus = {
  kind: "" | "focus" | "break" | "timer";
  label: string;
  total: number;
  ends_at: number | null;
  paused_left: number | null;
  done_at: number | null;
  today: number;
};

export const secsLeft = (f: Focus, now = Date.now()) =>
  f.ends_at ? Math.max(0, Math.ceil((f.ends_at - now) / 1000)) : f.paused_left ?? 0;

/** The shared countdown, re-rendered every second while it runs. */
export function useFocus() {
  const [f, setF] = useState<Focus | null>(null);
  const [, tick] = useState(0);
  useEffect(() => {
    const load = () => invoke<Focus>("focus_get").then(setF).catch(() => {});
    load();
    const off = listen("focus-changed", load);
    return () => {
      off.then((u) => u());
    };
  }, []);
  useEffect(() => {
    if (!f?.ends_at) return;
    const stop = pollActive(1000, () => tick((n) => n + 1));
    return () => stop();
  }, [f?.ends_at]);
  return f;
}

export function FocusRing({ f, size = 22, stroke = 3 }: { f: Focus | null; size?: number; stroke?: number }) {
  const r = 11 - stroke / 2 - 0.5;
  const c = 2 * Math.PI * r;
  const p = f && f.kind && f.total ? secsLeft(f) / f.total : 0;
  return (
    <svg viewBox="0 0 22 22" width={size} height={size} className={`fring ${f?.kind || "idle"}`} aria-hidden="true">
      <circle cx="11" cy="11" r={r} fill="none" stroke="currentColor" strokeOpacity=".2" strokeWidth={stroke} />
      {p > 0 && (
        <circle cx="11" cy="11" r={r} fill="none" stroke="currentColor" strokeWidth={stroke} strokeLinecap="round" strokeDasharray={`${p * c} ${c}`}
          transform="rotate(-90 11 11)" />
      )}
      {!f?.kind && size < 40 && <circle cx="11" cy="11" r="2.4" fill="currentColor" />}
    </svg>
  );
}

const NAMES = { focus: "Focus", break: "Break", timer: "Timer", "": "Focus" };

/** Focus timer: idle it's a button, running it shows the time left; click for presets and controls. */
export default function FocusCell({ settings }: WidgetProps) {
  const f = useFocus();
  const vertical = settings.edge === "left" || settings.edge === "right";
  const running = !!f?.kind;
  const done = !running && f?.done_at && Date.now() - f.done_at < 2 * 60_000;
  // Stop pulsing after two minutes (nothing else re-renders an idle cell).
  const [, rerender] = useState(0);
  useEffect(() => {
    if (!done || !f?.done_at) return;
    const t = window.setTimeout(() => rerender((n) => n + 1), f.done_at + 2 * 60_000 - Date.now() + 50);
    return () => clearTimeout(t);
  }, [done, f?.done_at]);
  return (
    <button
      className={`cell focus${running ? " on" : ""}${f?.paused_left ? " paused" : ""}${done ? " done" : ""}`}
      title={running ? `${f!.label || NAMES[f!.kind]}: ${clock(secsLeft(f!))} left${f!.paused_left ? " (paused)" : ""}` : "Focus timer"}
      onClick={(e) => openAt("focus", e, vertical)}
    >
      <FocusRing f={f} />
      {!vertical && (
        <span className="wtext">
          <small>{running ? f!.label || NAMES[f!.kind] : done ? "Done" : "Focus"}</small>
          {running && <b>{clock(secsLeft(f!))}</b>}
          {!running && f && f.today > 0 && <b>{f.today} today</b>}
        </span>
      )}
    </button>
  );
}
