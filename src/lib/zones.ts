// World clock helpers. Pure, so `npm test` covers them.

export const ZONES: [string, string][] = [
  ["America/Los_Angeles", "San Francisco"],
  ["America/Denver", "Denver"],
  ["America/Chicago", "Chicago"],
  ["America/New_York", "New York"],
  ["America/Sao_Paulo", "São Paulo"],
  ["Europe/London", "London"],
  ["Europe/Paris", "Paris"],
  ["Europe/Berlin", "Berlin"],
  ["Europe/Istanbul", "Istanbul"],
  ["Asia/Dubai", "Dubai"],
  ["Asia/Kolkata", "India"],
  ["Asia/Singapore", "Singapore"],
  ["Asia/Shanghai", "Shanghai"],
  ["Asia/Hong_Kong", "Hong Kong"],
  ["Asia/Seoul", "Seoul"],
  ["Asia/Tokyo", "Tokyo"],
  ["Australia/Sydney", "Sydney"],
  ["Pacific/Auckland", "Auckland"],
  ["UTC", "UTC"],
];

/** Minutes east of UTC for `zone` at `when`. */
export function offsetMin(zone: string, when: Date): number {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: zone,
    hourCycle: "h23",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).formatToParts(when);
  const n = (t: string) => Number(parts.find((p) => p.type === t)?.value);
  const asUtc = Date.UTC(n("year"), n("month") - 1, n("day"), n("hour"), n("minute"));
  return Math.round((asUtc - Math.floor(when.getTime() / 60000) * 60000) / 60000);
}

/** "+5h", "-3h", "+5:30", "same time" relative to here. */
export function diffLabel(zone: string, when: Date, localOffsetMin = -when.getTimezoneOffset()): string {
  const d = offsetMin(zone, when) - localOffsetMin;
  if (d === 0) return "same time";
  const sign = d > 0 ? "+" : "-";
  const a = Math.abs(d);
  return a % 60 ? `${sign}${Math.floor(a / 60)}:${String(a % 60).padStart(2, "0")}` : `${sign}${a / 60}h`;
}

/** "Tomorrow" / "Yesterday" / "" for the zone's calendar day against ours. */
export function dayLabel(zone: string, when: Date): string {
  const f = (tz?: string) => new Intl.DateTimeFormat("en-CA", { timeZone: tz, year: "numeric", month: "2-digit", day: "2-digit" }).format(when);
  const there = f(zone);
  const here = f(undefined);
  return there > here ? "Tomorrow" : there < here ? "Yesterday" : "";
}

export function validZone(zone: string): boolean {
  try {
    new Intl.DateTimeFormat("en-US", { timeZone: zone });
    return true;
  } catch {
    return false;
  }
}
