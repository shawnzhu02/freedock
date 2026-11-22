// Weather from Open-Meteo (free, no key; only the city's coordinates are sent). The mapping and
// formatting are pure, so `npm test` covers them.

export type Kind = "clear" | "partly" | "cloudy" | "fog" | "drizzle" | "rain" | "snow" | "storm";

/** WMO weather code -> our icon kind and a short label. */
export function describe(code: number): { kind: Kind; label: string } {
  if (code === 0) return { kind: "clear", label: "Clear" };
  if (code === 1) return { kind: "partly", label: "Mostly clear" };
  if (code === 2) return { kind: "partly", label: "Partly cloudy" };
  if (code === 3) return { kind: "cloudy", label: "Cloudy" };
  if (code === 45 || code === 48) return { kind: "fog", label: "Fog" };
  if (code >= 51 && code <= 57) return { kind: "drizzle", label: code >= 56 ? "Freezing drizzle" : "Drizzle" };
  if ((code >= 61 && code <= 67) || (code >= 80 && code <= 82)) {
    const heavy = code === 65 || code === 82;
    const freezing = code === 66 || code === 67;
    return { kind: "rain", label: freezing ? "Freezing rain" : heavy ? "Heavy rain" : code >= 80 ? "Showers" : "Rain" };
  }
  if ((code >= 71 && code <= 77) || code === 85 || code === 86) return { kind: "snow", label: code === 75 || code === 86 ? "Heavy snow" : "Snow" };
  if (code >= 95) return { kind: "storm", label: "Thunderstorm" };
  return { kind: "cloudy", label: "—" };
}

export type Weather = {
  temp: number;
  feels: number;
  code: number;
  day: boolean;
  wind: number;
  humidity: number;
  hours: { time: string; temp: number; code: number; day: boolean; rain: number }[];
  days: { date: string; code: number; max: number; min: number; rain: number; sunrise: string; sunset: string }[];
  unit: "f" | "c";
  at: number;
  /** The city's UTC offset in seconds. Every time above is the city's local time (timezone=auto). */
  offset: number;
};

/** Open-Meteo's JSON -> our shape; hours start at the current hour and run 24 ahead. */
export function parse(j: any, unit: "f" | "c", nowIso: string): Weather {
  const c = j.current;
  const h = j.hourly;
  const start = Math.max(0, (h.time as string[]).findIndex((t) => t.slice(0, 13) >= nowIso.slice(0, 13)));
  const hours = (h.time as string[]).slice(start, start + 24).map((time, i) => ({
    time,
    temp: h.temperature_2m[start + i],
    code: h.weather_code[start + i],
    day: h.is_day[start + i] === 1,
    rain: h.precipitation_probability?.[start + i] ?? 0,
  }));
  const d = j.daily;
  const days = (d.time as string[]).map((date, i) => ({
    date,
    code: d.weather_code[i],
    max: d.temperature_2m_max[i],
    min: d.temperature_2m_min[i],
    rain: d.precipitation_probability_max?.[i] ?? 0,
    sunrise: d.sunrise[i],
    sunset: d.sunset[i],
  }));
  return {
    temp: c.temperature_2m,
    feels: c.apparent_temperature,
    code: c.weather_code,
    day: c.is_day === 1,
    wind: c.wind_speed_10m,
    humidity: c.relative_humidity_2m,
    hours,
    days,
    unit,
    offset: j.utc_offset_seconds ?? -new Date().getTimezoneOffset() * 60,
    at: Date.now(),
  };
}

/** "6:42 AM" from Open-Meteo's local "2026-11-16T06:42". */
/** The 12/24-hour choice: the setting when it's set, else what this PC's language uses (Auto). */
export const use12h = (setting: boolean | null | undefined): boolean =>
  setting ?? new Intl.DateTimeFormat(undefined, { hour: "numeric" }).resolvedOptions().hour12 ?? true;

export function hm(iso: string, hour12 = true): string {
  const [hh, mm] = iso.slice(11, 16).split(":").map(Number);
  if (!hour12) return `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;
  return `${hh % 12 || 12}:${String(mm).padStart(2, "0")} ${hh < 12 ? "AM" : "PM"}`;
}

/** The next sunrise or sunset after `nowIso` (local, same format), from the daily list. */
export function nextSun(days: Weather["days"], nowIso: string): { what: "sunrise" | "sunset"; at: string } | null {
  for (const d of days) {
    if (d.sunrise > nowIso) return { what: "sunrise", at: d.sunrise };
    if (d.sunset > nowIso) return { what: "sunset", at: d.sunset };
  }
  return null;
}

/** The time now in a city `offset` seconds from UTC, in Open-Meteo's format. Compare forecast times with
 *  this, not with the PC's clock: with a city in another time zone the two differ by hours. */
export function cityIso(offset: number, now = Date.now()): string {
  return new Date(now + offset * 1000).toISOString().slice(0, 16);
}

/** "2026-11-16T14:05" in local time, the format Open-Meteo uses with timezone=auto. */
export function localIso(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`;
}

// ---- fetch with a shared 15-minute cache (not used by tests)

const cache = new Map<string, Promise<Weather>>();
const stamp = new Map<string, number>();

export function getWeather(lat: number, lon: number, unit: "f" | "c", maxAgeMs = 15 * 60_000): Promise<Weather> {
  const key = `${lat.toFixed(3)},${lon.toFixed(3)},${unit}`;
  const hit = cache.get(key);
  if (hit && Date.now() - (stamp.get(key) ?? 0) < maxAgeMs) return hit;
  const url =
    `https://api.open-meteo.com/v1/forecast?latitude=${lat}&longitude=${lon}` +
    "&current=temperature_2m,apparent_temperature,weather_code,is_day,wind_speed_10m,relative_humidity_2m" +
    "&hourly=temperature_2m,weather_code,precipitation_probability,is_day" +
    "&daily=weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,precipitation_probability_max" +
    `&timezone=auto&forecast_days=7${unit === "f" ? "&temperature_unit=fahrenheit&wind_speed_unit=mph" : ""}`;
  const p = fetch(url)
    .then((r) => {
      if (!r.ok) throw new Error(`Open-Meteo answered ${r.status}`);
      return r.json();
    })
    .then((j) => parse(j, unit, typeof j.utc_offset_seconds === "number" ? cityIso(j.utc_offset_seconds) : localIso()));
  cache.set(key, p);
  stamp.set(key, Date.now());
  p.catch(() => {
    cache.delete(key);
    stamp.delete(key);
  });
  return p;
}

const WET: Kind[] = ["drizzle", "rain", "snow", "storm"];

/** The first of the next `within` hours (not counting the current one) when rain or snow is likely,
 *  unless it's already wet now; null when the next few hours look dry. */
export function rainSoon(w: Weather, within = 3, threshold = 60): { time: string; chance: number; kind: Kind } | null {
  if (WET.includes(describe(w.code).kind)) return null;
  for (const h of w.hours.slice(1, within + 1)) {
    const kind = describe(h.code).kind;
    if (h.rain >= threshold || (WET.includes(kind) && h.rain >= 40)) return { time: h.time, chance: h.rain, kind: WET.includes(kind) ? kind : "rain" };
  }
  return null;
}
