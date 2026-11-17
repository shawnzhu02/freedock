import { test } from "node:test";
import assert from "node:assert/strict";
import { quality, stats } from "./ping.ts";

test("ping stats: average, jitter, loss", () => {
  const s = stats([10, 20, null, 30]);
  assert.equal(s.last, 30);
  assert.equal(s.avg, 20);
  assert.equal(s.jitter, 10);
  assert.equal(s.loss, 0.25);
  assert.deepEqual(stats([]), { last: null, avg: null, jitter: null, loss: 0, n: 0 });
});

test("ping quality", () => {
  assert.equal(quality(stats([12, 14, 13])), "good");
  assert.equal(quality(stats([80, 90, 85])), "ok");
  assert.equal(quality(stats([20, 20, null, 20, 20, 20, 20, 20, 20, 20])), "poor"); // 10% loss
  assert.equal(quality(stats([200, 210])), "poor");
  assert.equal(quality(stats([null, null, null])), "poor");
  assert.equal(quality(stats([])), "none");
});
