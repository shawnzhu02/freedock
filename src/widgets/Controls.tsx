import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { isActive } from "../lib/active";

export type Audio = { volume: number; muted: boolean; device: string };
type Power = { percent: number | null; charging: boolean; plugged: boolean; secs_left: number | null };

const isVertical = (p: WidgetProps) => p.settings.edge === "left" || p.settings.edge === "right";

/** Speaker with 0-2 waves by level, or a cross when muted. */
export function SpeakerGlyph({ volume, muted, size = 16 }: { volume: number; muted: boolean; size?: number }) {
  const waves = muted ? 0 : volume === 0 ? 0 : volume < 50 ? 1 : 2;
  return (
    <svg viewBox="0 0 18 16" width={size} height={(size * 14) / 16} aria-hidden="true">
      <path d="M2 6h3l4-3.5v11L5 10H2z" fill="currentColor" />
      {muted || volume === 0 ? (
        <path d="M12.2 5.6l4.6 4.8M16.8 5.6l-4.6 4.8" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      ) : (
        <>
          <path d="M12 5.2a4 4 0 0 1 0 5.6" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
          {waves > 1 && <path d="M14.3 3a7 7 0 0 1 0 10" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />}
        </>
      )}
    </svg>
  );
}

export function MicGlyph({ muted, size = 16 }: { muted: boolean; size?: number }) {
  return (
    <svg viewBox="0 0 16 16" width={size} height={size} aria-hidden="true" className="micg">
      <rect x="5.5" y="1.5" width="5" height="8.5" rx="2.5" fill={muted ? "none" : "currentColor"} stroke="currentColor" strokeWidth="1.4" />
      <path d="M3 8a5 5 0 0 0 10 0M8 13v2" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
      {muted && <path d="M2.5 2l11 12" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />}
    </svg>
  );
}

/** Speaker volume (polled every few seconds). */
export function useAudio(every = 3000) {
  const [a, setA] = useState<Audio | null>(null);
  const load = () => invoke<Audio | null>("quick_audio").then(setA).catch(() => {});
  useEffect(() => {
    load();
    const t = window.setInterval(() => isActive() && load(), every);
    return () => clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [every]);
  return { a, setA, load };
}

/** Volume: the level at a glance; the wheel changes it, a click mutes (or opens quick settings). */
export function VolumeCell(p: WidgetProps) {
  const { a, setA, load } = useAudio();
  const step = Number(p.options.step ?? 2);
  const pending = useRef(0);
  if (!a) return null;
  const wheel = (e: React.WheelEvent) => {
    const next = Math.max(0, Math.min(100, a.volume + (e.deltaY < 0 ? step : -step)));
    setA({ ...a, volume: next, muted: next === 0 ? a.muted : false });
    clearTimeout(pending.current);
    pending.current = window.setTimeout(() => invoke("quick_set_audio", { volume: next, muted: next > 0 ? false : null }).finally(load), 60);
  };
  return (
    <button
      className={`cell stat ctl${a.muted ? " off" : ""}`}
      title={`${a.device}: ${a.muted ? "muted" : `${a.volume}%`}. Scroll to change${p.options.click === "mute" ? ", click to mute" : ""}.`}
      onWheel={wheel}
      onClick={(e) => {
        if (p.options.click === "mute") {
          setA({ ...a, muted: !a.muted });
          invoke("quick_set_audio", { volume: null, muted: !a.muted }).finally(load);
        } else openAt("quick", e, isVertical(p));
      }}
    >
      <SpeakerGlyph volume={a.volume} muted={a.muted} />
      {!isVertical(p) && <b className="cval">{a.muted ? "Muted" : `${a.volume}%`}</b>}
    </button>
  );
}

/** Mic: one click mutes or unmutes every default microphone; red while muted is off, so a live mic is obvious. */
export function MicCell(p: WidgetProps) {
  const [m, setM] = useState<Audio | null>(null);
  const load = () => invoke<Audio | null>("quick_mic").then(setM).catch(() => {});
  useEffect(() => {
    load();
    const t = window.setInterval(() => isActive() && load(), 3000);
    return () => clearInterval(t);
  }, []);
  if (!m) return null;
  return (
    <button
      className={`cell stat ctl mic${m.muted ? " off" : " live"}`}
      title={`${m.device}: ${m.muted ? "muted" : "on"}. Click to ${m.muted ? "unmute" : "mute"}.`}
      onClick={() => {
        setM({ ...m, muted: !m.muted });
        invoke("quick_set_mic", { muted: !m.muted }).finally(load);
      }}
    >
      <MicGlyph muted={m.muted} />
      {!isVertical(p) && p.options.label !== false && <b className="cval">{m.muted ? "Mic off" : "Mic on"}</b>}
    </button>
  );
}

/** Brightness (laptop panel): the wheel changes it; hidden where Windows can't set it. */
export function BrightnessCell(p: WidgetProps) {
  const [b, setB] = useState<number | null | undefined>(undefined);
  const pending = useRef(0);
  useEffect(() => {
    invoke<number | null>("quick_brightness").then(setB).catch(() => setB(null));
  }, []);
  if (b === undefined || b === null) return null;
  const wheel = (e: React.WheelEvent) => {
    const next = Math.max(0, Math.min(100, b + (e.deltaY < 0 ? 5 : -5)));
    setB(next);
    clearTimeout(pending.current);
    // Setting it goes through WMI and takes a moment: send only where the wheel stops.
    pending.current = window.setTimeout(() => invoke("quick_set_brightness", { level: next }), 250);
  };
  return (
    <button className="cell stat ctl" title={`Brightness ${b}%. Scroll to change.`} onWheel={wheel} onClick={(e) => openAt("quick", e, isVertical(p))}>
      <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
        <circle cx="8" cy="8" r="3" fill="currentColor" />
        {[0, 45, 90, 135, 180, 225, 270, 315].map((r) => (
          <line key={r} x1="8" y1="1.4" x2="8" y2="3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" transform={`rotate(${r} 8 8)`} />
        ))}
      </svg>
      {!isVertical(p) && <b className="cval">{b}%</b>}
    </button>
  );
}

const hm = (s: number) => {
  const h = Math.floor(s / 3600);
  const m = Math.round((s % 3600) / 60);
  return h ? `${h} h ${String(m).padStart(2, "0")}` : `${m} min`;
};

/** Battery percent, charging state and Windows' estimate of time left. */
export function BatteryCell(p: WidgetProps) {
  const [pw, setPw] = useState<Power | null>(null);
  useEffect(() => {
    const get = () => invoke<Power>("power").then(setPw).catch(() => {});
    get();
    const t = window.setInterval(() => isActive() && get(), 30_000);
    return () => clearInterval(t);
  }, []);
  if (!pw || pw.percent === null) return null;
  const pct = pw.percent;
  const low = pct <= 20 && !pw.plugged;
  const state = pw.charging ? "charging" : pw.plugged ? "plugged in" : pw.secs_left ? `${hm(pw.secs_left)} left` : "on battery";
  const style = String(p.options.style ?? "bar");
  return (
    <button className={`cell stat ctl battc${low ? " low" : ""}`} title={`Battery ${pct}%, ${state}`} onClick={(e) => openAt("quick", e, isVertical(p))}>
      {style === "ring" ? (
        <svg viewBox="0 0 24 24" width="22" height="22" className={`bring${low ? " low" : pw.charging ? " chg" : ""}`} aria-hidden="true">
          <circle cx="12" cy="12" r="9.5" className="track" />
          <circle cx="12" cy="12" r="9.5" className="fill" pathLength="100" strokeDasharray={`${pct} 100`} transform="rotate(-90 12 12)" />
          {pw.charging ? <path d="M12.8 6.5 8.8 12.6h2.8l-.8 4.9 4-6.2H12z" className="bolt" /> : <circle cx="12" cy="12" r="2.2" className="dot" />}
        </svg>
      ) : style === "dots" ? (
        <span className={`bdots${low ? " low" : pw.charging ? " chg" : ""}`} aria-hidden="true">
          {[1, 2, 3, 4, 5].map((i) => (
            <i key={i} className={pct >= i * 20 - 10 ? "on" : ""} />
          ))}
        </span>
      ) : (
        <span className="batt">
          <i style={{ width: `${pct}%` }} className={low ? "low" : ""} />
          {pw.charging && <b>⚡</b>}
        </span>
      )}
      {!isVertical(p) && (
        <span className="stext">
          <b>{pct}%</b>
          {p.options.time !== false && <small>{pw.charging ? "charging" : pw.plugged ? "plugged in" : pw.secs_left ? hm(pw.secs_left) : ""}</small>}
        </span>
      )}
    </button>
  );
}
