import { useEffect, useState } from "react";
import type { WidgetProps } from "./registry";
import { ZONES, dayLabel, diffLabel, validZone } from "../lib/zones";

/** The time in another city. Add it more than once for several cities. */
export default function WorldClock({ settings, options }: WidgetProps) {
  const zone = String(options.zone ?? "Europe/London");
  const ok = validZone(zone);
  const label = String(options.label || ZONES.find(([z]) => z === zone)?.[1] || zone.split("/").pop()!.replace(/_/g, " "));
  const vertical = settings.edge === "left" || settings.edge === "right";
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    // Tick on the minute.
    let t = window.setTimeout(function tick() {
      setNow(new Date());
      t = window.setTimeout(tick, 60_000 - (Date.now() % 60_000) + 50);
    }, 60_000 - (Date.now() % 60_000) + 50);
    return () => clearTimeout(t);
  }, []);
  if (!ok) return <div className="cell stat" title={`Unknown time zone: ${zone}`}>?</div>;
  const time = new Intl.DateTimeFormat(undefined, { timeZone: zone, hour: "numeric", minute: "2-digit", hour12: settings.hour12 ?? undefined }).format(now);
  const day = dayLabel(zone, now);
  return (
    <div className="cell stat wclock" title={`${label}: ${time}${day ? ` ${day.toLowerCase()}` : ""} (${diffLabel(zone, now)})`}>
      <span className="stext">
        <small>{vertical ? label.slice(0, 3) : label}</small>
        <b>
          {time}
          {day && !vertical && <i>{day === "Tomorrow" ? " +1" : " -1"}</i>}
        </b>
      </span>
    </div>
  );
}
