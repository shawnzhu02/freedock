// Is anyone looking? Pollers ask this before doing work, so a tucked-away dock or a hidden flyout
// costs nothing. `document.visibilityState` alone isn't enough: tucking the dock only slides it off
// screen (a CSS transform), so the page still reports "visible".
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

let dockShown = true;
const wakers = new Set<() => void>();

try {
  if (getCurrentWindow().label === "dock") {
    // Rust sends `dock-visibility` only when it changes, so a dock that starts tucked (Smart with a
    // maximized window, or Auto-hide) must ask once, or every poller runs until it is first shown.
    invoke<boolean>("dock_visible")
      .then((v) => {
        if (typeof v === "boolean") dockShown = v;
      })
      .catch(() => {});
    listen<boolean>("dock-visibility", (e) => {
      const was = dockShown;
      dockShown = e.payload;
      // Coming back into view: let pollers refresh right away instead of waiting a whole interval.
      if (dockShown && !was) wakers.forEach((f) => f());
    });
  }
} catch {
  // Not inside Tauri (unit tests): always active.
}

// The window itself coming back (restored, unlocked, uncovered) counts as waking too.
if (typeof document !== "undefined") {
  document.addEventListener("visibilitychange", () => {
    if (isActive()) wakers.forEach((f) => f());
  });
}

/** True when this window is on screen and, for the dock, not tucked away. */
export function isActive(): boolean {
  return document.visibilityState === "visible" && dockShown;
}

/** Calls `f` whenever the dock comes back into view. Returns an unsubscribe. */
export function onWake(f: () => void): () => void {
  wakers.add(f);
  return () => wakers.delete(f);
}

/** Runs `f` every `ms` while this window is active. A tick that falls while the dock is tucked away (or
 *  the window is hidden) is skipped, and the skipped work runs as soon as it is back in view. Without
 *  that, an auto-hiding dock that shows for a few seconds at a time almost never lands on a tick, and a
 *  battery or Wi-Fi cell could stay minutes or hours out of date. Returns a stop function. */
export function pollActive(ms: number, f: () => void): () => void {
  let missed = false;
  const t = window.setInterval(() => {
    if (isActive()) {
      missed = false;
      f();
    } else missed = true;
  }, ms);
  const off = onWake(() => {
    if (missed) {
      missed = false;
      f();
    }
  });
  return () => {
    clearInterval(t);
    off();
  };
}
