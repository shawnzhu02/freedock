// Is anyone looking? Pollers ask this before doing work, so a tucked-away dock or a hidden flyout
// costs nothing. `document.visibilityState` alone isn't enough: tucking the dock only slides it off
// screen (a CSS transform), so the page still reports "visible".
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

let dockShown = true;
const wakers = new Set<() => void>();

try {
  if (getCurrentWindow().label === "dock") {
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

/** True when this window is on screen and, for the dock, not tucked away. */
export function isActive(): boolean {
  return document.visibilityState === "visible" && dockShown;
}

/** Calls `f` whenever the dock comes back into view. Returns an unsubscribe. */
export function onWake(f: () => void): () => void {
  wakers.add(f);
  return () => wakers.delete(f);
}
