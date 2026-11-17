import { test } from "node:test";
import assert from "node:assert/strict";
import { rankName, SETTINGS_PAGES } from "./start.ts";

test("rankName matches startmenu.rs", () => {
  assert.equal(rankName("Visual Studio Code", "visual studio code"), 0);
  assert.equal(rankName("Visual Studio Code", "vis"), 1);
  assert.equal(rankName("Visual Studio Code", "code"), 2);
  assert.equal(rankName("Visual Studio Code", "vsc"), 3);
  assert.equal(rankName("Notepad", "pad"), 4);
  assert.equal(rankName("Notepad", "x"), null);
  assert.equal(rankName("Notepad", " "), null);
});

test("settings pages are all ms-settings links", () => {
  assert.ok(SETTINGS_PAGES.every((p) => p.uri.startsWith("ms-settings:")));
  assert.ok(SETTINGS_PAGES.some((p) => rankName(`${p.name} ${p.words}`, "wallpaper") !== null));
});
