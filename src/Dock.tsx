import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getSettings, isVertical, onSettings, type Settings } from "./settings";
import Glass, { GLASS_STYLES } from "./widgets/Glass";
import { byId } from "./widgets/registry";
import { slotKeys } from "./lib/moods";
import { isLight, span } from "./lib/contrast";


/** The dock: one row of rounded cells on a glass bar, in the order the settings list them.
 *  A pill grows and shrinks to hug its cells. Right-click opens the settings panel. */
export default function Dock() {
  // How bright the glass is along the bar (from Glass), and the cells marked as sitting on light glass.
  const luma = useRef<number[]>([]);
  const dockEl = useRef<HTMLElement>(null);
  const tone = useCallback(() => {
    const dock = dockEl.current, cells = row.current?.children;
    if (!dock || !cells) return;
    const d = dock.getBoundingClientRect();
    const wide = d.width >= d.height;
    for (const el of Array.from(cells) as HTMLElement[]) {
      const r = el.getBoundingClientRect();
      const [a, b] = wide ? [(r.left - d.left) / d.width, (r.right - d.left) / d.width] : [(r.top - d.top) / d.height, (r.bottom - d.top) / d.height];
      const light = luma.current.length > 0 && isLight(el.dataset.bg === "light", span(luma.current, a, b));
      if (light) el.dataset.bg = "light";
      else delete el.dataset.bg;
    }
  }, []);
  const onLuma = useCallback(
    (b: number[]) => {
      luma.current = b;
      tone();
    },
    [tone],
  );
  // True while an app's hover preview is up (the popup window in "thumbs" mode).
  const previewing = useRef(false);
  const endPreview = useRef<number | undefined>(undefined);
  useEffect(() => {
    const a = listen("preview-shown", () => (previewing.current = true));
    const b = listen("popup-hidden", () => (previewing.current = false));
    return () => {
      a.then((f) => f());
      b.then((f) => f());
    };
  }, []);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [glassOk, setGlassOk] = useState(false);
  // Tucked into the handle (smart or auto-hide) or showing; Rust watches the pointer and decides.
  const [shown, setShown] = useState(true);
  // Move mode: an outline, a Done cell, and any drag on the dock moves it to another edge or place.
  const [editing, setEditing] = useState(false);
  // A drag ends with the button released over the same cell; that click must not open anything.
  const dragged = useRef(false);
  const row = useRef<HTMLDivElement>(null);
  // 0 = room for everything; 1 = text cells narrower; 2 = text cells show only their label/art.
  const [tight, setTight] = useState(0);
  const [fading, setFading] = useState(false);
  useEffect(() => {
    getSettings().then(setSettings).catch(() => setSettings(null));
    const off = onSettings(setSettings);
    invoke<boolean>("dock_visible").then(setShown).catch(() => {});
    const offVis = listen<boolean>("dock-visibility", (e) => setShown(e.payload));
    const offEdit = listen<boolean>("edit-mode", (e) => setEditing(e.payload));
    // A mood switch fades the new row in (CSS skips it when Windows asks for less motion).
    let fadeT = 0;
    const offMood = listen("mood-changed", () => {
      setFading(true);
      clearTimeout(fadeT);
      fadeT = window.setTimeout(() => setFading(false), 200);
    });
    return () => {
      offMood.then((f) => f());
      clearTimeout(fadeT);
      off.then((f) => f());
      offVis.then((f) => f());
      offEdit.then((f) => f());
    };
  }, []);
  const onReady = useCallback((ok: boolean) => setGlassOk(ok), []);
  const shape = settings?.shape ?? "pill";
  const ready = settings !== null;
  const vertical = settings ? isVertical(settings) : false;

  // The row is max-content long, so it measures what the tiles need even while the window is shorter.
  // Upright (left/right) docks measure height instead of width.
  useLayoutEffect(() => {
    if (!ready || shape !== "pill" || !row.current) return;
    const el = row.current;
    let last = 0;
    const ro = new ResizeObserver(() => {
      const box = el.getBoundingClientRect();
      const w = Math.ceil(vertical ? box.height : box.width) + 2 * 7;
      if (w !== last) {
        last = w;
        invoke("dock_fit", { width: w }).catch(() => {});
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [shape, ready, vertical]);

  // Too many cells for the widest the dock may be: compact the text cells a step at a time. The window
  // is at its cap when it's narrower than the row; checked after each fit settles.
  // Keyed on the widgets' content: every settings-changed event brings a new array, and resetting on
  // each would make an overfull dock re-expand and re-compact (flicker) on any unrelated change.
  const widgetsKey = JSON.stringify(settings?.widgets ?? []);
  useEffect(() => {
    setTight(0);
  }, [widgetsKey, settings?.pill_width, settings?.edge, settings?.icon_size, settings?.height]);
  // Cells moved or the style changed: work out each cell's ink again (none without the glass).
  const glassOn = !!settings && GLASS_STYLES.includes(settings.style) && glassOk;
  useLayoutEffect(() => {
    if (!glassOn) luma.current = [];
    tone();
  }, [widgetsKey, glassOn, tone, settings?.edge, settings?.shape]);
  useEffect(() => {
    if (!ready || !row.current) return;
    const el = row.current;
    let t = 0;
    const check = () => {
      clearTimeout(t);
      t = window.setTimeout(() => {
        const box = el.getBoundingClientRect();
        const need = Math.ceil(vertical ? box.height : box.width) + 2 * 7;
        const have = vertical ? window.innerHeight : window.innerWidth;
        if (need > have + 1) setTight((n) => Math.min(2, n + 1));
      }, 250);
    };
    const ro = new ResizeObserver(check);
    ro.observe(el);
    window.addEventListener("resize", check);
    check();
    return () => {
      clearTimeout(t);
      ro.disconnect();
      window.removeEventListener("resize", check);
    };
  }, [ready, vertical, tight]);

  if (!settings) return null;
  const glass = GLASS_STYLES.includes(settings.style);
  // An app's hover preview belongs to that app: resting on another widget closes it (Rust closes it
  // once the pointer leaves both the dock and the preview). Only another widget counts, and only
  // after a moment: on the way from an app up to its preview the pointer crosses the dock's padding
  // and may clip a neighbor, and that must not close the preview.
  const overDock = (e: React.MouseEvent) => {
    const t = e.target as HTMLElement;
    clearTimeout(endPreview.current);
    if (!previewing.current || t.closest(".apps") || !t.closest(".cell")) return;
    endPreview.current = window.setTimeout(() => {
      previewing.current = false;
      invoke("preview_end").catch(() => {});
    }, 250);
  };
  const vars = {
    "--accent": settings.accent,
    "--radius": `${shape === "pill" ? settings.radius : 0}px`,
    "--icon": `${settings.icon_size}px`,
    "--magnify": settings.magnify,
  } as CSSProperties;

  return (
    <>
    <main
      ref={dockEl}
      className={`dock style-${settings.style} shape-${shape} edge-${settings.edge}${vertical ? " vertical" : ""}${glass && glassOk ? " has-glass" : ""}${shown ? "" : " tucked"}${editing ? " editing" : ""}${tight ? ` tight tight${tight}` : ""}${fading ? " moodfade" : ""}`}
      style={vars}
      onMouseOver={overDock}
      onMouseLeave={() => clearTimeout(endPreview.current)}
      onMouseDown={(e) => {
        // In move mode any press drags; otherwise Alt+drag does. The Done cell is the way out.
        dragged.current = false;
        if (e.button !== 0 || (e.target as HTMLElement).closest(".done")) return;
        if (editing || e.altKey) {
          e.preventDefault();
          e.stopPropagation();
          dragged.current = true;
          invoke("drag_begin");
        }
      }}
      onClickCapture={(e) => {
        if (dragged.current || (editing && !(e.target as HTMLElement).closest(".done"))) {
          dragged.current = false;
          e.preventDefault();
          e.stopPropagation();
        }
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        invoke("open_settings");
      }}
    >
      {glass && (
        <Glass
          style={settings.style}
          radius={shape === "pill" ? settings.radius : 0}
          tint={settings.glass_tint}
          blur={settings.glass_blur}
          live={settings.glass_live}
          onReady={onReady}
          onLuma={onLuma}
        />
      )}
      <div className="row" ref={row}>
        {(() => {
          const keys = slotKeys(settings.widgets);
          return settings.widgets.map((slot, i) => {
            const def = byId(slot.id);
            if (!slot.enabled || !def?.component) return null;
            const W = def.component;
            return <W key={keys[i]} settings={settings} options={{ ...def.defaults, ...slot.options }} />;
          });
        })()}
        {editing && (
          <button className="cell done" title="Finish moving the dock" onClick={() => invoke("edit_mode", { on: false })}>
            Done
          </button>
        )}
      </div>
    </main>
    {settings.handle && <i className={`handle edge-${settings.edge}${shown ? "" : " on"}`} aria-hidden="true" />}
    </>
  );
}
