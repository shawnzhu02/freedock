import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { isActive } from "../lib/active";

export type Session = { project: string; cwd: string; state: "working" | "waiting" | "stuck"; idle: number };

/** Claude Code sessions from the last 3 hours, re-read every `everyMs` while visible. */
export function useSessions(everyMs: number, key = 0) {
  const [s, setS] = useState<Session[] | null>(null);
  useEffect(() => {
    const get = () => invoke<Session[]>("claude_sessions").then(setS).catch(() => setS([]));
    get();
    const t = window.setInterval(() => isActive() && get(), everyMs);
    return () => clearInterval(t);
  }, [everyMs, key]);
  return s;
}

/** Only sessions the user would care about right now: active in the last `freshMin` minutes (older ones were
 *  most likely closed). */
export function relevant(s: Session[], freshMin = 60): Session[] {
  return s.filter((x) => x.idle < freshMin * 60);
}

/** Claude sessions: how many are working, and how many finished and are waiting on you (or stuck on
 *  an approval). Click for the list. */
export default function SessionsCell({ settings }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  const all = useSessions(15_000);
  const s = relevant(all ?? []);
  const working = s.filter((x) => x.state === "working").length;
  const waiting = s.filter((x) => x.state !== "working").length;
  const tip = !all
    ? "Claude sessions"
    : s.length
      ? s.map((x) => `${x.project}: ${x.state === "working" ? "working" : x.state === "stuck" ? "needs you (approval?)" : "done, your turn"}`).join("\n")
      : "No Claude sessions in the last hour";
  return (
    <button className={`cell stat ctl sessc${waiting ? " attn" : ""}`} title={tip} onClick={(e) => openAt("sessions", e, vertical)}>
      <span className={`sspin${working ? " on" : ""}`} aria-hidden="true" />
      {!vertical ? (
        <b className="cval">
          {working}
          {waiting > 0 && <i className="swait"> · {waiting} waiting</i>}
        </b>
      ) : (
        waiting > 0 && <i className="sbadge">{waiting}</i>
      )}
    </button>
  );
}
