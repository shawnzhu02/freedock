import { test } from "node:test";
import assert from "node:assert/strict";
import { keptClear, keptGet, keptSet, same } from "./kept.ts";

test("starts from the initial value, then from what was kept", () => {
  keptClear();
  assert.equal(keptGet("a", 1), 1);
  keptSet("a", 1, 5);
  assert.equal(keptGet("a", 1), 5);
});

test("an equal fresh copy keeps the old object, so nothing re-renders", () => {
  keptClear();
  const old = { wifi: true, bluetooth: false };
  const now = keptSet("r", null as typeof old | null, old);
  assert.equal(now, old);
  const again = keptSet("r", now, { wifi: true, bluetooth: false });
  assert.equal(again, old);
  assert.equal(keptGet("r", null), old);
});

test("a changed value is kept and returned", () => {
  keptClear();
  const next = keptSet("r", { wifi: true }, { wifi: false });
  assert.deepEqual(next, { wifi: false });
  assert.deepEqual(keptGet("r", null), { wifi: false });
});

test("same compares by value", () => {
  assert.ok(same([1, { a: 2 }], [1, { a: 2 }]));
  assert.ok(!same([1], [2]));
  assert.ok(same(null, null));
  assert.ok(!same(null, undefined));
});
