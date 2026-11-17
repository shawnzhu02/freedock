import { test } from "node:test";
import assert from "node:assert/strict";
import { filter, since } from "./projects.ts";

test("since", () => {
  assert.equal(since(100, 130), "now");
  assert.equal(since(0, 12 * 60), "12m");
  assert.equal(since(0, 3 * 3600 + 5), "3h");
  assert.equal(since(0, 2 * 86400), "2d");
  assert.equal(since(0, 35 * 86400), "5w");
  assert.equal(since(0, 250 * 86400), "8mo");
});

test("filter", () => {
  const p = (name: string, branch = "main") => ({ name, branch, path: name, touched: 0, claude: null, web: null });
  const list = [p("freedock"), p("chess-bot", "season-1"), p("home/server", "hq-brain")];
  assert.equal(filter(list, "").length, 3);
  assert.deepEqual(filter(list, "chess").map((x) => x.name), ["chess-bot"]);
  assert.deepEqual(filter(list, "season").map((x) => x.name), ["chess-bot"]);
  assert.deepEqual(filter(list, "serv brain").map((x) => x.name), ["home/server"]);
  assert.equal(filter(list, "nothing").length, 0);
});
