import { test } from "node:test";
import assert from "node:assert/strict";
import { byDue, countdown, dueLabel, joinLink, nextMeeting, parseClock, startingNow, timeline } from "./today.ts";

test("parseClock reads agenda times", () => {
  assert.equal(parseClock("10 am"), 600);
  assert.equal(parseClock("10:30 pm"), 22 * 60 + 30);
  assert.equal(parseClock("12 pm"), 720);
  assert.equal(parseClock("12 am"), 0);
  assert.equal(parseClock("9:05am"), 545);
  assert.equal(parseClock("14:00"), 840);
  assert.equal(parseClock("noon"), 720);
  assert.equal(parseClock("10"), null);
  assert.equal(parseClock("all day"), null);
  assert.equal(parseClock("13 pm"), null);
});

test("nextMeeting picks the next unfinished event", () => {
  const ev = [
    { time: "2 pm", title: "Later" },
    { time: "10 am", title: "Review", place: "Google Meet" },
    { time: "all day", title: "Holiday" },
  ];
  const at = (h: number, m = 0) => new Date(2026, 10, 16, h, m);
  assert.equal(nextMeeting(ev, at(9, 50))?.event.title, "Review");
  assert.equal(nextMeeting(ev, at(9, 50))?.state, "soon");
  assert.equal(nextMeeting(ev, at(9, 0))?.state, "later");
  assert.equal(nextMeeting(ev, at(10, 30))?.state, "now");
  assert.equal(nextMeeting(ev, at(11, 0))?.event.title, "Later");
  assert.equal(nextMeeting(ev, at(15, 30)), null);
});

test("countdown wording", () => {
  const m = (minutes: number) => ({ event: { time: "10 am", title: "x" }, start: new Date(), minutes, state: minutes <= 0 ? "now" : minutes <= 15 ? "soon" : "later" }) as const;
  assert.equal(countdown(m(0)), "now");
  assert.equal(countdown(m(7)), "in 7 min");
  assert.equal(countdown(m(65)), "in 1 h 05");
  assert.equal(countdown(m(400)), "10 am");
});

test("joinLink finds links and known services", () => {
  assert.equal(joinLink("Zoom https://acme.zoom.us/j/123?pwd=x"), "https://acme.zoom.us/j/123?pwd=x");
  assert.equal(joinLink("Google Meet (instructions in description)"), "https://calendar.google.com/calendar/r/day");
  assert.equal(joinLink("CAS 116"), null);
  assert.equal(joinLink("Room 4 https://phish.example/login"), null, "other links never become Join");
  assert.equal(joinLink("https://zoom.us.evil.example/j/1"), "https://zoom.us/myhome", "suffix trick: only the fixed Zoom page");
  assert.equal(joinLink("https://zoom.us@evil.example/j/1"), "https://zoom.us/myhome", "userinfo trick: only the fixed Zoom page");
  assert.equal(joinLink("https://evil.example/x https://meet.google.com/abc"), "https://meet.google.com/abc");
  assert.equal(joinLink(undefined), null);
});

test("due dates group and label", () => {
  const now = new Date(2026, 10, 16, 1);
  const g = byDue([{ id: "a", title: "A", due: "2026-11-18" }, { id: "b", title: "B", due: "2026-11-18" }, { id: "c", title: "C" }]);
  assert.equal(g.get("2026-11-18")?.length, 2);
  assert.equal(g.size, 1);
  assert.equal(dueLabel("2026-11-16", now), "Today");
  assert.equal(dueLabel("2026-11-17", now), "Tomorrow");
  assert.equal(dueLabel("2026-11-15", now), "Overdue");
  assert.equal(dueLabel("2026-11-19", now), "Thu");
  assert.equal(dueLabel("2026-11-30", now), "Nov 30");
});

test("startingNow fires from a minute before to 90 s after the start, once per meeting key", () => {
  const ev = [{ time: "3:00 pm", title: "Standup" }];
  const at = (h: number, m: number, s = 0) => new Date(2026, 10, 16, h, m, s);
  assert.equal(startingNow(nextMeeting(ev, at(14, 58)), at(14, 58)), null);
  const k1 = startingNow(nextMeeting(ev, at(14, 59, 30)), at(14, 59, 30));
  const k2 = startingNow(nextMeeting(ev, at(15, 0, 45)), at(15, 0, 45));
  assert.ok(k1 && k1.endsWith("Standup"));
  assert.equal(k1, k2);
  assert.equal(startingNow(nextMeeting(ev, at(15, 3)), at(15, 3)), null);
  assert.equal(startingNow(null, at(15, 0)), null);
});

test("timeline places events on a now-1h..now+5h strip", () => {
  const now = new Date(2026, 10, 16, 14, 0);
  const t = timeline([{ time: "1:30 pm", title: "Standup" }, { time: "4 pm", title: "Review" }, { time: "9 pm", title: "Late" }, { time: "TBD", title: "x" }], now);
  assert.equal(t.nowAt, (60 / 360) * 100);
  assert.equal(t.blocks.length, 2, "9 pm is past the strip, TBD has no time");
  assert.equal(t.blocks[0].title, "Standup");
  assert.ok(Math.abs(t.blocks[0].left - (30 / 360) * 100) < 1e-9);
  assert.equal(t.blocks[0].now, true);
  assert.ok(Math.abs(t.blocks[1].left - (180 / 360) * 100) < 1e-9);
  assert.equal(t.ticks[0].hour, 13);
  assert.equal(t.ticks.length, 7);
});
