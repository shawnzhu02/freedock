import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { appName, dur, type StDay } from "../lib/screentime";
import { pollActive } from "../lib/active";

/** Screen Time: today's active time at the PC; the flyout has programs and the week. */
export default function ScreenTime({ settings }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  const [today, setToday] = useState<StDay | null>(null);
  useEffect(() => {
    invoke("screentime_watch");
    const get = () => invoke<StDay[]>("screentime_get", { days: 1, icons: false }).then((d) => setToday(d[0] ?? null)).catch(() => {});
    get();
    const stop = pollActive(60_000, () => get());
    return () => stop();
  }, []);
  const top = today?.apps[0]?.[0];
  return (
    <button
      className="cell stat ctl stime"
      title={today ? `Screen time today: ${dur(today.total)}${top ? `, most in ${appName(top.name)}` : ""}` : "Screen time"}
      onClick={(e) => openAt("screentime", e, vertical)}
    >
      <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" className="pglyph">
        <path d="M7 3.5h10M7 20.5h10M8 3.5v3a4 4 0 0 0 1.6 3.2L12 12l2.4-1.8A4 4 0 0 0 16 7V3.5M8 20.5v-3a4 4 0 0 1 1.6-3.2L12 12l2.4 2.3a4 4 0 0 1 1.6 3.2v3" />
      </svg>
      {!vertical && <b className="cval">{today ? dur(today.total) : "0m"}</b>}
    </button>
  );
}
