import { test } from "node:test";
import assert from "node:assert/strict";
import { FIVE_HOURS, ago, level, pct, resetIn, runsOutAt } from "./usage.ts";

const w = (utilization: number, status = "allowed") => ({ utilization, resets_at: 0, status });

test("levels", () => {
  assert.equal(level(w(0.58)), "ok");
  assert.equal(level(w(0.8)), "high");
  assert.equal(level(w(0.3, "allowed_warning")), "high");
  assert.equal(level(w(0.95)), "max");
  assert.equal(level(w(0.2, "rejected")), "max");
  assert.equal(level(null), "ok");
  assert.equal(pct(w(0.444)), 44);
});

test("reset wording", () => {
  const now = 1_000_000;
  assert.equal(resetIn(now - 5, now), "now");
  assert.equal(resetIn(now + 42 * 60, now), "in 42 min");
  assert.equal(resetIn(now + 185 * 60, now), "in 3 h 05");
  assert.match(resetIn(now + 3 * 86400, now), /^[A-Z][a-z]{2} \d{1,2} (AM|PM)$/);
});

test("ago wording", () => {
  assert.equal(ago(100, 130), "just now");
  assert.equal(ago(0, 180), "3 min ago");
  assert.equal(ago(0, 7300), "2 h ago");
});

test("runs out: extrapolates the pace so far, only when it lands before the reset", () => {
  const now = 1_000_000;
  // 2 h into a 5 h window at 60%: 1.33 h more reaches 100%, before the reset 3 h away.
  const w = { utilization: 0.6, resets_at: now + 3 * 3600, status: "allowed" };
  const at = runsOutAt(w, FIVE_HOURS, now)!;
  assert.ok(Math.abs(at - (now + 4800)) < 1);
  // 2 h in at 30%: the pace reaches 75% by the reset, so no warning.
  assert.equal(runsOutAt({ ...w, utilization: 0.3 }, FIVE_HOURS, now), null);
  // 10 min in: too early to tell.
  assert.equal(runsOutAt({ ...w, resets_at: now + FIVE_HOURS - 600 }, FIVE_HOURS, now), null);
  assert.equal(runsOutAt(null, FIVE_HOURS, now), null);
  assert.equal(runsOutAt({ ...w, utilization: 1 }, FIVE_HOURS, now), null);
});
