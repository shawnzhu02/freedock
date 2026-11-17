// Air quality and UV for the Weather flyout, from Open-Meteo's key-free air-quality API.

export type Air = { aqi: number; pm25: number; uv: number };

/** US AQI bands: [upper bound, label, colour class]. */
const BANDS: [number, string, string][] = [
  [50, "Good", "good"],
  [100, "Moderate", "fair"],
  [150, "Unhealthy for sensitive groups", "poor"],
  [200, "Unhealthy", "bad"],
  [300, "Very unhealthy", "bad"],
  [Infinity, "Hazardous", "bad"],
];

export function aqiBand(aqi: number): { label: string; tone: string } {
  const b = BANDS.find(([max]) => aqi <= max)!;
  return { label: b[1], tone: b[2] };
}

export function uvLabel(uv: number): string {
  return uv < 3 ? "Low" : uv < 6 ? "Moderate" : uv < 8 ? "High" : uv < 11 ? "Very high" : "Extreme";
}

export function parseAir(j: any): Air | null {
  const c = j?.current;
  if (!c || typeof c.us_aqi !== "number") return null;
  return { aqi: Math.round(c.us_aqi), pm25: Number(c.pm2_5 ?? 0), uv: Number(c.uv_index ?? 0) };
}

const cache = new Map<string, { at: number; p: Promise<Air | null> }>();

export function getAir(lat: number, lon: number): Promise<Air | null> {
  const k = `${lat.toFixed(2)},${lon.toFixed(2)}`;
  const hit = cache.get(k);
  if (hit && Date.now() - hit.at < 30 * 60_000) return hit.p;
  const p = fetch(`https://air-quality-api.open-meteo.com/v1/air-quality?latitude=${lat}&longitude=${lon}&current=us_aqi,pm2_5,uv_index&timezone=auto`)
    .then((r) => (r.ok ? r.json() : null))
    .then(parseAir)
    .catch(() => null);
  cache.set(k, { at: Date.now(), p });
  return p;
}
