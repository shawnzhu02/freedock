import { test } from "node:test";
import assert from "node:assert/strict";
import { diffLabel, offsetMin, validZone } from "./zones.ts";

const winter = new Date(Date.UTC(2026, 0, 15, 12, 0));
const summer = new Date(Date.UTC(2026, 6, 15, 12, 0));

test("offsets", () => {
  assert.equal(offsetMin("UTC", winter), 0);
  assert.equal(offsetMin("America/New_York", winter), -300);
  assert.equal(offsetMin("America/New_York", summer), -240);
  assert.equal(offsetMin("Asia/Kolkata", winter), 330);
  assert.equal(offsetMin("Asia/Tokyo", summer), 540);
});

test("differences from here", () => {
  // Pretend "here" is New York in winter (UTC-5).
  assert.equal(diffLabel("Europe/London", winter, -300), "+5h");
  assert.equal(diffLabel("America/Los_Angeles", winter, -300), "-3h");
  assert.equal(diffLabel("Asia/Kolkata", winter, -300), "+10:30");
  assert.equal(diffLabel("America/New_York", winter, -300), "same time");
});

test("zone check", () => {
  assert.ok(validZone("Asia/Tokyo"));
  assert.ok(!validZone("Mars/Olympus"));
});
