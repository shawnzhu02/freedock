import { test } from "node:test";
import assert from "node:assert/strict";
import { appName, dur, weekday } from "./screentime.ts";

test("durations", () => {
  assert.equal(dur(20), "<1m");
  assert.equal(dur(2700), "45m");
  assert.equal(dur(11580), "3h 13m");
  assert.equal(dur(3600), "1h 0m");
});

test("names and weekdays", () => {
  assert.equal(appName("WindowsTerminal"), "Terminal");
  assert.equal(appName("firefox"), "Firefox");
  assert.equal(appName("SomethingElse"), "SomethingElse");
  assert.equal(weekday("2026-11-16"), "Mon");
});
