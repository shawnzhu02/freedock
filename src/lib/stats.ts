// System stats for the dock cells and the System flyout. Formatting is pure (tested); the poller is
// shared, so five stats cells cost one call every two seconds.
import { useEffect, useState } from "react";
import { isActive, onWake } from "./active.ts";

export type Disk = { root: string; total: number; free: number };
export type App = { name: string; cpu: number; mem: number; count: number };
export type Stats = {
  cpu: number;
  cores: number;
  mem_used: number;
  mem_total: number;
  down: number;
  up: number;
  disks: Disk[];
  apps: App[];
  uptime: number;
};

/** 300 -> "5 min", 7500 -> "2 h 5 min", 270000 -> "3 d 3 h". */
export function uptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d) return `${d} d ${h} h`;
  if (h) return `${h} h ${m} min`;
  return `${m} min`;
}

/** "512 MB", "12.4 GB", "1.8 TB" (binary units, the way Windows shows them). */
export function bytes(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }
  const digits = i === 0 || n >= 100 ? 0 : 1;
  return `${n.toFixed(digits)} ${units[i]}`;
}

/** "0 KB/s", "840 KB/s", "2.4 MB/s". */
export function speed(n: number): string {
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB/s`;
  return `${(n / 1024 / 1024).toFixed(n < 100 * 1024 * 1024 ? 1 : 0)} MB/s`;
}

/** An SVG polyline path for values 0-100 across `w` x `h`, newest at the right. */
export function sparkPath(values: number[], w: number, h: number, max = 100): string {
  if (values.length < 2) return "";
  const step = w / (values.length - 1);
  return values
    .map((v, i) => `${i ? "L" : "M"}${(i * step).toFixed(1)} ${(h - (Math.min(max, Math.max(0, v)) / max) * h).toFixed(1)}`)
    .join(" ");
}

// ---- shared poller (not used by tests)

type Listener = (s: Stats, history: number[]) => void;
const listeners = new Set<Listener>();
let wantApps = 0;
let timer = 0;
let last: Stats | null = null;
const history: number[] = [];
/** Bumped whenever a poll loop starts or stops, so an in-flight tick from an older loop ends there. */
let loopId = 0;

async function tick(loop = ++loopId) {
  const { invoke } = await import("@tauri-apps/api/core");
  // Tucked away or hidden: nobody sees the numbers, so don't ask for them (onWake restarts at once).
  if (isActive()) try {
    const s = await invoke<Stats>("system_stats", { apps: wantApps > 0 });
    last = s;
    history.push(s.cpu);
    if (history.length > 60) history.shift();
    listeners.forEach((f) => f(s, history.slice()));
  } catch {
    /* next tick */
  }
  if (loop !== loopId) return;
  timer = listeners.size ? window.setTimeout(() => tick(loop), isActive() ? 2000 : 8000) : 0;
}

/** Live stats; `apps` asks for the busiest-apps list too (the flyout's job). */
export function useStats(apps = false): { s: Stats | null; history: number[] } {
  const [v, setV] = useState<{ s: Stats | null; history: number[] }>({ s: last, history: history.slice() });
  useEffect(() => {
    const f: Listener = (s, h) => setV({ s, history: h });
    listeners.add(f);
    if (apps) wantApps++;
    if (!timer || apps) {
      clearTimeout(timer);
      tick();
    }
    const offWake = onWake(() => {
      clearTimeout(timer);
      tick();
    });
    return () => {
      offWake();
      listeners.delete(f);
      if (apps) wantApps--;
      if (!listeners.size) {
        clearTimeout(timer);
        timer = 0;
        loopId++;
      }
    };
  }, [apps]);
  return v;
}

// ---- GPU (its own poller: the counters only run while a GPU cell or the System flyout is shown)

export type Gpu = { name: string; load: number; engine: string; mem_used: number; mem_total: number };

/** "NVIDIA GeForce RTX 5070 Laptop GPU" -> "RTX 5070"; "Intel(R) UHD Graphics" -> "Intel UHD". */
export function gpuShort(name: string): string {
  const n = name.replace(/\((R|TM)\)/gi, "").replace(/\s+/g, " ").trim();
  const m = n.match(/\b(RTX|GTX|RX|Arc)\s?[A-Z]?\d{3,4}\s?(Ti|XTX|XT|Super)?\b/i);
  if (m) return m[0].replace(/\s+/g, " ").trim();
  if (/intel/i.test(n)) return n.replace(/Graphics/i, "").replace(/Corporation/i, "").replace(/\s+/g, " ").trim();
  if (/radeon/i.test(n)) return n.replace(/AMD/i, "").replace(/Graphics/i, "").trim();
  return n;
}

/** "Compute" -> "compute", "VideoDecode" -> "video decode", "3D" -> "3D". */
export function engineLabel(e: string): string {
  return e === "3D" ? "3D" : e.replace(/_\d+$/, "").replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase();
}

type GpuListener = (g: Gpu[], history: number[]) => void;
const gpuListeners = new Set<GpuListener>();
let gpuTimer = 0;
let gpuLast: Gpu[] | null = null;
const gpuHistory: number[] = [];
/** Bumped whenever a new poll loop starts, so an older in-flight tick doesn't start a second loop. */
let gpuLoop = 0;

async function gpuTick(loop = gpuLoop) {
  const { invoke } = await import("@tauri-apps/api/core");
  // Tucked away or hidden: skip the read, like tick() (onWake starts a fresh loop at once).
  if (isActive()) try {
    const g = await invoke<Gpu[]>("gpu_stats");
    gpuLast = g;
    gpuHistory.push(g[0]?.load ?? 0);
    if (gpuHistory.length > 60) gpuHistory.shift();
    gpuListeners.forEach((f) => f(g, gpuHistory.slice()));
  } catch {
    /* next tick */
  }
  if (loop !== gpuLoop) return;
  if (gpuListeners.size) gpuTimer = window.setTimeout(() => gpuTick(loop), isActive() ? 2000 : 8000);
  else gpuTimer = 0;
}

/** Live GPU readings, discrete card first. */
export function useGpu(): { g: Gpu[] | null; history: number[] } {
  const [v, setV] = useState<{ g: Gpu[] | null; history: number[] }>({ g: gpuLast, history: gpuHistory.slice() });
  useEffect(() => {
    const f: GpuListener = (g, h) => setV({ g, history: h });
    gpuListeners.add(f);
    if (!gpuTimer) {
      gpuTimer = -1;
      gpuTick(++gpuLoop);
    }
    const offWake = onWake(() => {
      // One restart per wake, however many cells show the GPU: only the oldest listener does it.
      if (gpuListeners.values().next().value !== f) return;
      clearTimeout(gpuTimer);
      gpuTimer = -1;
      gpuTick(++gpuLoop);
    });
    return () => {
      offWake();
      gpuListeners.delete(f);
      if (!gpuListeners.size) {
        clearTimeout(gpuTimer);
        gpuTimer = 0;
      }
    };
  }, []);
  return v;
}
