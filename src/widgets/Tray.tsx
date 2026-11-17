import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";

export type TrayIcon = { id: string; tooltip: string; icon: string | null };
type Act = "left" | "right" | "double" | "middle";

/** Background apps (the tray the hidden taskbar used to show): a chevron that opens their icons in a
 *  flyout above the dock, like Windows' own overflow, plus the first few icons right on the dock if
 *  the widget's "On the dock" option asks for them. */
export default function Tray({ settings, options }: WidgetProps) {
  const inline = Number(options.inline) || 0;
  const [count, setCount] = useState(0);
  const [icons, setIcons] = useState<TrayIcon[]>([]);
  const load = useCallback(() => {
    invoke<number>("tray_count").then(setCount).catch(() => {});
    // The icons themselves (images) are only fetched when some are shown on the dock.
    if (inline > 0) invoke<TrayIcon[]>("tray_list").then(setIcons).catch(() => {});
  }, [inline]);
  useEffect(() => {
    load();
    const off = listen("tray-changed", load);
    return () => {
      off.then((f) => f());
    };
  }, [load]);
  const clickTimer = useRef(0);
  const act = (id: string, action: Act) => invoke("tray_action", { id, action });
  const vertical = settings.edge === "left" || settings.edge === "right";
  const up = settings.edge === "top" ? "M3 6l5 5 5-5" : settings.edge === "left" ? "M6 3l5 5-5 5" : settings.edge === "right" ? "M10 3L5 8l5 5" : "M3 10l5-5 5 5";
  return (
    <div className="cell group tray">
      {icons.slice(0, inline).map((i) => (
        <button
          key={i.id}
          className="trayi"
          title={i.tooltip}
          aria-label={i.tooltip || "Background app"}
          onClick={() => {
            clearTimeout(clickTimer.current);
            clickTimer.current = window.setTimeout(() => act(i.id, "left"), 220);
          }}
          onDoubleClick={() => {
            clearTimeout(clickTimer.current);
            act(i.id, "double");
          }}
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            act(i.id, "right");
          }}
          onAuxClick={(e) => e.button === 1 && act(i.id, "middle")}
        >
          {i.icon ? <img src={i.icon} alt="" draggable={false} /> : <span>{i.tooltip.slice(0, 1)}</span>}
        </button>
      ))}
      <button className="chev" title={`Background apps (${count})`} aria-label={`Background apps, ${count}`} onClick={(e) => openAt("tray", e, vertical)}>
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
          <path d={up} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
    </div>
  );
}
