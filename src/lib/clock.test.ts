import { test } from "node:test";
import assert from "node:assert/strict";
import { clock } from "./clock.ts";

test("stopwatch clock", () => {
  assert.equal(clock(0), "0:00");
  assert.equal(clock(65.9), "1:05");
  assert.equal(clock(3725), "1:02:05");
});
