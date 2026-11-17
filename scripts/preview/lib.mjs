// Shared helpers for the preview tools: build the UI with the fake backend, serve it, and drive a
// headless Microsoft Edge over the DevTools protocol (Node's built-in WebSocket; nothing to install).
import { execFileSync, spawn } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, join, resolve } from "node:path";

export const ROOT = resolve(import.meta.dirname, "..", "..");
const HERE = import.meta.dirname;

/** Builds the UI into `out` (default: a temp folder) with mock.js injected before the app's scripts. */
export function build(out = join(tmpdir(), "freedock-preview")) {
  const vite = join(ROOT, "node_modules", "vite", "bin", "vite.js");
  execFileSync(process.execPath, [vite, "build", "--outDir", out, "--emptyOutDir", "--logLevel", "warn"], { cwd: ROOT, stdio: "inherit" });
  const index = join(out, "index.html");
  writeFileSync(index, readFileSync(index, "utf8").replace("<head>", '<head>\n    <script src="/mock.js"></script>'));
  cpSync(join(HERE, "mock.js"), join(out, "mock.js"));
  if (!existsSync(join(out, "settings.json"))) cpSync(join(HERE, "base-settings.json"), join(out, "settings.json"));
  return out;
}

/** The base settings with this dock layout: `widgets` is a list of widget ids or {id, options}. */
export function settingsWith(widgets, look = {}) {
  const base = JSON.parse(readFileSync(join(HERE, "base-settings.json"), "utf8"));
  const list = widgets.map((w) => (typeof w === "string" ? { id: w, enabled: true, options: {} } : { enabled: true, options: {}, ...w }));
  return { ...base, ...look, widgets: list, moods: [{ ...base.moods[0], widgets: list }] };
}

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".json": "application/json", ".png": "image/png", ".svg": "image/svg+xml", ".woff2": "font/woff2" };

/** Serves `dir` on 127.0.0.1 (port 0 = any free port). Resolves to { url, close }. */
export function serve(dir, port = 0) {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
    const file = resolve(dir, "." + (path === "/" ? "/index.html" : path));
    if (!file.startsWith(resolve(dir)) || !existsSync(file)) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" }).end(readFileSync(file));
  });
  return new Promise((ok) => server.listen(port, "127.0.0.1", () => ok({ url: `http://127.0.0.1:${server.address().port}`, close: () => server.close() })));
}

const EDGE = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
];

/** Starts a headless Edge with its own throwaway profile and opens one page. */
export async function browser({ width = 1600, height = 400, scale = 2 } = {}) {
  const exe = process.env.FREEDOCK_EDGE ?? EDGE.find(existsSync);
  if (!exe) throw new Error("Microsoft Edge not found; set FREEDOCK_EDGE to its msedge.exe");
  const profile = join(tmpdir(), `freedock-preview-edge-${process.pid}`);
  mkdirSync(profile, { recursive: true });
  const port = 9300 + (process.pid % 500);
  const proc = spawn(exe, ["--headless=new", `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, "--no-first-run", "--hide-scrollbars", `--window-size=${width},${height}`, "about:blank"], { stdio: "ignore" });
  let target;
  for (let i = 0; i < 50 && !target; i++) {
    await sleep(200);
    target = await fetch(`http://127.0.0.1:${port}/json/list`).then((r) => r.json()).then((l) => l.find((t) => t.type === "page")).catch(() => null);
  }
  if (!target) throw new Error("Edge did not start");
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((ok) => ws.addEventListener("open", ok));
  let id = 0;
  const waiting = new Map();
  const events = [];
  ws.addEventListener("message", (m) => {
    const msg = JSON.parse(m.data);
    if (msg.id && waiting.has(msg.id)) {
      waiting.get(msg.id)(msg);
      waiting.delete(msg.id);
    } else if (msg.method) events.push(msg.method);
  });
  const send = (method, params = {}) =>
    new Promise((ok, fail) => {
      const n = ++id;
      waiting.set(n, (msg) => (msg.error ? fail(new Error(`${method}: ${msg.error.message}`)) : ok(msg.result)));
      ws.send(JSON.stringify({ id: n, method, params }));
    });
  await send("Page.enable");
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: scale, mobile: false });
  const page = {
    send,
    async open(url, settle = 1200) {
      events.length = 0;
      await send("Page.navigate", { url });
      for (let i = 0; i < 100 && !events.includes("Page.loadEventFired"); i++) await sleep(50);
      await sleep(settle);
    },
    eval: (expression) => send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }).then((r) => r.result.value),
    hover: (x, y) => send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y }),
    async shot(file, clip) {
      const r = await send("Page.captureScreenshot", { format: "png", ...(clip ? { clip: { ...clip, scale: 1 } } : {}), captureBeyondViewport: !!clip });
      writeFileSync(file, Buffer.from(r.data, "base64"));
    },
    async size(w, h) {
      await send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: scale, mobile: false });
    },
    close() {
      ws.close();
      proc.kill();
    },
  };
  return page;
}

export const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
