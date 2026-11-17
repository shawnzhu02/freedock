// A fake Tauri backend, so the UI can be rendered in a plain browser without touching the real dock.
// scripts/preview/serve.mjs injects this file before the app's own scripts.
//
// - Settings come from `settings.json` next to the page, or `settings-<name>.json` with `?set=<name>`.
// - `#ask` renders the flyout window (pick the flyout with `?mode=<mode>`), `#settings` the Settings
//   window, and no hash the dock.
// - Every command in `data` below returns that sample. Unknown commands return null, which widgets treat
//   as "nothing yet". `?lag=1` makes every command answer after 20-400 ms (fixed pseudo-random order),
//   the way the real backend fills a view in piece by piece.
// - `window.__mock.calls` lists every command with its time (performance.now()), and
//   `window.__mock.emit(event, payload)` sends an event to the page's listeners (for example
//   "popup-mode" to open a flyout the way Rust does). Add a sample here when you build a widget with a new command, so previews and
//   the hover sweep (states.mjs) show it filled in.
(function () {
  const q = new URLSearchParams(location.search);
  const label = location.hash === "#ask" ? "popup" : location.hash === "#settings" ? "settings" : "dock";
  const set = q.get("set");
  let settings = null;
  const load = fetch(set ? `settings-${set}.json` : "settings.json")
    .then((r) => r.json())
    .then((s) => (settings = s));
  const pts = (a, n, d) => Array.from({ length: n }, (_, i) => a + Math.sin(i / 3) * d + i * d * 0.1);
  const svg = (body, w = 160, h = 160) => "data:image/svg+xml;base64," + btoa(`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">${body}</svg>`);
  const photo = svg(
    '<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ff9a5a"/><stop offset="1" stop-color="#5a6bff"/></linearGradient></defs><rect width="160" height="160" fill="url(#g)"/><circle cx="110" cy="50" r="18" fill="#ffe9a8"/>',
  );
  const icon = (c) => svg(`<rect x="2" y="2" width="28" height="28" rx="7" fill="${c}"/>`, 32, 32);
  const now = new Date();
  const hhmm = (min) => {
    const d = new Date(now.getTime() + min * 60000);
    return `${d.getHours()}:${String(d.getMinutes()).padStart(2, "0")}`;
  };
  const win = (hwnd, title, extra = {}) => ({ hwnd, title, active: false, minimized: false, flashing: false, ...extra });
  const data = {
    stocks_quotes: [
      { symbol: "^GSPC", name: "S&P 500", price: 7426.67, change: -1.79, currency: "USD", points: pts(7500, 20, 30) },
      { symbol: "AAPL", name: "Apple Inc.", price: 352.42, change: 1.08, currency: "USD", points: pts(348, 20, 2) },
      { symbol: "BTC-USD", name: "Bitcoin USD", price: 64895.23, change: -0.64, currency: "USD", points: pts(65000, 40, 300) },
    ],
    fx_rates: { base: "USD", date: "2026-11-16", rates: { EUR: 0.90794, GBP: 0.75767, JPY: 159.6, CAD: 1.39 } },
    news_headlines: [
      { title: "Central bank holds rates steady as inflation cools for a third month", link: "https://example.com/1", source: "Example News", age: "12m" },
      { title: "New bridge opens after four years of construction", link: "https://example.com/2", source: "Example News", age: "1h" },
      { title: "Local team wins the championship in overtime", link: "https://example.com/3", source: "Example News", age: "3h" },
    ],
    photo_frame: { path: "C:\\Users\\alex\\Pictures\\trip.jpg", url: photo },
    shortcuts_list: [
      { name: "Documents", target: "C:\\Users\\alex\\Documents", kind: "path" },
      { name: "GitHub", target: "https://github.com", kind: "web" },
      { name: "Settings", target: "ms-settings:", kind: "settings" },
    ],
    apps_list: [
      { key: "edge", name: "Browser", pinned: true, lnk: null, exe: null, icon: icon("#2b7cff"), windows: [win(1, "Example page", { active: true })] },
      { key: "files", name: "File Explorer", pinned: true, lnk: null, exe: null, icon: icon("#f2b632"), windows: [] },
      { key: "code", name: "Code editor", pinned: false, lnk: null, exe: null, icon: icon("#7b5cff"), windows: [win(2, "main.rs"), win(3, "README.md")] },
      { key: "chat", name: "Chat", pinned: false, lnk: null, exe: null, icon: icon("#30d158"), windows: [win(4, "Chat", { flashing: true })] },
    ],
    app_icon: null,
    tray_count: 6,
    tray_list: [
      { id: "1", tooltip: "Cloud sync: up to date", icon: icon("#5ac8fa") },
      { id: "2", tooltip: "Volume mixer", icon: icon("#ff9f0a") },
      { id: "3", tooltip: "Antivirus", icon: icon("#34c759") },
    ],
    media_now: { title: "Morning Light", artist: "Example Band", album: "Sunrise", app: "Music", playing: true, position: 42, duration: 210, key: "m1", can_prev: true, can_next: true },
    media_art: photo,
    recycle_bin: { bytes: 734003200, items: 12 },
    agenda_today: {
      events: [
        { time: hhmm(12), title: "Design review", place: "https://meet.google.com/abc-defg-hij" },
        { time: hhmm(150), title: "Lunch with Sam", place: "Cafe" },
      ],
      today: [{ id: "t1", title: "Send the slides" }],
      soon: [],
      school: [],
    },
    start_state: { pins: ["files", "code"], recent: ["edge"], user: "Alex" },
    start_apps: [
      { name: "Browser", id: "edge" },
      { name: "Code editor", id: "code" },
      { name: "File Explorer", id: "files" },
      { name: "Chat", id: "chat" },
    ],
    files_list: () => [
      { name: "Report.docx", path: "C:\Users\alex\Documents\Report.docx", dir: false, size: 48213, age: 3600, modified: 0, kind: "file" },
      { name: "Photos", path: "C:\Users\alex\Pictures", dir: true, size: 0, age: 86400, modified: 0, kind: "folder" },
    ],
    clip_list: [
      { id: 2, text: "https://example.com/notes", at: 0 },
      { id: 1, text: "Meeting moved to 3 pm", at: 0 },
    ],
    dock_visible: true,
    popup_state: q.get("mode") ? { mode: q.get("mode"), prefill: "" } : null,
    // `?wall=split` gives the glass a wallpaper that is bright on the left and dark on the right, to
    // check that text over light glass turns dark. The window is treated as the bottom of a 1000 px
    // tall screen.
    wallpaper: () => ({
      data:
        q.get("wall") === "split"
          ? svg('<defs><linearGradient id="w" x1="0" x2="1"><stop offset="0" stop-color="#fdfcf7"/><stop offset=".45" stop-color="#f0ece0"/><stop offset=".55" stop-color="#2a3550"/><stop offset="1" stop-color="#121826"/></linearGradient></defs><rect width="1600" height="1000" fill="url(#w)"/>', 1600, 1000)
          : null,
      fit: "fill",
    }),
    geometry: () => ({ mon_w: Math.max(innerWidth, 1600), mon_h: Math.max(innerHeight, 1000), left: 0, top: Math.max(0, 1000 - innerHeight), width: innerWidth, height: innerHeight }),
    "plugin:app|version": "0.1.0",
  };
  let cb = 0;
  const listeners = {};
  const calls = [];
  let seed = 7;
  const lag = q.get("lag") ? () => new Promise((ok) => setTimeout(ok, 20 + ((seed = (seed * 48271) % 2147483647) % 381))) : () => null;
  window.__mock = {
    calls,
    emit: (event, payload) => (listeners[event] ?? []).forEach((h) => window["_" + h]?.({ event, id: h, payload })),
  };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label }, currentWebview: { windowLabel: label, label } },
    transformCallback: (f) => {
      const id = ++cb;
      window["_" + id] = f;
      return id;
    },
    convertFileSrc: (p) => p,
  };
  // Read-only, like the real one (Tauri defines it with Object.defineProperty): a page that tries to
  // replace it must fail here too, not only in the real app.
  Object.defineProperty(window.__TAURI_INTERNALS__, "invoke", {
    value: async (cmd, args) => {
      if (cmd === "plugin:event|listen") {
        (listeners[args.event] ??= []).push(args.handler);
        return args.handler;
      }
      if (cmd.startsWith("plugin:event|")) return 1;
      calls.push({ cmd, t: performance.now(), args });
      if (cmd === "get_settings" || cmd === "default_settings") return (await load, JSON.parse(JSON.stringify(settings)));
      await lag();
      if (typeof data[cmd] === "function") return data[cmd]();
      if (cmd in data) return JSON.parse(JSON.stringify(data[cmd]));
      return null;
    },
  });
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
})();
