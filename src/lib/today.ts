// The agenda's today view (src-tauri/src/agenda.rs) and the small calculations the dock does on it.
// Pure functions only, so they run under `node --test`.

export type TodayEvent = { time: string; title: string; place?: string };
export type TodayItem = { id: string; title: string; meta?: string; area?: string; due?: string; day?: string; course?: string };
export type TodayView = { events: TodayEvent[]; today: TodayItem[]; soon: TodayItem[]; school: TodayItem[]; errors?: string[] };

/** "10 am", "10:30 pm", "10:30am", "14:00", "noon" -> minutes after midnight, or null. */
export function parseClock(text: string): number | null {
  const t = text.trim().toLowerCase();
  if (t === "noon") return 12 * 60;
  if (t === "midnight") return 0;
  const m = /^(\d{1,2})(?::(\d{2}))?\s*(am|pm|a\.m\.|p\.m\.)?$/.exec(t);
  if (!m) return null;
  let h = Number(m[1]);
  const min = m[2] ? Number(m[2]) : 0;
  const ap = m[3]?.[0];
  if (min > 59 || h > 23) return null;
  if (ap) {
    if (h < 1 || h > 12) return null;
    if (ap === "p" && h !== 12) h += 12;
    if (ap === "a" && h === 12) h = 0;
  } else if (!m[2]) {
    return null; // a bare "10" is ambiguous
  }
  return h * 60 + min;
}

export type NextMeeting = {
  event: TodayEvent;
  start: Date;
  /** "now" while it runs (assumed `lengthMin` long), "soon" within 15 minutes, else "later". */
  state: "now" | "soon" | "later";
  /** Minutes until it starts (negative once started). */
  minutes: number;
};

/** Today's next event that hasn't finished yet. Events without a readable time are skipped. */
export function nextMeeting(events: TodayEvent[], now: Date, lengthMin = 60): NextMeeting | null {
  const midnight = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const timed = events
    .map((event) => ({ event, at: parseClock(event.time) }))
    .filter((e): e is { event: TodayEvent; at: number } => e.at !== null)
    .sort((a, b) => a.at - b.at);
  for (const { event, at } of timed) {
    const start = new Date(midnight.getTime() + at * 60_000);
    const minutes = Math.round((start.getTime() - now.getTime()) / 60_000);
    if (minutes + lengthMin <= 0) continue;
    return { event, start, minutes, state: minutes <= 0 ? "now" : minutes <= 15 ? "soon" : "later" };
  }
  return null;
}

/** "now", "in 5 min", "in 1 h 05", or the start time for anything more than 3 hours away. */
export function countdown(m: NextMeeting): string {
  if (m.state === "now") return "now";
  if (m.minutes < 60) return `in ${m.minutes} min`;
  if (m.minutes < 180) return `in ${Math.floor(m.minutes / 60)} h ${String(m.minutes % 60).padStart(2, "0")}`;
  return m.event.time;
}

/** Video-call services a Join button may open. Calendar invites can come from anyone, so other links
 *  never become a Join button; hosts are compared exactly (or as a subdomain on a dot boundary). */
const MEETING_HOSTS = ["meet.google.com", "zoom.us", "teams.microsoft.com", "teams.live.com", "webex.com"];

export function isMeetingLink(url: string): boolean {
  try {
    const u = new URL(url);
    if (u.protocol !== "https:" || u.username || u.password) return false;
    const host = u.hostname.toLowerCase();
    return MEETING_HOSTS.some((h) => host === h || host.endsWith(`.${h}`));
  } catch {
    return false;
  }
}

/** Where "Join" goes: a meeting link written in the place, or the service's own page for Meet, Zoom
 *  and Teams events without one. Null when there's nothing to join. */
export function joinLink(place: string | undefined): string | null {
  if (!place) return null;
  const url = (place.match(/https:\/\/[^\s)>"]+/gi) ?? []).find(isMeetingLink);
  if (url) return url;
  if (/google meet|meet\.google/i.test(place)) return "https://calendar.google.com/calendar/r/day";
  if (/zoom/i.test(place)) return "https://zoom.us/myhome";
  if (/teams/i.test(place)) return "https://teams.microsoft.com/";
  return null;
}

/** "2026-11-16" for a local date. */
export const ymd = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;

/** Items grouped by due date. */
export function byDue(items: TodayItem[]): Map<string, TodayItem[]> {
  const out = new Map<string, TodayItem[]>();
  for (const i of items) {
    if (!i.due) continue;
    out.set(i.due, [...(out.get(i.due) ?? []), i]);
  }
  return out;
}

/** "Mon", "Tomorrow", "Today" or "Overdue" for a due date, relative to `now`. */
export function dueLabel(due: string, now: Date): string {
  const [y, m, d] = due.split("-").map(Number);
  const days = Math.round((new Date(y, m - 1, d).getTime() - new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()) / 86_400_000);
  if (days < 0) return "Overdue";
  if (days === 0) return "Today";
  if (days === 1) return "Tomorrow";
  if (days < 7) return new Date(y, m - 1, d).toLocaleDateString("en-US", { weekday: "short" });
  return new Date(y, m - 1, d).toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

/** The meeting's key when it starts within a minute either side of `now` (time to pause music), else null. */
export function startingNow(m: NextMeeting | null, now: Date): string | null {
  if (!m) return null;
  const d = m.start.getTime() - now.getTime();
  return d <= 60_000 && d >= -90_000 ? `${m.start.toISOString()} ${m.event.title}` : null;
}

export type Block = { left: number; width: number; title: string; time: string; now: boolean };

/** Today's timed events laid out on a strip from `now - before` to `now + after` hours, as
 *  percentages; events are assumed `lengthMin` long and clipped to the strip. */
export function timeline(events: TodayEvent[], now: Date, before = 1, after = 5, lengthMin = 60): { blocks: Block[]; ticks: { left: number; hour: number }[]; nowAt: number } {
  const span = (before + after) * 60;
  const startMin = now.getHours() * 60 + now.getMinutes() - before * 60;
  const pctOf = (min: number) => ((min - startMin) / span) * 100;
  const blocks: Block[] = [];
  for (const e of events) {
    const at = parseClock(e.time);
    if (at === null) continue;
    const a = Math.max(at, startMin);
    const b = Math.min(at + lengthMin, startMin + span);
    if (b <= a) continue;
    const nowMin = now.getHours() * 60 + now.getMinutes();
    blocks.push({ left: pctOf(a), width: pctOf(b) - pctOf(a), title: e.title, time: e.time, now: at <= nowMin && nowMin < at + lengthMin });
  }
  const ticks: { left: number; hour: number }[] = [];
  for (let h = Math.ceil(startMin / 60); h * 60 <= startMin + span; h++) ticks.push({ left: pctOf(h * 60), hour: ((h % 24) + 24) % 24 });
  return { blocks, ticks, nowAt: pctOf(startMin + before * 60) };
}
