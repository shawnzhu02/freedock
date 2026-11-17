// Countdown to a date the user types: "2026-11-19" or "2026-11-19 14:00". Pure, so `npm test` covers it.

/** The local moment for "YYYY-MM-DD" (midnight) or "YYYY-MM-DD HH:MM", or null when it doesn't parse. */
export function parseWhen(s: string): Date | null {
  const m = /^\s*(\d{4})-(\d{1,2})-(\d{1,2})(?:[ T](\d{1,2}):(\d{2}))?\s*$/.exec(s);
  if (!m) return null;
  const [y, mo, d, h, mi] = [+m[1], +m[2], +m[3], m[4] ? +m[4] : 0, m[5] ? +m[5] : 0];
  const t = new Date(y, mo - 1, d, h, mi);
  // Reject roll-overs like 2026-02-31.
  if (t.getMonth() !== mo - 1 || t.getDate() !== d || h > 23 || mi > 59) return null;
  return t;
}

/** Calendar days from today to the target's day (0 = today, negative = past). */
export function daysUntil(target: Date, now: Date): number {
  const a = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const b = new Date(target.getFullYear(), target.getMonth(), target.getDate());
  return Math.round((b.getTime() - a.getTime()) / 86_400_000);
}

/** What the cell shows: "3d", "Tomorrow", "Today", "4h 20m" when a time is set and it's under a day,
 *  or "Done" once it has passed. */
export function countdownText(target: Date, now: Date, hasTime: boolean): string {
  const ms = target.getTime() - now.getTime();
  const days = daysUntil(target, now);
  if (hasTime ? ms <= 0 : days < 0) return "Done";
  if (hasTime && ms < 86_400_000) {
    const m = Math.ceil(ms / 60_000);
    return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h ${m % 60}m`;
  }
  if (days === 0) return "Today";
  if (days === 1) return "Tomorrow";
  return `${days}d`;
}
