import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { FIVE_HOURS, SEVEN_DAYS, level, pct, resetIn, runsOutAt, type Usage, type UsageWindow } from "../lib/usage";
import { pollActive } from "../lib/active";

/** A small ring that fills with the share used. */
export function Ring({ w, size = 22 }: { w: UsageWindow | null; size?: number }) {
  const r = 8.5;
  const c = 2 * Math.PI * r;
  const p = w ? Math.min(1, w.utilization) : 0;
  return (
    <svg viewBox="0 0 22 22" width={size} height={size} className={`ring ${level(w)}`} aria-hidden="true">
      <circle cx="11" cy="11" r={r} fill="none" stroke="currentColor" strokeOpacity=".18" strokeWidth="3" />
      <circle cx="11" cy="11" r={r} fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeDasharray={`${p * c} ${c}`}
        transform="rotate(-90 11 11)" />
    </svg>
  );
}

/** Fetch usage at most `every` minutes old, and pick up readings Quick Ask gets for free. */
export function useUsage(everyMin: number) {
  const [u, setU] = useState<Usage | null>(null);
  const [err, setErr] = useState("");
  useEffect(() => {
    const get = (maxAge: number) =>
      invoke<Usage>("claude_usage", { maxAge })
        .then((x) => {
          setU(x);
          setErr("");
        })
        .catch((e) => setErr(String(e)));
    get(everyMin * 60);
    const stop = pollActive(60_000, () => get(everyMin * 60));
    const off = listen("usage-changed", () => get(1e9));
    return () => {
      stop();
      off.then((f) => f());
    };
  }, [everyMin]);
  return { u, err };
}

/** Claude's 5-hour and weekly limits as two rings with the busier percentage; a click shows details. */
export default function ClaudeUsage({ settings, options }: WidgetProps) {
  const { u, err } = useUsage(Number(options.every ?? 15));
  const vertical = settings.edge === "left" || settings.edge === "right";
  const five = u?.five_hour ?? null;
  const week = u?.seven_day ?? null;
  const worst = [five, week].reduce<UsageWindow | null>((a, b) => (b && (!a || b.utilization > a.utilization) ? b : a), null);
  // At this pace, when would a limit run out (before it resets)?
  const nowS = Date.now() / 1000;
  const out5 = runsOutAt(five, FIVE_HOURS, nowS);
  const outW = runsOutAt(week, SEVEN_DAYS, nowS);
  const outText = out5 ? `. At this pace the 5-hour limit runs out ${resetIn(out5, nowS)}` : outW ? `. At this pace the weekly limit runs out ${resetIn(outW, nowS)}` : "";
  return (
    <button
      className={`cell usage ${level(worst)}${out5 || outW ? " pace" : ""}`}
      title={err || (u ? `Claude: 5-hour ${pct(five)}%, week ${pct(week)}%${outText}` : "Claude usage")}
      onClick={(e) => openAt("usage", e, vertical)}
    >
      <Ring w={five} />
      {!vertical && (
        <span className="utext">
          <small>Claude</small>
          <b>
            {u ? `${pct(five)}%` : err ? "—" : "…"}
            <i> · wk {u ? `${pct(week)}%` : ""}</i>
          </b>
        </span>
      )}
    </button>
  );
}
