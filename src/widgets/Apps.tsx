import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useReorder } from "../lib/useReorder";
import type { WidgetProps } from "./registry";

// Mirrors src-tauri/src/apps.rs.
type Win = { hwnd: number; title: string; active: boolean; minimized: boolean; flashing: boolean };
type App = { key: string; name: string; pinned: boolean; lnk: string | null; exe: string | null; windows: Win[]; icon: string | null };

/** Icons by source path, fetched once per program for the life of the dock. */
const icons = new Map<string, string | null>();
const asked = new Set<string>();
/** Every mounted icon waiting on a source, so all copies of an app (Pinned and All apps in Start, say)
 *  redraw when its icon arrives, not only the one that asked first. */
const waiting = new Map<string, Set<() => void>>();
const wake = (src: string) => waiting.get(src)?.forEach((f) => f());

export function AppIcon({ src, name }: { src: string | null; name: string }) {
  const [tries, redraw] = useState(0);
  useEffect(() => {
    if (!src || icons.get(src)) return;
    const f = () => redraw((n) => n + 1);
    let set = waiting.get(src);
    if (!set) waiting.set(src, (set = new Set()));
    set.add(f);
    if (!asked.has(src)) {
      asked.add(src);
      invoke<string | null>("app_icon", { path: src })
        .then((u) => icons.set(src, u ?? null))
        .catch(() => icons.set(src, null))
        .finally(() => {
          wake(src);
          // No icon this time (the program was busy, or Windows wasn't ready): ask again later.
          if (!icons.get(src) && tries < 3)
            setTimeout(() => {
              asked.delete(src);
              wake(src);
            }, 30_000);
        });
    }
    return () => {
      set.delete(f);
      if (!set.size) waiting.delete(src);
    };
  }, [src, tries]);
  const url = src ? icons.get(src) : null;
  return url ? <img src={url} alt="" draggable={false} /> : <span className="glyph">{name.slice(0, 1)}</span>;
}

/** Pinned apps and open windows. Click focuses or minimizes, middle-click opens another window,
 *  right-click opens the app's menu, and pinned apps can be dragged into a new order. */
export default function Apps({ settings }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  const [apps, setApps] = useState<App[]>([]);
  const last = useRef("");
  const load = useCallback(() => {
    invoke<App[]>("apps_list")
      .then((l) => {
        // Most updates change nothing the dock shows; skip the re-render then.
        const k = JSON.stringify(l);
        if (k !== last.current) {
          last.current = k;
          setApps(l);
        }
      })
      .catch(() => {});
  }, []);
  useEffect(() => {
    load();
    const off = listen("apps-changed", load);
    return () => {
      off.then((f) => f());
    };
  }, [load]);

  // Pins drag among themselves; the order is saved when the drag ends. A drag ends with a click on
  // the dragged icon, which must not open the app.
  const pinned = apps.filter((a) => a.pinned);
  const dragged = useRef(false);
  // Previews come after the pointer rests on an open app for a moment, and move with it.
  const hover = useRef(0);
  const showing = useRef(false);
  const rest = (a: App, el: HTMLElement) => {
    clearTimeout(hover.current);
    if (!settings.previews || !a.windows.length) return;
    hover.current = window.setTimeout(() => {
      const r = el.getBoundingClientRect();
      showing.current = true;
      invoke("preview_open", { key: a.key, anchor: vertical ? r.top + r.height / 2 : r.left + r.width / 2 }).catch(() => {});
    }, showing.current ? 60 : 450);
  };
  const leave = () => {
    clearTimeout(hover.current);
  };
  useEffect(() => {
    const off = listen("popup-hidden", () => (showing.current = false));
    return () => {
      clearTimeout(hover.current);
      off.then((f) => f());
    };
  }, []);
  // Files dragged from Explorer onto an app's icon open with that app; the icon under the pointer
  // lights up while dragging.
  const [dropOn, setDropOn] = useState<string | null>(null);
  useEffect(() => {
    const keyAt = (x: number, y: number) => {
      const k = window.devicePixelRatio || 1;
      return (document.elementFromPoint(x / k, y / k)?.closest("[data-app]") as HTMLElement | null)?.dataset.app ?? null;
    };
    const off = getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "over") setDropOn(keyAt(p.position.x, p.position.y));
      else if (p.type === "drop") {
        const key = keyAt(p.position.x, p.position.y);
        setDropOn(null);
        if (key && p.paths.length) invoke("app_open_files", { key, paths: p.paths });
      } else setDropOn(null);
    });
    return () => {
      off.then((f) => f());
    };
  }, []);
  const { dragging, item } = useReorder(
    "pins",
    pinned,
    (next) => setApps([...next, ...apps.filter((a) => !a.pinned)]),
    (final) => {
      dragged.current = true;
      invoke("pins_order", { keys: final.map((a) => a.key) }).catch(() => {});
    },
  );

  return (
    <div className="cell apps" role="toolbar" aria-label="Apps">
      {apps.map((a, i) => {
        const open = a.windows.length > 0;
        const active = a.windows.some((w) => w.active);
        const flashing = a.windows.some((w) => w.flashing);
        const title = a.windows.length > 1 ? `${a.name} · ${a.windows.length} windows` : a.windows[0]?.title || a.name;
        return (
          <button
            key={a.key}
            {...(a.pinned ? { ...item(i), "data-grip": "" } : {})}
            onPointerDown={(e) => a.pinned && !e.altKey && item(i).onPointerDown(e)}
            data-app={a.key}
            className={`app${open ? " open" : ""}${active ? " active" : ""}${flashing ? " flashing" : ""}${dragging === i ? " dragging" : ""}${dropOn === a.key ? " dropon" : ""}`}
            title={title}
            aria-label={open ? `${a.name}, ${a.windows.length} window${a.windows.length > 1 ? "s" : ""}` : a.name}
            onMouseEnter={(e) => rest(a, e.currentTarget)}
            onMouseLeave={leave}
            onClick={() => {
              clearTimeout(hover.current);
              if (dragged.current) return void (dragged.current = false);
              invoke("app_click", { key: a.key });
            }}
            onAuxClick={(e) => e.button === 1 && invoke("app_launch", { key: a.key })}
            onContextMenu={(e) => {
              clearTimeout(hover.current);
              e.preventDefault();
              e.stopPropagation();
              const r = e.currentTarget.getBoundingClientRect();
              invoke("popup_open", { mode: "appmenu", anchor: vertical ? r.top + r.height / 2 : r.left + r.width / 2, arg: a.key });
            }}
          >
            <AppIcon src={a.icon} name={a.name} />
            {open && <i className="dot" />}
          </button>
        );
      })}
    </div>
  );
}
