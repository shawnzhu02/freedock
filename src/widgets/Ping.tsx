import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { quality, stats } from "../lib/ping";
import { pollActive } from "../lib/active";

const KEEP = 24;

/** Pings `target` every `everySec` seconds while the dock is visible; keeps the last 24 samples. */
function usePing(target: string, everySec: number) {
  const [samples, setSamples] = useState<(number | null)[]>([]);
  const [err, setErr] = useState("");
  useEffect(() => {
    setSamples([]);
    let alive = true;
    const go = () => {
      invoke<number | null>("ping_once", { target })
        .then((ms) => alive && (setErr(""), setSamples((s) => [...s, ms].slice(-KEEP))))
        .catch((e) => alive && setErr(String(e)));
    };
    go();
    const stop = pollActive(everySec * 1000, go);
    return () => {
      alive = false;
      stop();
    };
  }, [target, everySec]);
  return { samples, err };
}

/** Ping: latency to a server with a little bar per sample; green, amber or red by quality. */
export default function PingCell({ settings, options }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  const target = String(options.target ?? "1.1.1.1");
  const { samples, err } = usePing(target, Number(options.every ?? 5));
  const s = stats(samples);
  const q = quality(s);
  const max = Math.max(60, ...samples.map((x) => x ?? 0));
  const ms = (v: number | null) => (v === null ? "—" : `${Math.round(v)} ms`);
  const tip = err
    ? `Ping: ${err}`
    : `Ping ${target}: ${ms(s.last)} now, ${ms(s.avg)} average, ${s.jitter === null ? "—" : `${Math.round(s.jitter)} ms`} jitter, ${Math.round(s.loss * 100)}% lost (last ${s.n})`;
  return (
    <button className={`cell stat ctl pingc ${q}`} title={tip} onClick={(e) => openAt("quick", e, vertical)}>
      <span className="pbars" aria-hidden="true">
        {Array.from({ length: vertical ? 8 : 12 }, (_, i) => {
          const v = samples[samples.length - (vertical ? 8 : 12) + i];
          return <i key={i} className={v === null ? "lost" : v === undefined ? "none" : ""} style={{ height: v ? `${Math.max(12, Math.min(100, (v / max) * 100))}%` : undefined }} />;
        })}
      </span>
      {!vertical && <b className="cval">{err ? "—" : s.last === null ? (s.n ? "lost" : "…") : Math.round(s.last)}</b>}
    </button>
  );
}
