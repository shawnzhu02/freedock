import { test } from "node:test";
import assert from "node:assert/strict";
import { evaluate, show } from "./calc.ts";

test("arithmetic, precedence, powers", () => {
  assert.equal(evaluate("1 + 2 * 3"), 7);
  assert.equal(evaluate("(1 + 2) * 3"), 9);
  assert.equal(evaluate("2 ^ 3 ^ 2"), 512);
  assert.equal(evaluate("-2 ^ 2"), -4);
  assert.equal(evaluate("10 / 4"), 2.5);
  assert.equal(evaluate("1,200 + 5"), 1205);
  assert.equal(evaluate("6 × 7 ÷ 2"), 21);
});

test("functions, constants, implicit multiply, ans", () => {
  assert.equal(evaluate("sqrt(16)"), 4);
  assert.equal(evaluate("sqrt 16 + 1"), 5);
  assert.ok(Math.abs(evaluate("2pi") - 2 * Math.PI) < 1e-12);
  assert.equal(evaluate("3(4+1)"), 15);
  assert.equal(evaluate("ans * 2", 21), 42);
  assert.equal(evaluate("log(1000)"), 3);
});

test("percentages", () => {
  assert.equal(evaluate("15% of 80"), 12);
  assert.equal(evaluate("50%"), 0.5);
  assert.equal(evaluate("200 * 15%"), 30);
});

test("errors", () => {
  for (const bad of ["", "1 +", "(1", "2 ** 3x", "foo(2)", "1..2", "1/0", "alert(1)", "1; 2"]) {
    assert.throws(() => evaluate(bad), bad);
  }
});

test("display", () => {
  assert.equal(show(0.1 + 0.2), "0.3");
  assert.equal(show(1234567.5), "1,234,567.5");
  assert.equal(show(2 / 3), "0.6666666667");
  assert.equal(show(1e20), "1e+20");
});
