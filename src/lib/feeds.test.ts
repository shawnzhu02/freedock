import { test } from "node:test";
import assert from "node:assert/strict";
import { convert, pct, price, rangeSpark, rotation, shortSymbol } from "./feeds.ts";

test("sparkline uses the values' own range", () => {
  assert.equal(rangeSpark([5], 40, 20), "");
  const p = rangeSpark([100, 110, 105], 40, 20);
  assert.equal(p, "M0.0 19.0 L20.0 1.0 L40.0 10.0");
  // A flat line doesn't divide by zero.
  assert.ok(!rangeSpark([3, 3, 3], 40, 20).includes("NaN"));
});

test("formats changes and prices", () => {
  assert.equal(pct(1.234), "+1.23%");
  assert.equal(pct(-0.5), "-0.50%");
  assert.equal(pct(0), "0.00%");
  assert.match(price(64895.23), /^64.?895$/);
  assert.equal(price(352.421), "352.42");
  assert.equal(price(1.08421), "1.0842");
  assert.equal(shortSymbol("^GSPC"), "GSPC");
  assert.equal(shortSymbol("BTC-USD"), "BTC");
  assert.equal(shortSymbol("EURUSD=X"), "EURUSD");
});

test("converts with rates based on the source currency", () => {
  const r = { base: "USD", date: "2026-11-16", rates: { EUR: 0.9, GBP: 0.75 } };
  assert.equal(convert(10, "USD", "EUR", r), 9);
  assert.equal(convert(10, "USD", "USD", r), 10);
  assert.equal(convert(10, "USD", "XYZ", r), null);
  assert.equal(convert(10, "EUR", "USD", r), null, "rates for another base don't apply");
  assert.equal(convert(Number.NaN, "USD", "EUR", r), null);
  assert.equal(convert(1, "USD", "EUR", null), null);
});

test("rotates through items", () => {
  assert.equal(rotation(0, 123456, 8000), 0);
  assert.equal(rotation(3, 0, 8000), 0);
  assert.equal(rotation(3, 8000, 8000), 1);
  assert.equal(rotation(3, 24000, 8000), 0);
});
