import { test } from "node:test";
import assert from "node:assert/strict";
import { moonAt } from "./moon.ts";

const at = (iso: string) => moonAt(Date.parse(iso));

test("full moon of 5 Nov 2025 is full", () => {
  const m = at("2025-11-05T13:19:00Z");
  assert.ok(m.lit > 0.98, String(m.lit));
  assert.equal(m.name, "Full moon");
});

test("new moon of 20 Nov 2025 is new", () => {
  const m = at("2025-11-20T06:47:00Z");
  assert.ok(m.lit < 0.02, String(m.lit));
  assert.equal(m.name, "New moon");
});

test("waxing between new and full, waning after", () => {
  assert.equal(at("2025-11-24T00:00:00Z").waxing, true);
  assert.equal(at("2025-11-09T00:00:00Z").waxing, false);
  assert.equal(at("2025-11-28T06:00:00Z").name, "First quarter");
});
