// Flicker check: opens every flyout the way Rust does (a "popup-mode" event) against a slow fake
// backend (mock.js `?lag=1`: each command answers after 20-400 ms), and measures how much the
// flyout still moves after it has been revealed (Rust uncloaks it on `popup_ready`):
//
// - "fits": height changes sent to Rust after the reveal. Each one resizes the real window.
// - "shift": Chromium's layout-shift score after the reveal (content jumping around).
//
// Each flyout is opened twice: cold (nothing known yet) and again after closing (state kept from the
// first open). A reopen should be perfectly still: 0 fits and 0 shift. A cold open may settle once.
//
//   node scripts/preview/flyouts.mjs [--only quick,media] [--film <folder>]
//
// `--film` also saves a strip of shots from the first 400 ms of each reopen (the open animation). Needs Microsoft Edge.
// Never touches the running dock.
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { browser, build, serve, settingsWith, sleep } from "./lib.mjs";

const arg = (name) => {
  const i = process.argv.indexOf(name);
  return i > 0 ? process.argv[i + 1] : undefined;
};
const only = arg("--only")?.split(",");
const film = arg("--film");
if (film) mkdirSync(film, { recursive: true });

// Popup modes with the window size Rust gives them (popup.rs size_for) and any prefill they need.
const MODES = [
  ["tray", 300, 200], ["quick", 360, 300], ["calendar", 340, 460], ["media", 300, 420], ["usage", 320, 240],
  ["stats", 320, 700], ["weather", 360, 520], ["projects", 420, 460], ["clipboard", 400, 460], ["focus", 320, 360],
  ["capture", 360, 200], ["downloads", 380, 460], ["screenshots", 420, 460], ["recent", 380, 460], ["notes", 380, 420],
  ["github", 400, 480], ["docker", 380, 420], ["power", 300, 260], ["todos", 360, 460], ["sites", 340, 360],
  ["calc", 320, 420], ["moods", 260, 300], ["start", 560, 640], ["appmenu", 280, 320, "code"], ["sessions", 340, 400],
  ["screentime", 360, 480], ["transit", 340, 360], ["stocks", 360, 420], ["currency", 300, 260], ["news", 400, 480],
  ["thumbs", 440, 178, "code"],
].filter(([m]) => !only || only.includes(m));

const dir = build();
writeFileSync(join(dir, "settings-flyouts.json"), JSON.stringify(settingsWith(["start", "apps", "time"], {})));
const site = await serve(dir);
const page = await browser();
// Layout shifts, recorded from the very first frame of every page.
await page.send("Page.addScriptToEvaluateOnNewDocument", {
  source: `window.__errs = []; addEventListener("error", (e) => window.__errs.push(String(e.message))); addEventListener("unhandledrejection", (e) => window.__errs.push(String(e.reason))); window.__shifts = []; new PerformanceObserver((l) => l.getEntries().forEach((e) => window.__shifts.push({ t: e.startTime, v: e.value }))).observe({ type: "layout-shift", buffered: true });`,
});

const measure = (since) =>
  page.eval(`(() => {
    const calls = window.__mock.calls.filter((c) => c.t >= ${since});
    const ready = calls.find((c) => c.cmd === "popup_ready")?.t ?? null;
    const after = (t) => ready !== null && t > ready;
    const fits = calls.filter((c) => c.cmd === "popup_fit" && after(c.t)).length;
    const shift = window.__shifts.filter((s) => s.t >= ${since} && after(s.t)).reduce((a, s) => a + s.v, 0);
    return { ready: ready === null ? null : Math.round(ready - ${since}), fits, shift: Math.round(shift * 1000) / 1000, errs: window.__errs.slice(0, 2) };
  })()`);

const rows = [];
try {
  for (const [mode, w, h, prefill = ""] of MODES) {
    await page.size(w, h);
    await page.open(`${site.url}/?set=flyouts&lag=1&for=${mode}#ask`, 500);
    const open = async () => {
      const t = await page.eval(`(() => { const t = performance.now(); window.__mock.emit("popup-mode", { mode: ${JSON.stringify(mode)}, prefill: ${JSON.stringify(prefill)} }); return t; })()`);
      return t;
    };
    const t1 = await open();
    await sleep(900);
    const cold = await measure(t1);
    await page.eval(`window.__mock.emit("popup-hidden", null)`);
    await sleep(150);
    const t2 = await open();
    if (film) {
      for (const at of [0, 50, 90, 130, 180, 400]) {
        const now = await page.eval(`performance.now() - ${t2}`);
        if (at > now) await sleep(at - now);
        await page.shot(join(film, `${mode}-${String(at).padStart(3, "0")}.png`));
      }
    }
    await sleep(900);
    const warm = await measure(t2);
    rows.push({ mode, cold, warm });
    if (cold.errs.length || warm.errs.length) console.log(`${mode}: page errors: ${[...cold.errs, ...warm.errs].join(" | ")}`);
    const bad = cold.ready === null || warm.ready === null ? "  <-- NEVER READY (the page is broken: nothing would show)" : warm.fits || warm.shift > 0.001 ? "  <-- moves on reopen" : cold.fits > 1 || cold.shift > 0.05 ? "  <-- settles late" : "";
    console.log(`${mode.padEnd(12)} cold: ready ${String(cold.ready).padStart(4)} ms, fits ${cold.fits}, shift ${cold.shift}   reopen: ready ${String(warm.ready).padStart(4)} ms, fits ${warm.fits}, shift ${warm.shift}${bad}`);
  }
} finally {
  page.close();
  site.close();
}
const dead = rows.filter((r) => r.cold.ready === null || r.warm.ready === null || r.cold.errs.length || r.warm.errs.length);
if (dead.length) {
  console.log(`
${dead.length} flyout(s) never got ready or threw: ${dead.map((r) => r.mode).join(", ")}`);
  process.exitCode = 1;
}
const bad = rows.filter((r) => r.warm.fits || r.warm.shift > 0.001);
console.log(bad.length ? `\n${bad.length} flyout(s) move on reopen: ${bad.map((r) => r.mode).join(", ")}` : "\nEvery flyout is still on reopen.");
