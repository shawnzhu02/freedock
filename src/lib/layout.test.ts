import { test } from "node:test";
import assert from "node:assert/strict";
import { moveItem, resolve, serialize } from "./layout.ts";

const all = ["cpu", "gpu", "memory", "apps"];

test("defaults when nothing is saved", () => {
  const r = resolve(undefined, all, ["apps"]);
  assert.deepEqual(r.order, all);
  assert.deepEqual([...r.hidden], ["apps"]);
});

test("saved order and hidden sections; unknown and repeated ids dropped", () => {
  const r = resolve(["memory", "-cpu", "old", "memory", "gpu"], all);
  assert.deepEqual(r.order, ["memory", "cpu", "gpu", "apps"]);
  assert.deepEqual([...r.hidden], ["cpu"]);
});

test("a new section appears at the end, hidden only if it's hidden by default", () => {
  assert.deepEqual(resolve(["gpu", "cpu"], all, ["apps"]).order, ["gpu", "cpu", "memory", "apps"]);
  assert.ok(resolve(["gpu", "cpu"], all, ["apps"]).hidden.has("apps"));
  assert.ok(!resolve(["gpu", "cpu"], all, ["apps"]).hidden.has("memory"));
});

test("round trip", () => {
  const r = resolve(["-apps", "cpu"], all);
  assert.deepEqual(resolve(serialize(r.order, r.hidden), all), r);
});

test("moveItem", () => {
  assert.deepEqual(moveItem(["a", "b", "c"], 0, 2), ["b", "c", "a"]);
  assert.deepEqual(moveItem(["a", "b", "c"], 2, 0), ["c", "a", "b"]);
});
