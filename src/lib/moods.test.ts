import { test } from "node:test";
import assert from "node:assert/strict";
import { newMood, ownLook, removeMood, slotKeys } from "./moods.ts";
import type { Settings } from "../settings.ts";

const slot = (id: string) => ({ id, enabled: true, options: null });

test("shared widgets keep their key across moods, wherever they sit", () => {
  const daily = slotKeys(["apps", "weather", "time"].map(slot));
  const coding = slotKeys(["apps", "cpu", "time"].map(slot));
  assert.equal(daily[0], coding[0]);
  assert.equal(daily[2], coding[2], "time moved nowhere but keeps its key even with other cells around it");
  const twice = slotKeys(["spacer", "apps", "spacer"].map(slot));
  assert.deepEqual(twice, ["spacer#0", "apps#0", "spacer#1"], "copies of one widget get their own keys");
});

const base = {
  style: "glass",
  glass_live: false,
  edge: "bottom",
  visibility: "smart",
  widgets: [slot("apps")],
  active_mood: "daily",
  shared_look: { style: "glass", glass_live: false, edge: "bottom", visibility: "smart" },
  moods: [
    { id: "daily", name: "Daily", icon: "", widgets: [], look: null },
    { id: "focus", name: "Focus", icon: "", widgets: [], look: null },
  ],
} as unknown as Settings;

test("new moods copy the widgets and get a fresh id", () => {
  const m = newMood({ ...base, moods: [...base.moods, { ...base.moods[0], id: "mood-3" }] });
  assert.equal(m.id, "mood-4");
  assert.deepEqual(m.widgets, base.widgets);
  assert.notEqual(m.widgets[0], base.widgets[0], "a copy, not the same objects");
});

test("own look on keeps the current look; off brings the shared one back", () => {
  const on = ownLook({ ...base, style: "black" } as Settings, true);
  assert.equal(on.moods![0].look!.style, "black");
  const off = ownLook({ ...base, style: "black", moods: on.moods! } as Settings, false);
  assert.equal(off.moods![0].look, null);
  assert.equal(off.style, "glass");
});

test("the active mood and the last mood can't be removed", () => {
  assert.equal(removeMood(base, "daily"), null);
  assert.deepEqual(removeMood(base, "focus")!.moods!.map((m) => m.id), ["daily"]);
  assert.equal(removeMood({ ...base, moods: [base.moods[0]] } as Settings, "focus"), null);
});
