import { test } from "node:test";
import assert from "node:assert/strict";
import { lines, minsLabel, parseArrivals } from "./mbta.ts";
import { aqiBand, parseAir, uvLabel } from "./air.ts";

const now = Date.parse("2026-11-16T09:50:00-05:00");
const j = {
  data: [
    { attributes: { arrival_time: "2026-11-16T09:53:10-05:00", direction_id: 1, trip_headsign: null }, relationships: { route: { data: { id: "Green-B" } } } },
    { attributes: { arrival_time: "2026-11-16T09:49:00-05:00", direction_id: 0 }, relationships: { route: { data: { id: "Green-B" } } } },
    { attributes: { arrival_time: null, departure_time: "2026-11-16T09:51:30-05:00", direction_id: 0 }, relationships: { route: { data: { id: "Green-B" } } } },
    { attributes: { arrival_time: "2026-11-16T10:01:00-05:00", direction_id: 1 }, relationships: { route: { data: { id: "Green-B" } } } },
    { attributes: { arrival_time: "2026-11-16T09:55:00-05:00", direction_id: 0, trip_headsign: "Cleveland Circle" }, relationships: { route: { data: { id: "57" } } } },
  ],
  included: [
    { type: "route", id: "Green-B", attributes: { short_name: "B", long_name: "Green Line B", color: "00843D", direction_destinations: ["Boston College", "Government Center"] } },
    { type: "route", id: "57", attributes: { short_name: "57", color: "FFC72C", direction_destinations: ["Watertown Yard", "Kenmore"] } },
  ],
};

test("MBTA arrivals: departed dropped, soonest first, headsign or destination", () => {
  const a = parseArrivals(j, now);
  assert.deepEqual(a.map((x) => [x.route, x.to, x.mins]), [
    ["B", "Boston College", 1],
    ["B", "Government Center", 3],
    ["57", "Cleveland Circle", 5],
    ["B", "Government Center", 11],
  ]);
  assert.equal(a[0].color, "#00843D");
});

test("MBTA lines group by route and destination", () => {
  const l = lines(parseArrivals(j, now));
  assert.deepEqual(l.map((x) => [x.route, x.to, x.mins]), [
    ["B", "Boston College", [1]],
    ["B", "Government Center", [3, 11]],
    ["57", "Cleveland Circle", [5]],
  ]);
  assert.equal(minsLabel(0), "Now");
  assert.equal(minsLabel(4), "4 min");
});

test("air quality", () => {
  assert.deepEqual(parseAir({ current: { us_aqi: 33.4, pm2_5: 8.9, uv_index: 0.8 } }), { aqi: 33, pm25: 8.9, uv: 0.8 });
  assert.equal(parseAir({}), null);
  assert.equal(aqiBand(33).label, "Good");
  assert.equal(aqiBand(120).tone, "poor");
  assert.equal(uvLabel(6.5), "High");
});
