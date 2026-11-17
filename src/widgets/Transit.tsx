// MBTA arrivals: the cell shows the next train/bus each way at your stop; the flyout lists every
// line there with its next three times. Data: lib/mbta.ts (key-free MBTA v3 API).
import { useEffect, useState } from "react";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { getArrivals, lines, minsLabel, type Line } from "../lib/mbta";
import { isActive, onWake } from "../lib/active";

export function useArrivals(stop: string) {
  const [l, setL] = useState<Line[] | null>(null);
  const [err, setErr] = useState("");
  const [at, setAt] = useState(0);
  useEffect(() => {
    let t = 0;
    let alive = true;
    const tick = () => {
      if (!isActive()) {
        t = window.setTimeout(tick, 120_000);
        return;
      }
      getArrivals(stop)
        .then((a) => {
          if (!alive) return;
          setL(lines(a));
          setErr("");
          setAt(Date.now());
        })
        .catch((e) => alive && setErr(String(e?.message ?? e)));
      t = window.setTimeout(tick, isActive() ? 30_000 : 120_000);
    };
    tick();
    // Back from tucked or asleep: fetch now instead of waiting out the long wait.
    const off = onWake(() => {
      clearTimeout(t);
      tick();
    });
    return () => {
      alive = false;
      clearTimeout(t);
      off();
    };
  }, [stop]);
  return { lines: l, err, at };
}

/** "Government Center" -> "Gov Center"; short names stay. */
export const shortTo = (s: string) => s.replace("Government", "Gov").replace("Station", "Sta").replace("Boston College", "BC");

export default function Transit({ options, settings }: WidgetProps) {
  const stop = String(options.stop || "place-pktrm");
  const { lines: l, err } = useArrivals(stop);
  const vertical = settings.edge === "left" || settings.edge === "right";
  // The soonest line each way.
  const each = [0, 1].map((d) => l?.find((x) => x.direction === d)).filter(Boolean) as Line[];
  return (
    <button className="cell transit" title={err || "Next arrivals"} onClick={(e) => openAt("transit", e, vertical)}>
      {!l ? (
        <span className="dim">{err ? "MBTA ?" : "…"}</span>
      ) : each.length === 0 ? (
        <span className="dim">No trips</span>
      ) : (
        each.slice(0, vertical ? 1 : 2).map((x) => (
          <span className="trow" key={x.direction}>
            <i style={{ background: x.color }}>{x.route}</i>
            {!vertical && <small>{shortTo(x.to)}</small>}
            <b>{x.mins[0] <= 0 ? "Now" : `${x.mins[0]}m`}</b>
          </span>
        ))
      )}
    </button>
  );
}

export function TransitView({ stop, name, opened }: { stop: string; name: string; opened: number }) {
  const { lines: l, err, at } = useArrivals(stop);
  void opened;
  return (
    <div className="view transitv">
      <header>
        <b>{name}</b>
        <span className="sp" />
        {at > 0 && <small className="dim">updated {new Date(at).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}</small>}
      </header>
      <div className="ubody">
        {err && <p className="note">{err}</p>}
        {!l && !err && <span className="dots" />}
        {l?.length === 0 && <p className="empty">No upcoming trips at this stop.</p>}
        {l?.map((x) => (
          <div className="tline" key={`${x.route}-${x.to}`}>
            <i style={{ background: x.color }}>{x.route}</i>
            <span>
              <b>{x.to}</b>
            </span>
            <span className="tmins">
              {x.mins.map((m, i) => (
                <em key={i} className={i === 0 ? "first" : ""}>
                  {minsLabel(m)}
                </em>
              ))}
            </span>
          </div>
        ))}
      </div>
      <footer className="qfoot">
        <small className="dim">MBTA predictions</small>
      </footer>
    </div>
  );
}
