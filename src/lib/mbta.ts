// Next trains/buses at an MBTA stop, from the key-free MBTA v3 API (rate limited per address, so
// it's polled at most every 30 s and shared between the cell and the flyout).

export type Arrival = { route: string; direction: number; to: string; mins: number; color: string };
export type Line = { route: string; direction: number; to: string; color: string; mins: number[] };

export const STOPS: [string, string][] = [
  ["place-dwnxg", "Downtown Crossing"],
  ["place-north", "North Station"],
  ["place-coecl", "Copley"],
  ["place-kencl", "Kenmore (Green)"],
  ["place-pktrm", "Park Street"],
  ["place-harsq", "Harvard (Red)"],
  ["place-sstat", "South Station"],
];

/** Predictions JSON (with include=route) at `nowMs` -> arrivals, soonest first; departed ones dropped. */
export function parseArrivals(j: any, nowMs: number): Arrival[] {
  const routes = new Map<string, any>();
  for (const r of j?.included ?? []) if (r.type === "route") routes.set(r.id, r.attributes ?? {});
  const out: Arrival[] = [];
  for (const p of j?.data ?? []) {
    const a = p.attributes ?? {};
    const t = a.arrival_time ?? a.departure_time;
    if (!t) continue;
    const mins = Math.floor((Date.parse(t) - nowMs) / 60_000);
    if (mins < 0) continue;
    const route = p.relationships?.route?.data?.id ?? "";
    const ra = routes.get(route) ?? {};
    const dir = Number(a.direction_id ?? 0);
    out.push({
      route: ra.short_name || ra.long_name?.replace(/ Line$/, "") || route,
      direction: dir,
      to: a.trip_headsign || ra.direction_destinations?.[dir] || (dir ? "Inbound" : "Outbound"),
      mins,
      color: ra.color ? `#${ra.color}` : "#888",
    });
  }
  return out.sort((x, y) => x.mins - y.mins);
}

/** One row per route and destination, with its next few times. */
export function lines(arrivals: Arrival[], perLine = 3): Line[] {
  const out: Line[] = [];
  for (const a of arrivals) {
    let l = out.find((x) => x.route === a.route && x.to === a.to);
    if (!l) out.push((l = { route: a.route, direction: a.direction, to: a.to, color: a.color, mins: [] }));
    if (l.mins.length < perLine) l.mins.push(a.mins);
  }
  return out;
}

/** "Now", "1 min", "12 min". */
export const minsLabel = (m: number) => (m <= 0 ? "Now" : `${m} min`);

const cache = new Map<string, { at: number; p: Promise<Arrival[]> }>();

export function getArrivals(stop: string): Promise<Arrival[]> {
  const hit = cache.get(stop);
  if (hit && Date.now() - hit.at < 30_000) return hit.p;
  const url = `https://api-v3.mbta.com/predictions?filter%5Bstop%5D=${encodeURIComponent(stop)}&sort=arrival_time&page%5Blimit%5D=20&include=route`;
  const p = fetch(url)
    .then((r) => {
      if (!r.ok) throw new Error(r.status === 429 ? "MBTA is rate limiting; try again in a minute" : `MBTA said ${r.status}`);
      return r.json();
    })
    .then((j) => parseArrivals(j, Date.now()));
  cache.set(stop, { at: Date.now(), p });
  p.catch(() => cache.delete(stop));
  return p;
}
