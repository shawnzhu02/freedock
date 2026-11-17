import { test } from "node:test";
import assert from "node:assert/strict";
import { isLight, luma, profile, shadeFor, span } from "./contrast.ts";

const line = (colors: [number, number, number, number][]) => new Uint8Array(colors.flat());

test("luma is 0 for black and 1 for white", () => {
  assert.equal(luma(0, 0, 0), 0);
  assert.ok(Math.abs(luma(255, 255, 255) - 1) < 1e-9);
});

test("profile buckets a line from left to right", () => {
  const px = line([
    [255, 255, 255, 255],
    [255, 255, 255, 255],
    [0, 0, 0, 255],
    [0, 0, 0, 255],
  ]);
  const p = profile(px, 1, 2);
  assert.ok(p[0] > 0.99 && p[1] < 0.01);
});

test("profile skips transparent pixels and undoes premultiplied alpha", () => {
  // A half-transparent white pixel is stored as 128,128,128,128: it is still white.
  const px = line([
    [128, 128, 128, 128],
    [0, 0, 0, 0],
  ]);
  const p = profile(px, 1, 2);
  assert.ok(p[0] > 0.99);
  assert.equal(p[1], -1);
});

test("profile averages several lines", () => {
  const px = line([
    [255, 255, 255, 255],
    [0, 0, 0, 255],
  ]);
  const p = profile(px, 2, 1);
  assert.ok(Math.abs(p[0] - 0.5) < 0.01);
});

test("span averages the buckets a cell covers and ignores unknown ones", () => {
  const b = [0, 1, 1, -1];
  assert.equal(span(b, 0.25, 0.75), 1);
  assert.ok(Math.abs(span(b, 0, 0.75) - 2 / 3) < 1e-9);
  assert.equal(span(b, 0.8, 1), -1);
  assert.equal(span([], 0, 1), -1);
});

test("isLight has a gap so a cell doesn't flicker between inks", () => {
  assert.equal(isLight(false, 0.5), false);
  assert.equal(isLight(false, 0.55), true);
  assert.equal(isLight(true, 0.47), true);
  assert.equal(isLight(true, 0.4), false);
  assert.equal(isLight(true, -1), true);
});

test("shadeFor darkens only over a bright backdrop", () => {
  assert.equal(shadeFor([0.1, 0.2, 0.3]), 0);
  assert.equal(shadeFor([]), 0);
  assert.equal(shadeFor([-1, -1]), 0);
  const s = shadeFor([1, 1, 0.2, 0.2]);
  assert.ok(s > 0.6 && s <= 0.75);
  // With that shade a bright backdrop comes out near 0.3.
  assert.ok(Math.abs(0.9 * (1 - shadeFor([0.9])) - 0.3) < 0.01);
});
