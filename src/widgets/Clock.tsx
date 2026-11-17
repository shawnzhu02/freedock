import type { MouseEvent } from "react";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { useNow } from "./useNow";

// Like the real taskbar, the clock opens a calendar (ours, with today's schedule from your calendars).
const openCalendar = (e: MouseEvent<HTMLElement>, s: WidgetProps["settings"]) => openAt("calendar", e, s.edge === "left" || s.edge === "right");

/** The day and date: a small weekday in the accent colour over "Nov 15". */
export function DateCell({ settings, options }: WidgetProps) {
  const now = useNow();
  const weekday = options.weekday !== false;
  if (settings.edge === "left" || settings.edge === "right") {
    // Upright dock: "SUN" over "15".
    return (
      <button className="cell date" title={now.toLocaleDateString(undefined, { dateStyle: "full" })} onClick={(e) => openCalendar(e, settings)}>
        <small>{now.toLocaleDateString(undefined, { weekday: "short" })}</small>
        <b>{now.getDate()}</b>
      </button>
    );
  }
  return (
    <button className="cell date" title={now.toLocaleDateString(undefined, { dateStyle: "full" })} onClick={(e) => openCalendar(e, settings)}>
      {weekday && <small>{now.toLocaleDateString(undefined, { weekday: "long" })}</small>}
      <b>{now.toLocaleDateString(undefined, { month: "short", day: "numeric" })}</b>
    </button>
  );
}

/** The time, with a small AM/PM. Ticks on the minute unless seconds are on. */
/** A small clock face; the hands' angles come from the local time. */
export function AnalogFace({ now, seconds, size = 26 }: { now: Date; seconds: boolean; size?: number }) {
  const s = now.getSeconds();
  const m = now.getMinutes() + s / 60;
  const h = (now.getHours() % 12) + m / 60;
  const hand = (deg: number, len: number, w: number, cls = "") => (
    <line x1="12" y1="12" x2="12" y2={12 - len} strokeWidth={w} strokeLinecap="round" transform={`rotate(${deg} 12 12)`} className={cls} />
  );
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" className="aface">
      <circle cx="12" cy="12" r="10.6" fill="none" strokeWidth="1.3" />
      {[0, 90, 180, 270].map((r) => (
        <line key={r} x1="12" y1="2.6" x2="12" y2="4" strokeWidth="1.3" transform={`rotate(${r} 12 12)`} />
      ))}
      {hand(h * 30, 5.2, 2)}
      {hand(m * 6, 7.6, 1.5)}
      {seconds && hand(s * 6, 8.2, 0.8, "sec")}
      <circle cx="12" cy="12" r="1.2" className="hub" />
    </svg>
  );
}

export function TimeCell({ settings, options }: WidgetProps) {
  const seconds = options.seconds === true;
  const face = String(options.face ?? "digital");
  const now = useNow(seconds);
  const fmt = new Intl.DateTimeFormat(undefined, {
    hour: "numeric",
    minute: "2-digit",
    second: seconds ? "2-digit" : undefined,
    hour12: settings.hour12 ?? undefined,
  });
  // "10:43 PM" splits into the digits and a small suffix; 24-hour clocks have no suffix.
  const parts = fmt.formatToParts(now);
  const suffix = parts.find((p) => p.type === "dayPeriod")?.value ?? "";
  const digits = parts.filter((p) => p.type !== "dayPeriod").map((p) => p.value).join("").trim();
  if (face === "analog") {
    return (
      <button className="cell time analog" title={now.toLocaleString()} onClick={(e) => openCalendar(e, settings)}>
        <AnalogFace now={now} seconds={seconds} />
      </button>
    );
  }
  if (settings.edge === "left" || settings.edge === "right") {
    // Upright dock: hours over minutes.
    const hour = parts.find((p) => p.type === "hour")?.value ?? "";
    const minute = parts.find((p) => p.type === "minute")?.value ?? "";
    return (
      <button className="cell time" title={now.toLocaleString()} onClick={(e) => openCalendar(e, settings)}>
        <b>{hour}</b>
        <b>{minute}</b>
      </button>
    );
  }
  return (
    <button className={`cell time${face === "both" ? " both" : ""}`} title={now.toLocaleString()} onClick={(e) => openCalendar(e, settings)}>
      {face === "both" && <AnalogFace now={now} seconds={seconds} size={22} />}
      <b>
        {digits}
        {suffix && <sup>{suffix}</sup>}
      </b>
    </button>
  );
}
