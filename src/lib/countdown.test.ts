import { test } from "node:test";
import assert from "node:assert/strict";
import { countdownText, daysUntil, parseWhen } from "./countdown.ts";

test("parses dates and date-times, rejects nonsense", () => {
  assert.deepEqual(parseWhen("2026-11-19"), new Date(2026, 10, 19));
  assert.deepEqual(parseWhen(" 2026-11-19 14:30 "), new Date(2026, 10, 19, 14, 30));
  assert.equal(parseWhen("2026-02-31"), null);
  assert.equal(parseWhen("11/19/2026"), null);
  assert.equal(parseWhen("2026-11-19 25:00"), null);
  assert.equal(parseWhen(""), null);
});

test("countdown wording", () => {
  const now = new Date(2026, 10, 16, 4, 50);
  assert.equal(daysUntil(new Date(2026, 10, 19), now), 3);
  assert.equal(countdownText(new Date(2026, 10, 19), now, false), "3d");
  assert.equal(countdownText(new Date(2026, 10, 17), now, false), "Tomorrow");
  assert.equal(countdownText(new Date(2026, 10, 16), now, false), "Today");
  assert.equal(countdownText(new Date(2026, 10, 15), now, false), "Done");
  assert.equal(countdownText(new Date(2026, 10, 16, 9, 10), now, true), "4h 20m");
  assert.equal(countdownText(new Date(2026, 10, 16, 5, 0), now, true), "10m");
  assert.equal(countdownText(new Date(2026, 10, 16, 4, 0), now, true), "Done");
});
