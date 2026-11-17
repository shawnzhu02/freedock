import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { SpeakerGlyph, useAudio } from "./Controls";
import { isActive } from "../lib/active";

type Power = { percent: number | null; charging: boolean; plugged: boolean };

/** Wi-Fi, sound and battery at a glance; a click opens our quick-settings flyout. */
export default function System({ settings }: WidgetProps) {
  const [p, setP] = useState<Power | null>(null);
  useEffect(() => {
    const get = () => invoke<Power>("power").then(setP).catch(() => {});
    get();
    const t = window.setInterval(() => isActive() && get(), 60_000);
    return () => clearInterval(t);
  }, []);
  // Polled less often than the Volume cell: this is only the icon.
  const { a: audio } = useAudio(5000);
  const [link, setLink] = useState("wifi");
  useEffect(() => {
    const get = () => invoke<string>("net_link").then(setLink).catch(() => {});
    get();
    const t = window.setInterval(() => isActive() && get(), 15_000);
    return () => clearInterval(t);
  }, []);
  const pct = p?.percent ?? null;
  return (
    <button className="cell system" title={link === "ethernet" ? "Quick settings (on Ethernet)" : link === "none" ? "Quick settings (offline)" : "Quick settings"} onClick={(e) => openAt("quick", e, settings.edge === "left" || settings.edge === "right")}>
      {link === "ethernet" ? (
        <svg viewBox="0 0 20 16" width="17" height="14" aria-hidden="true">
          <rect x="3" y="2" width="14" height="9" rx="1.6" fill="none" stroke="currentColor" strokeWidth="1.6" />
          <path d="M7 11v2.5h6V11M7.5 5.5v2M10 5.5v2M12.5 5.5v2" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      ) : (
        <svg viewBox="0 0 20 16" width="17" height="14" aria-hidden="true" style={link === "none" ? { opacity: 0.4 } : undefined}>
          <path d="M1.5 5.2a12 12 0 0 1 17 0M4.3 8.2a8 8 0 0 1 11.4 0M7.1 11.1a4 4 0 0 1 5.8 0" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
          <circle cx="10" cy="14" r="1.4" fill="currentColor" />
        </svg>
      )}
      <SpeakerGlyph volume={audio?.volume ?? 100} muted={audio?.muted ?? false} />
      {pct !== null && (
        <span className="batt" title={`${pct}%${p?.charging ? ", charging" : p?.plugged ? ", plugged in" : ""}`}>
          <i style={{ width: `${pct}%` }} className={pct <= 20 && !p?.plugged ? "low" : ""} />
          {p?.charging && <b>⚡</b>}
        </span>
      )}
    </button>
  );
}
