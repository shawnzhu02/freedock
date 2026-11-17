import { test } from "node:test";
import assert from "node:assert/strict";
import { describe as wx, hm, localIso, nextSun, parse, rainSoon, type Weather } from "./weather.ts";

test("WMO codes", () => {
  assert.deepEqual(wx(0), { kind: "clear", label: "Clear" });
  assert.equal(wx(2).kind, "partly");
  assert.equal(wx(48).kind, "fog");
  assert.equal(wx(53).kind, "drizzle");
  assert.equal(wx(65).label, "Heavy rain");
  assert.equal(wx(81).label, "Showers");
  assert.equal(wx(67).label, "Freezing rain");
  assert.equal(wx(75).kind, "snow");
  assert.equal(wx(96).kind, "storm");
});

test("clock wording", () => {
  assert.equal(hm("2026-11-16T06:42"), "6:42 AM");
  assert.equal(hm("2026-11-16T16:05"), "4:05 PM");
  assert.equal(hm("2026-11-16T00:30"), "12:30 AM");
  assert.equal(hm("2026-11-16T16:05", false), "16:05");
  assert.equal(localIso(new Date(2026, 10, 6, 9, 3)), "2026-11-06T09:03");
});

const days = [
  { date: "2026-11-16", code: 0, max: 50, min: 40, rain: 0, sunrise: "2026-11-16T06:38", sunset: "2026-11-16T16:22" },
  { date: "2026-11-17", code: 0, max: 50, min: 40, rain: 0, sunrise: "2026-11-17T06:39", sunset: "2026-11-17T16:21" },
];

test("next sun event", () => {
  assert.deepEqual(nextSun(days, "2026-11-16T02:00"), { what: "sunrise", at: "2026-11-16T06:38" });
  assert.deepEqual(nextSun(days, "2026-11-16T12:00"), { what: "sunset", at: "2026-11-16T16:22" });
  assert.deepEqual(nextSun(days, "2026-11-16T20:00"), { what: "sunrise", at: "2026-11-17T06:39" });
  assert.equal(nextSun(days, "2026-11-18T20:00"), null);
});

test("parse starts hours at now", () => {
  const time = Array.from({ length: 48 }, (_, i) => `2026-11-${i < 24 ? "16" : "17"}T${String(i % 24).padStart(2, "0")}:00`);
  const j = {
    current: { temperature_2m: 41, apparent_temperature: 36, weather_code: 3, is_day: 0, wind_speed_10m: 9, relative_humidity_2m: 70 },
    hourly: { time, temperature_2m: time.map((_, i) => i), weather_code: time.map(() => 1), is_day: time.map(() => 1), precipitation_probability: time.map(() => 5) },
    daily: { time: ["2026-11-16"], weather_code: [3], temperature_2m_max: [50], temperature_2m_min: [38], sunrise: ["2026-11-16T06:38"], sunset: ["2026-11-16T16:22"], precipitation_probability_max: [20] },
  };
  const w = parse(j, "f", "2026-11-16T14:20");
  assert.equal(w.hours[0].time, "2026-11-16T14:00");
  assert.equal(w.hours.length, 24);
  assert.equal(w.temp, 41);
  assert.equal(w.day, false);
  assert.equal(w.days[0].rain, 20);
});

test("rain soon: first wet hour in the next three, not when it's already raining", () => {
  const hour = (time: string, code: number, rain: number) => ({ time, temp: 50, code, day: true, rain });
  const base: Weather = {
    temp: 50, feels: 48, code: 1, day: true, wind: 5, humidity: 60, unit: "f", at: 0, days: [],
    hours: [hour("2026-11-16T14:00", 1, 10), hour("2026-11-16T15:00", 2, 20), hour("2026-11-16T16:00", 61, 70), hour("2026-11-16T17:00", 63, 90)],
  };
  assert.deepEqual(rainSoon(base), { time: "2026-11-16T16:00", chance: 70, kind: "rain" });
  assert.equal(rainSoon({ ...base, code: 61 }), null, "already raining");
  assert.equal(rainSoon(base, 1), null, "only an hour ahead");
  const dry = { ...base, hours: base.hours.map((h) => ({ ...h, code: 1, rain: 10 })) };
  assert.equal(rainSoon(dry), null);
});
