import { test } from "node:test";
import assert from "node:assert/strict";
import { badge, checkState, reasonLabel, type GitHub } from "./github.ts";

test("check states", () => {
  assert.equal(checkState("SUCCESS"), "ok");
  assert.equal(checkState("FAILURE"), "fail");
  assert.equal(checkState("ERROR"), "fail");
  assert.equal(checkState("PENDING"), "wait");
  assert.equal(checkState(""), "");
});

test("badge counts unread and review requests, nothing on error", () => {
  const g: GitHub = { login: "me", unread: 2, notes: [], prs: [], reviews: [{ title: "", repo: "", url: "", draft: false, review: "", checks: "", updated: "" }], error: null };
  assert.equal(badge(g), 3);
  assert.equal(badge({ ...g, error: "Run gh auth login" }), 0);
  assert.equal(badge(null), 0);
  assert.equal(reasonLabel("review_requested"), "review requested");
});
