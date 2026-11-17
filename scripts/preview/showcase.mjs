// Marketing screenshots for the README, rendered with the fake backend (mock.js) in headless Edge:
//
// - docs/screenshot.png: a desktop scene (our own gradient wallpaper) with the dock at the bottom and
//   the Start and Quick settings flyouts open above their cells.
// - docs/flyouts.png: four flyouts side by side (calendar, weather, media, stocks) over the same dock.
//
//   node scripts/preview/showcase.mjs [--out <folder>] [--scale 1.25]
//
// Everything shown is demo data: the mock's samples plus the overrides below (a fixed 9:41 AM clock,
// Lisbon weather served locally instead of Open-Meteo, quick settings, stats, app icons). Nothing is
// fetched from the internet. The dock and each flyout are iframes in one page, the way flyouts.mjs
// renders them, and the glass reads the same wallpaper the page shows. Never touches the running dock.
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { ROOT, browser, build, serve, settingsWith, sleep } from "./lib.mjs";

const arg = (name) => {
  const i = process.argv.indexOf(name);
  return i > 0 ? process.argv[i + 1] : undefined;
};
const out = arg("--out") ?? join(ROOT, "docs");
const scale = Number(arg("--scale") ?? 1.25);
const W = 1600, H = 900;

// ---- the wallpaper: soft colour glows on deep indigo (SVG gradients only, so the glass can read it)

const WALL = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
<defs>
  <linearGradient id="base" x1="0" y1="0" x2="1" y2="1">
    <stop offset="0" stop-color="#141a46"/><stop offset=".55" stop-color="#2a1a5e"/><stop offset="1" stop-color="#40164f"/>
  </linearGradient>
  <radialGradient id="g1" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(330 800) scale(760 520)">
    <stop offset="0" stop-color="#ff5fa2" stop-opacity=".85"/><stop offset=".5" stop-color="#e0459a" stop-opacity=".35"/><stop offset="1" stop-color="#e0459a" stop-opacity="0"/>
  </radialGradient>
  <radialGradient id="g2" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(1180 880) scale(820 470)">
    <stop offset="0" stop-color="#ffb35c" stop-opacity=".9"/><stop offset=".45" stop-color="#ff7a59" stop-opacity=".45"/><stop offset="1" stop-color="#ff7a59" stop-opacity="0"/>
  </radialGradient>
  <radialGradient id="g3" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(1380 130) scale(700 560)">
    <stop offset="0" stop-color="#3fd0ff" stop-opacity=".7"/><stop offset=".5" stop-color="#3a7bff" stop-opacity=".3"/><stop offset="1" stop-color="#3a7bff" stop-opacity="0"/>
  </radialGradient>
  <radialGradient id="g4" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(240 90) scale(620 480)">
    <stop offset="0" stop-color="#7b5cff" stop-opacity=".7"/><stop offset="1" stop-color="#7b5cff" stop-opacity="0"/>
  </radialGradient>
  <radialGradient id="g5" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(820 420) scale(520 300)">
    <stop offset="0" stop-color="#b65cff" stop-opacity=".35"/><stop offset="1" stop-color="#b65cff" stop-opacity="0"/>
  </radialGradient>
</defs>
<rect width="${W}" height="${H}" fill="url(#base)"/>
<rect width="${W}" height="${H}" fill="url(#g4)"/><rect width="${W}" height="${H}" fill="url(#g3)"/>
<rect width="${W}" height="${H}" fill="url(#g5)"/><rect width="${W}" height="${H}" fill="url(#g1)"/>
<rect width="${W}" height="${H}" fill="url(#g2)"/>
</svg>`;

// ---- demo app icons (generic glyphs on rounded squares)

const tile = (a, b, glyph) =>
  "data:image/svg+xml;base64," +
  Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64"><defs><linearGradient id="t" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${a}"/><stop offset="1" stop-color="${b}"/></linearGradient></defs><rect x="3" y="3" width="58" height="58" rx="15" fill="url(#t)"/><g fill="none" stroke="#fff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">${glyph}</g></svg>`,
  ).toString("base64");
const ICONS = {
  browser: tile("#4f9dff", "#1f5fd8", '<circle cx="32" cy="32" r="16"/><ellipse cx="32" cy="32" rx="7" ry="16"/><path d="M16 32h32"/>'),
  files: tile("#ffd25e", "#f29a1f", '<path d="M15 22a3 3 0 0 1 3-3h9l4 4h15a3 3 0 0 1 3 3v16a3 3 0 0 1-3 3H18a3 3 0 0 1-3-3z" fill="#fff" fill-opacity=".25"/>'),
  code: tile("#9a7bff", "#5a3bd8", '<path d="M25 22l-10 10 10 10M39 22l10 10-10 10M35 18l-6 28"/>'),
  chat: tile("#3ee0a1", "#0f9e68", '<path d="M18 20h28a3 3 0 0 1 3 3v15a3 3 0 0 1-3 3H30l-9 7v-7h-3a3 3 0 0 1-3-3V23a3 3 0 0 1 3-3z"/>'),
  music: tile("#ff7aa8", "#e2306c", '<path d="M27 42V20l17-4v22"/><circle cx="23" cy="42" r="4.5" fill="#fff"/><circle cx="40" cy="38" r="4.5" fill="#fff"/>'),
  terminal: tile("#4a4f5c", "#1d2027", '<path d="M18 23l9 9-9 9M32 42h14"/>'),
  photos: tile("#ffb067", "#ff5f3d", '<path d="M14 44l11-12 8 8 6-6 11 10"/><circle cx="40" cy="22" r="4" fill="#fff"/>'),
  mail: tile("#6fd3ff", "#2a86d8", '<rect x="15" y="20" width="34" height="24" rx="3"/><path d="M16 22l16 12 16-12"/>'),
  calc: tile("#a3abbd", "#5b6375", '<rect x="19" y="15" width="26" height="34" rx="3"/><path d="M24 23h16M25 32h1M32 32h1M39 32h1M25 40h1M32 40h1M39 40h1"/>'),
  notes: tile("#ffe680", "#f5c022", '<path d="M22 22h20M22 31h20M22 40h12"/>'),
};
const APPS = [
  ["browser", "Browser"], ["files", "File Explorer"], ["code", "Code editor"], ["chat", "Chat"], ["music", "Music"],
  ["terminal", "Terminal"], ["photos", "Photos"], ["mail", "Mail"], ["calc", "Calculator"], ["notes", "Notes"],
];

// ---- overrides served before the mock's own samples (run inside each frame, after mock.js)

const DEMO = {
  icons: ICONS,
  apps: APPS,
};

// Runs before mock.js: a fixed clock (today, 9:41 AM), so the dock, agenda and weather agree.
const PRE = `(() => {
  const R = Date, t = new R();
  t.setHours(9, 41, 0, 0);
  const off = t.getTime() - R.now();
  class D extends R {
    constructor(...a) { if (a.length) super(...a); else super(R.now() + off); }
    static now() { return R.now() + off; }
  }
  window.Date = D;
})();`;

// Runs after mock.js: wraps its invoke with the demo data, sizes the iframe the way Rust sizes the
// window (dock_fit, popup_fit), and places the glass by the iframe's spot in the scene.
const POST = `(() => {
  const DEMO = ${JSON.stringify(DEMO)};
  const fe = window.frameElement;
  const iso = (d) => { const p = (n) => String(n).padStart(2, "0"); return d.getFullYear() + "-" + p(d.getMonth() + 1) + "-" + p(d.getDate()); };
  const today = new Date();
  // Open-Meteo's answer for Lisbon in late autumn: mild, partly cloudy, showers on Friday.
  const weather = () => {
    const hourly = { time: [], temperature_2m: [], weather_code: [], precipitation_probability: [], is_day: [] };
    for (let d = 0; d < 7; d++) for (let h = 0; h < 24; h++) {
      const day = new Date(today.getFullYear(), today.getMonth(), today.getDate() + d);
      hourly.time.push(iso(day) + "T" + String(h).padStart(2, "0") + ":00");
      hourly.temperature_2m.push(Math.round((15 + 4 * Math.sin(((h - 8) / 24) * Math.PI * 2) - d * 0.3) * 10) / 10);
      hourly.weather_code.push(h >= 13 && h <= 15 ? 0 : h >= 10 && h <= 16 ? 1 : h > 18 ? 3 : 2);
      hourly.precipitation_probability.push(h > 19 ? 20 : 5);
      hourly.is_day.push(h >= 8 && h < 18 ? 1 : 0);
    }
    const codes = [2, 1, 0, 61, 80, 2, 1], max = [19, 20, 21, 17, 16, 18, 19], min = [12, 12, 13, 12, 11, 11, 12], rain = [5, 0, 0, 70, 60, 10, 5];
    const daily = { time: [], weather_code: codes, temperature_2m_max: max, temperature_2m_min: min, precipitation_probability_max: rain, sunrise: [], sunset: [] };
    for (let d = 0; d < 7; d++) {
      const s = iso(new Date(today.getFullYear(), today.getMonth(), today.getDate() + d));
      daily.time.push(s);
      daily.sunrise.push(s + "T07:" + String(31 + Math.floor(d / 2)).padStart(2, "0"));
      daily.sunset.push(s + "T17:" + String(22 - Math.floor(d / 3)).padStart(2, "0"));
    }
    return { current: { temperature_2m: 16.4, apparent_temperature: 15.8, weather_code: 2, is_day: 1, wind_speed_10m: 11, relative_humidity_2m: 68 }, hourly, daily };
  };
  const realFetch = window.fetch.bind(window);
  window.fetch = (u, o) => {
    const url = String(u && u.url ? u.url : u);
    if (url.includes("api.open-meteo.com")) return Promise.resolve(new Response(JSON.stringify(weather()), { headers: { "content-type": "application/json" } }));
    if (/^https?:/.test(url) && !url.startsWith(location.origin)) return Promise.resolve(new Response("", { status: 503 }));
    return realFetch(u, o);
  };
  let tick = 0;
  const cpu = () => Math.max(4, Math.round(22 + 11 * Math.sin(tick / 3.2) + 6 * Math.sin(tick / 1.3) + (tick % 7 === 3 ? 14 : 0)));
  const win = (hwnd, title, extra) => Object.assign({ hwnd, title, active: false, minimized: false, flashing: false }, extra || {});
  const app = (key, windows, pinned) => ({ key, name: DEMO.apps.find((a) => a[0] === key)[1], pinned, lnk: null, exe: null, icon: key, windows });
  const over = {
    geometry: () => {
      const r = fe.getBoundingClientRect();
      return { mon_w: parent.innerWidth, mon_h: parent.innerHeight, left: r.x, top: r.y, width: r.width, height: r.height };
    },
    wallpaper: () => ({ data: parent.__wall, fit: "fill" }),
    dock_fit: (a) => {
      fe.style.width = a.width + "px";
      fe.style.left = Math.round((parent.innerWidth - a.width) / 2) + "px";
      setTimeout(() => window.__mock.emit("geometry-changed", null), 30);
    },
    popup_fit: (a) => { fe.style.height = a.height + "px"; },
    apps_list: () => [
      app("browser", [win(1, "Trip planning - Browser", { active: true })], true),
      app("files", [], true),
      app("code", [win(2, "main.rs"), win(3, "README.md")], true),
      app("chat", [win(4, "Team chat")], true),
      app("music", [win(5, "Music")], false),
      app("terminal", [win(6, "Terminal")], false),
    ],
    app_icon: (a) => DEMO.icons[a.path] || null,
    agenda_today: {
      events: [
        { time: "10:00", title: "Design review", place: "https://meet.example.com/design-review" },
        { time: "12:30", title: "Lunch with Sam", place: "Cafe on the corner" },
        { time: "16:00", title: "Gym", place: "" },
      ],
      today: [{ id: "t1", title: "Send the slides" }],
      soon: [],
      school: [],
    },
    start_apps: () => DEMO.apps.map(([id, name]) => ({ id, name })).sort((a, b) => a.name.localeCompare(b.name)),
    start_state: () => ({ pins: ["browser", "files", "code", "chat", "music", "terminal", "photos", "mail", "calc", "notes"], recent: ["code", "browser", "photos"], user: "Alex" }),
    files_list: () => [
      { name: "Quarterly plan.docx", path: "C:\\\\Users\\\\alex\\\\Documents\\\\Quarterly plan.docx", dir: false, size: 48213, age: 1800, modified: 0, kind: "file" },
      { name: "Trip photos", path: "C:\\\\Users\\\\alex\\\\Pictures\\\\Trip photos", dir: true, size: 0, age: 7200, modified: 0, kind: "folder" },
      { name: "Budget 2026.xlsx", path: "C:\\\\Users\\\\alex\\\\Documents\\\\Budget 2026.xlsx", dir: false, size: 20480, age: 86400, modified: 0, kind: "file" },
      { name: "Ideas.md", path: "C:\\\\Users\\\\alex\\\\Documents\\\\Ideas.md", dir: false, size: 2048, age: 172800, modified: 0, kind: "file" },
    ],
    system_stats: () => {
      tick++;
      return { cpu: cpu(), cores: 16, mem_used: 11.2 * 2 ** 30, mem_total: 32 * 2 ** 30, down: 2.4 * 2 ** 20, up: 310 * 1024, uptime: 3 * 3600 + 1200,
        disks: [{ root: "C:\\\\", total: 1000 * 2 ** 30, free: 412 * 2 ** 30 }],
        apps: [{ name: "Browser", cpu: 6.1, mem: 2.1 * 2 ** 30, count: 14 }, { name: "Code editor", cpu: 3.4, mem: 1.2 * 2 ** 30, count: 6 }, { name: "Music", cpu: 1.2, mem: 380 * 2 ** 20, count: 3 }] };
    },
    power: () => ({ percent: 82, charging: false, plugged: false, secs_left: 5 * 3600 + 20 * 60 }),
    net_link: "wifi",
    quick_audio: { volume: 64, muted: false, device: "Speakers" },
    quick_mic: { volume: 80, muted: false, device: "Microphone" },
    quick_brightness: 72,
    quick_dark: true,
    quick_radios: { wifi: true, bluetooth: true },
    quick_networks: [
      { ssid: "Home-5G", signal: 92, secure: true, connected: true, saved: true },
      { ssid: "Cafe Guest", signal: 58, secure: false, connected: false, saved: false },
    ],
    quick_extra: { airplane: false, hotspot: { on: false, clients: 0, name: "DEMO-PC" }, night_light: true, project: "pc", energy_saver: false, captions: false, magnifier: false, narrator: false, keyboard: false },
    quick_devices: [{ id: "spk", name: "Speakers", default: true }, { id: "hp", name: "Headphones", default: false }],
    quick_apps: [{ name: "Music", key: "music", volume: 80, muted: false, active: true }, { name: "Browser", key: "browser", volume: 100, muted: false, active: false }],
  };
  const T = window.__TAURI_INTERNALS__;
  const real = T.invoke;
  const next = Object.assign({}, T);
  Object.defineProperty(next, "invoke", {
    value: async (cmd, args) => {
      if (cmd in over) {
        window.__mock.calls.push({ cmd, t: performance.now(), args });
        const v = over[cmd];
        return typeof v === "function" ? v(args || {}) : JSON.parse(JSON.stringify(v));
      }
      return real(cmd, args);
    },
  });
  window.__TAURI_INTERNALS__ = next;
  // The CPU graph fills one point every 2 s; run that poll fast for the first moments so the graph
  // already has a history when the shot is taken.
  if (location.hash === "") {
    const st = window.setTimeout.bind(window), t0 = performance.now();
    window.setTimeout = (f, ms, ...a) => st(f, ms === 2000 && performance.now() - t0 < 2500 ? 40 : ms, ...a);
  }
})();`;

// ---- the scenes

const DOCK = [
  { id: "start", options: { opens: "freedock" } },
  "apps",
  { id: "spacer", options: { size: "small" } },
  { id: "weather", options: { unit: "c", range: true, rainAlert: true } },
  { id: "now-playing", options: { hideWhenIdle: true, skip: true } },
  { id: "cpu", options: { graph: true } },
  { id: "spacer", options: { size: "small" } },
  "system",
  { id: "date", options: { weekday: true } },
  { id: "time", options: { face: "digital", seconds: false } },
];
const LOOK = { style: "glass", glass_tint: 0.5, glass_blur: 11, glass_live: false, height: 52, margin: 10, radius: 16, hour12: true, popup_motion: "off", city: "Lisbon", lat: 38.7223, lon: -9.1393, previews: false };

// Popup sizes from popup.rs size_for (fit flyouts grow or shrink to their content).
const SIZE = { quick: [360, 300], start: [560, 640], calendar: [340, 460], weather: [360, 520], media: [300, 420], stocks: [360, 420] };

const sceneHtml = () => `<!doctype html><meta charset="utf-8">
<style>
  html, body { margin: 0; width: 100%; height: 100%; overflow: hidden; background: #1a1a4a url(wall.svg) 0 0 / 100% 100% no-repeat; }
  iframe { position: absolute; border: 0; background: transparent; }
  .dock { color-scheme: normal; border-radius: 16px; box-shadow: 0 10px 30px rgba(10, 6, 40, .35), 0 1px 3px rgba(0, 0, 0, .2); }
  .fly { color-scheme: dark; border-radius: 16px; box-shadow: 0 24px 60px rgba(10, 6, 40, .5), 0 2px 8px rgba(0, 0, 0, .25); }
</style>
<script>
  window.__wall = "data:image/svg+xml;base64,${Buffer.from(WALL).toString("base64")}";
  const frame = (src, cls, css) => {
    const f = document.createElement("iframe");
    f.className = cls;
    Object.assign(f.style, css);
    f.src = src;
    document.body.appendChild(f);
    return f;
  };
  // The dock: as wide as the screen allows until dock_fit narrows it, then centred.
  window.dock = (margin, height) => frame("/?set=showcase", "dock", { left: "20px", width: (innerWidth - 40) + "px", height: height + "px", bottom: margin + "px" });
  // A flyout above the dock, centred on x (or on the dock cell matching sel), 10 px above it.
  window.flyout = (mode, w, h, at) => {
    const d = document.querySelector("iframe.dock");
    const r = d.getBoundingClientRect();
    let x = at;
    if (typeof at === "string") {
      const c = d.contentDocument.querySelector(at).getBoundingClientRect();
      x = r.x + c.x + c.width / 2;
    }
    const left = Math.max(10, Math.min(innerWidth - w - 10, Math.round(x - w / 2)));
    return frame("/?set=showcase&mode=" + mode + "#ask", "fly", { left: left + "px", width: w + "px", height: h + "px", bottom: (innerHeight - r.y + 10) + "px" });
  };
</script>
<body></body>`;

const dir = build(join(tmpdir(), "freedock-showcase"));
const index = join(dir, "index.html");
writeFileSync(index, readFileSync(index, "utf8").replace('<script src="/mock.js"></script>', '<script src="/demo-pre.js"></script>\n    <script src="/mock.js"></script>\n    <script src="/demo.js"></script>'));
writeFileSync(join(dir, "demo-pre.js"), PRE);
writeFileSync(join(dir, "demo.js"), POST);
writeFileSync(join(dir, "wall.svg"), WALL);
writeFileSync(join(dir, "scene.html"), sceneHtml());
writeFileSync(join(dir, "settings-showcase.json"), JSON.stringify(settingsWith(DOCK, LOOK)));

const site = await serve(dir);
const page = await browser({ width: W, height: H, scale });
const errors = [];
const check = async (what) => {
  const errs = await page.eval(`[...document.querySelectorAll("iframe")].flatMap((f) => (f.contentWindow.__errs || []).filter((e) => !e.includes("ResizeObserver loop")).map((e) => f.src.split("?")[1] + ": " + e))`);
  if (errs.length) errors.push(`${what}: ${errs.join(" | ")}`);
};
await page.send("Page.addScriptToEvaluateOnNewDocument", {
  source: `window.__errs = []; addEventListener("error", (e) => window.__errs.push(String(e.message))); addEventListener("unhandledrejection", (e) => window.__errs.push(String(e.reason)));`,
});
const shots = [];
try {
  // 1. The desktop: Start above the Start button, Quick settings above the system cell.
  await page.open(`${site.url}/scene.html`, 200);
  await page.eval(`dock(${LOOK.margin}, ${LOOK.height}), true`);
  await sleep(3200);
  await page.eval(`flyout("start", ${SIZE.start}, ".cell.start"), flyout("quick", ${SIZE.quick}, ".cell.system"), true`);
  await sleep(2500);
  await check("screenshot");
  const one = join(out, "screenshot.png");
  await page.shot(one);
  shots.push(one);

  // 2. Four flyouts side by side above the dock, spread evenly.
  await page.open(`${site.url}/scene.html`, 200);
  await page.eval(`dock(${LOOK.margin}, ${LOOK.height}), true`);
  await sleep(3200);
  const row = ["calendar", "weather", "media", "stocks"];
  const total = row.reduce((a, m) => a + SIZE[m][0], 0);
  const gap = 36;
  let x = (W - total - gap * (row.length - 1)) / 2;
  const calls = [];
  for (const m of row) {
    calls.push(`flyout("${m}", ${SIZE[m]}, ${x + SIZE[m][0] / 2})`);
    x += SIZE[m][0] + gap;
  }
  await page.eval(`${calls.join(", ")}, true`);
  await sleep(2500);
  await check("flyouts");
  // Cropped to the flyouts and the dock: the sky above them is empty.
  const top = await page.eval(`Math.min(...[...document.querySelectorAll("iframe.fly")].map((f) => f.getBoundingClientRect().y))`);
  const y = Math.max(0, Math.floor(top - 40));
  const two = join(out, "flyouts.png");
  await page.shot(two, { x: 0, y, width: W, height: H - y });
  shots.push(two);
} finally {
  page.close();
  site.close();
}
for (const s of shots) console.log(`${s}  ${(statSync(s).size / 1024 / 1024).toFixed(2)} MB`);
if (errors.length) {
  console.log(`page errors:\n${errors.join("\n")}`);
  process.exitCode = 1;
}
