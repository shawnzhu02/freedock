import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Win = { hwnd: number; title: string; active: boolean; minimized: boolean };
type App = { key: string; name: string; windows: Win[] };

/** Hover previews: one box per window, which the compositor fills with a live thumbnail
 *  (src-tauri/src/thumbs.rs). Click a box to switch to that window; x closes it. */
/** An element's box in the page as laid out, ignoring transforms: the card is scaled while it opens,
 *  and the thumbnails must land where the boxes end up, not where they are mid-animation. */
function layoutBox(el: HTMLElement) {
  let left = 0;
  let top = 0;
  for (let e: HTMLElement | null = el; e; e = e.offsetParent as HTMLElement | null) {
    left += e.offsetLeft;
    top += e.offsetTop;
  }
  return { left, top, width: el.offsetWidth, height: el.offsetHeight };
}

/** `fade`: let the thumbnails fade in as the card opens (off when flyout animations are off). */
export default function Thumbs({ appKey, opened, fade }: { appKey: string; opened: number; fade: boolean }) {
  const [app, setApp] = useState<App | null>(null);
  const boxes = useRef<(HTMLDivElement | null)[]>([]);
  // apps_list is slow and answers can come back out of order: when the pointer has moved on to
  // another app, an answer for the previous one must not land here.
  const latest = useRef(appKey);
  latest.current = appKey;
  const load = () =>
    invoke<App[]>("apps_list")
      .then((l) => latest.current === appKey && setApp(l.find((a) => a.key === appKey) ?? null))
      .catch(() => latest.current === appKey && setApp(null));
  useEffect(() => {
    load();
  }, [appKey, opened]);
  // After layout, tell Rust where each box is (physical px), so the thumbnails land in them.
  useLayoutEffect(() => {
    if (!app) return;
    const k = window.devicePixelRatio || 1;
    const items = app.windows.slice(0, 4).flatMap((w, i) => {
      const el = boxes.current[i];
      if (!el) return [];
      const r = layoutBox(el);
      return [[w.hwnd, Math.round(r.left * k), Math.round(r.top * k), Math.round(r.width * k), Math.round(r.height * k)]];
    });
    const animate = fade && !matchMedia("(prefers-reduced-motion: reduce)").matches;
    invoke("thumbs_place", { items, fade: animate }).catch(() => {});
  }, [app]);
  if (!app) return <div className="view" />;
  return (
    <div className="thumbs" role="list" aria-label={`${app.name} windows`}>
      {app.windows.slice(0, 4).map((w, i) => (
        <div className={`thm${w.active ? " on" : ""}`} role="listitem" key={w.hwnd}>
          <div className="thtop">
            <span title={w.title}>{w.title || app.name}</span>
            <button
              className="x"
              title="Close this window"
              aria-label={`Close ${w.title || app.name}`}
              onClick={() => {
                invoke("app_window", { hwnd: w.hwnd, action: "close" });
                setTimeout(load, 400);
              }}
            >
              ×
            </button>
          </div>
          <div
            className="thimg"
            ref={(el) => {
              boxes.current[i] = el;
            }}
            onClick={() => {
              invoke("app_window", { hwnd: w.hwnd, action: "focus" });
              invoke("popup_close");
            }}
          >
            {w.minimized && <small>Minimized</small>}
          </div>
        </div>
      ))}
    </div>
  );
}
