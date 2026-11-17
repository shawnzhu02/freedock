// Hover sweep: renders the dock with every widget (fake data from mock.js) on the bottom and left
// edges, in a dark and a light style. It moves the pointer over each button, takes a close-up of the
// cell under it, and puts every shot on one contact sheet per layout.
//
// Use it after changing any widget's look. On the sheet, check that:
// - the highlight covers the whole button the pointer is on;
// - there is never a box inside a box;
// - nothing spills past rounded corners;
// - text fits and vertical docks line up.
//
//   node scripts/preview/states.mjs [--out <folder>] [--only <widget-id,...>]
//
// Prints the paths of the contact sheets (PNG). Needs Microsoft Edge. Never touches the running dock.
import { mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { browser, build, serve, settingsWith, sleep } from "./lib.mjs";

const arg = (name) => {
  const i = process.argv.indexOf(name);
  return i > 0 ? process.argv[i + 1] : undefined;
};
const out = arg("--out") ?? join(tmpdir(), "freedock-states");
const only = arg("--only")?.split(",");

// Every widget, in rows small enough to fit on one bar. Spacers show each size and look.
const ROWS = [
  ["start", "apps", { id: "spacer", options: { size: "small" } }, "tray", "now-playing", "meeting", "recycle", "shortcuts", "timer", "desktop"],
  ["taskview", "search", "keyboard", "mood", { id: "spacer", options: { size: "medium", look: "dot" } }, "system", "date", "time", "world-clock", "sun", "countdown", "weather"],
  ["ask", "claude", "agenda", "agent", "sessions", "claude-usage", "timeline", "todos", "capture", "projects", "clipboard", "snip", "focus"],
  ["cpu", "ram", "gpu", "top", "network", "storage", "battery", "ping", "screen-time", "power", "volume", "mic", "brightness"],
  ["stocks", "currency", "news", "photo", "downloads", "recent", "screenshots", "notes", "calc", "transit", "github", "docker", "sites", { id: "spacer", options: { size: "large", look: "line" } }],
].map((row) => (only ? row.filter((w) => only.includes(typeof w === "string" ? w : w.id)) : row)).filter((row) => row.length);

const LOOKS = [
  { name: "bottom-glass", edge: "bottom", style: "glass" },
  { name: "bottom-white", edge: "bottom", style: "white" },
  { name: "left-glass", edge: "left", style: "glass" },
];

mkdirSync(out, { recursive: true });
const dir = build();
for (const [r, row] of ROWS.entries()) for (const look of LOOKS) writeFileSync(join(dir, `settings-r${r}-${look.name}.json`), JSON.stringify(settingsWith(row, { edge: look.edge, style: look.style })));
const site = await serve(dir);
const page = await browser();
const sheets = [];
try {
  for (const [r] of ROWS.entries()) {
    for (const look of LOOKS) {
      const vertical = look.edge === "left";
      // The dock window's real size: the bar's thickness (height setting) plus its margins.
      const thick = 50 + 2 * 5 + 4;
      await page.size(vertical ? thick : 1800, vertical ? 1400 : thick);
      const set = `r${r}-${look.name}`;
      await page.open(`${site.url}/?set=${set}&wall=split`, 2000);
      await page.hover(1, 1);
      // Every visible button in the dock, with the cell it belongs to (the close-up frames the cell).
      const targets = await page.eval(`[...document.querySelectorAll(".dock button, .dock a[href], .dock [role=button]")]
        .filter((e) => { const r = e.getBoundingClientRect(); return r.width > 2 && r.height > 2 && getComputedStyle(e).visibility !== "hidden"; })
        .map((e) => {
          const r = e.getBoundingClientRect();
          const c = (e.closest(".cell") || e).getBoundingClientRect();
          return { x: r.x + r.width / 2, y: r.y + r.height / 2,
            clip: { x: Math.max(0, c.x - 8), y: Math.max(0, c.y - 8), width: c.width + 16, height: c.height + 16 },
            label: e.getAttribute("aria-label") || e.title.split("\\n")[0] || e.className || e.tagName };
        })`);
      const whole = await page.eval(`(() => { const r = document.querySelector(".dock")?.getBoundingClientRect(); return r && { x: Math.max(0, r.x - 10), y: Math.max(0, r.y - 10), width: r.width + 20, height: r.height + 20 }; })()`);
      const shots = [];
      if (whole) {
        await page.shot(join(out, `${set}-dock.png`), whole);
        shots.push({ file: `${set}-dock.png`, label: "whole dock, no hover", wide: true });
      }
      for (const [i, t] of targets.entries()) {
        await page.hover(t.x, t.y);
        await sleep(260);
        const file = `${set}-${String(i).padStart(2, "0")}.png`;
        await page.shot(join(out, file), t.clip);
        shots.push({ file, label: t.label });
      }
      await page.hover(1, 1);
      const sheet = join(out, `${set}.html`);
      const esc = (s) => String(s).replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`);
      writeFileSync(
        sheet,
        `<!doctype html><meta charset="utf-8"><body style="margin:0;padding:12px;background:#59627a;font:12px system-ui;color:#fff">
        <h3 style="margin:0 0 8px">${esc(set)}: hover on each button</h3>
        <div style="display:flex;flex-wrap:wrap;gap:10px;align-items:flex-start">
        ${shots.map((s) => `<figure style="margin:0;${s.wide ? "flex-basis:100%" : ""}"><img src="${s.file}" style="display:block;max-width:${s.wide ? "100%" : "360px"};zoom:.5"><figcaption style="max-width:180px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis">${esc(s.label)}</figcaption></figure>`).join("")}
        </div></body>`,
      );
      await page.size(1800, 1200);
      await page.open(`file:///${sheet.replace(/\\/g, "/")}`, 400);
      // Resize the page to the sheet's height and take a plain shot (capturing beyond the viewport
      // sometimes repeats the page).
      const h = await page.eval("document.documentElement.scrollHeight");
      await page.size(1800, Math.min(h, 8000));
      await sleep(300);
      const png = join(out, `${set}-sheet.png`);
      await page.shot(png);
      sheets.push(png);
    }
  }
} finally {
  page.close();
  site.close();
}
console.log(sheets.join("\n"));
