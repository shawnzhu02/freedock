import { useCallback, useState, type ReactNode } from "react";
import Glass, { GLASS_STYLES } from "./Glass";
import type { Settings } from "../settings";

/** A card (the settings panel or the agents' popup) in the dock's liquid glass: the same live screen,
 *  tint and shader, blurred more and veiled darker so text stays readable. Without a glass style, or
 *  while the glass can't draw, the card keeps its solid background. */
export default function GlassCard({ className, settings, window, children, enter }: {
  className: string;
  settings: Settings;
  window: "settings" | "ask";
  children: ReactNode;
  /** The open animation's state: "pre" (not shown yet) or "in" (animating in, or shown). */
  enter?: "pre" | "in";
}) {
  const [ok, setOk] = useState(false);
  const onReady = useCallback((r: boolean) => setOk(r), []);
  const glass = GLASS_STYLES.includes(settings.style);
  return (
    <div
      className={`${className}${glass && ok ? " has-glass" : ""}`}
      data-enter={enter}
      data-motion={settings.popup_motion ?? "warp"}
      data-edge={settings.edge}
    >
      {glass && (
        <Glass
          window={window}
          style={settings.style}
          radius={16}
          tint={settings.glass_tint}
          blur={settings.glass_blur * 1.8}
          live={settings.glass_live}
          onReady={onReady}
          autoShade
        />
      )}
      {children}
    </div>
  );
}
