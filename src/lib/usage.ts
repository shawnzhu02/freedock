// Claude usage limits as the dock shows them. Pure, so `npm test` covers it.

export type UsageWindow = { utilization: number; resets_at: number; status: string };
export type Usage = { five_hour: UsageWindow | null; seven_day: UsageWindow | null; at: number };

/** "ok" under 75%, "high" to 90%, "max" above (or when Claude says it's limiting). */
export function level(w: UsageWindow | null): "ok" | "high" | "max" {
  if (!w) return "ok";
  if (w.status === "rejected" || w.utilization >= 0.9) return "max";
  if (w.utilization >= 0.75 || w.status.includes("warning")) return "high";
  return "ok";
}

/** Whole percent. */
export const pct = (w: UsageWindow | null) => (w ? Math.round(w.utilization * 100) : 0);

/** "in 42 min", "in 3 h 05", "Thu 4 pm" (a day or more away), or "now". `nowSecs` is Unix seconds. */
export function resetIn(resetsAt: number, nowSecs: number, locale = "en-US"): string {
  const s = resetsAt - nowSecs;
  if (s <= 0) return "now";
  const m = Math.round(s / 60);
  if (m < 60) return `in ${m} min`;
  if (m < 24 * 60) return `in ${Math.floor(m / 60)} h ${String(m % 60).padStart(2, "0")}`;
  const d = new Date(resetsAt * 1000);
  return d.toLocaleString(locale, { weekday: "short", hour: "numeric" }).replace(",", "");
}

/** "just now", "3 min ago", "2 h ago". */
export function ago(atSecs: number, nowSecs: number): string {
  const m = Math.floor((nowSecs - atSecs) / 60);
  if (m < 1) return "just now";
  if (m < 60) return `${m} min ago`;
  return `${Math.floor(m / 60)} h ago`;
}

/** When the window would hit 100% at the pace used so far in it (Unix seconds), or null when it
 *  won't before it resets, or it's too early in the window to tell. `lengthSecs` is the window's
 *  length (5 h or 7 days). */
export function runsOutAt(w: UsageWindow | null, lengthSecs: number, nowSecs: number): number | null {
  if (!w || w.utilization <= 0 || w.utilization >= 1) return null;
  const started = w.resets_at - lengthSecs;
  const elapsed = nowSecs - started;
  // Too early to extrapolate: the first tenth of the window (30 min of 5 h, ~17 h of a week).
  if (elapsed < lengthSecs / 10 || elapsed > lengthSecs) return null;
  const at = nowSecs + ((1 - w.utilization) * elapsed) / w.utilization;
  return at < w.resets_at ? at : null;
}

export const FIVE_HOURS = 5 * 3600;
export const SEVEN_DAYS = 7 * 86400;
