import { test } from "node:test";
import assert from "node:assert/strict";
import { bytes, engineLabel, gpuShort, sparkPath, speed, uptime } from "./stats.ts";

test("bytes", () => {
  assert.equal(bytes(512), "512 B");
  assert.equal(bytes(1536), "1.5 KB");
  assert.equal(bytes(12.4 * 1024 ** 3), "12.4 GB");
  assert.equal(bytes(476 * 1024 ** 3), "476 GB");
  assert.equal(bytes(1.8 * 1024 ** 4), "1.8 TB");
});

test("speed", () => {
  assert.equal(speed(0), "0 KB/s");
  assert.equal(speed(840 * 1024), "840 KB/s");
  assert.equal(speed(2.4 * 1024 * 1024), "2.4 MB/s");
  assert.equal(speed(250 * 1024 * 1024), "250 MB/s");
});

test("spark path", () => {
  assert.equal(sparkPath([0, 100], 10, 20), "M0.0 20.0 L10.0 0.0");
  assert.equal(sparkPath([50], 10, 20), "");
  assert.equal(sparkPath([0, 150, -5], 20, 10), "M0.0 10.0 L10.0 0.0 L20.0 10.0");
});

test("short GPU names", () => {
  assert.equal(gpuShort("NVIDIA GeForce RTX 5070 Laptop GPU"), "RTX 5070");
  assert.equal(gpuShort("NVIDIA GeForce RTX 4070 Ti SUPER"), "RTX 4070 Ti");
  assert.equal(gpuShort("AMD Radeon RX 7900 XTX"), "RX 7900 XTX");
  assert.equal(gpuShort("Intel(R) UHD Graphics"), "Intel UHD");
  assert.equal(gpuShort("Some GPU"), "Some GPU");
});

test("engine labels", () => {
  assert.equal(engineLabel("3D"), "3D");
  assert.equal(engineLabel("VideoDecode"), "video decode");
  assert.equal(engineLabel("Compute_1"), "compute");
});

test("uptime", () => {
  assert.equal(uptime(300), "5 min");
  assert.equal(uptime(7500), "2 h 5 min");
  assert.equal(uptime(270000), "3 d 3 h");
});
