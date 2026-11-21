import { useEffect, useState } from "react";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { useNow } from "./useNow";
import { todayView } from "../lib/agenda";
import { timeline, type TodayEvent } from "../lib/today";
import { pollActive } from "../lib/active";
import { use12h } from "../lib/weather";

const label = (h: number, hour12: boolean) => (hour12 ? `${h % 12 || 12}${h < 12 ? "a" : "p"}` : String(h).padStart(2, "0"));

/** Day timeline: the next few hours as a strip, today's meetings (from your calendars) as blocks, and now. */
export default function Timeline({ settings, options }: WidgetProps) {
  const now = useNow();
  const [events, setEvents] = useState<TodayEvent[]>([]);
  useEffect(() => {
    const get = () => todayView().then((v) => setEvents(v.events)).catch(() => {});
    get();
    const stop = pollActive(5 * 60_000, () => get());
    return () => stop();
  }, []);
  const vertical = settings.edge === "left" || settings.edge === "right";
  if (vertical) return null;
  const after = Number(options.hours ?? 5);
  const t = timeline(events, now, 1, after, Number(options.length ?? 60));
  const hour12 = use12h(settings.hour12);
  const next = t.blocks.find((b) => b.left >= t.nowAt || b.now);
  return (
    <button
      className="cell tline"
      style={{ width: `${Number(options.width ?? 220)}px` }}
      title={t.blocks.length ? t.blocks.map((b) => `${b.time} ${b.title}`).join("\n") : `Nothing on in the next ${after} hours`}
      onClick={(e) => openAt("calendar", e, false)}
    >
      <div className="tl">
        {t.ticks.map((k) => (
          <span key={k.left} className="tick" style={{ left: `${k.left}%` }}>
            {(t.ticks.length <= 8 || k.hour % 2 === 0) && <small>{label(k.hour, hour12)}</small>}
          </span>
        ))}
        {t.blocks.map((b, i) => (
          <i key={i} className={`blk${b.now ? " now" : ""}${b === next ? " next" : ""}`} style={{ left: `${b.left}%`, width: `${b.width}%` }}>
            {b.width > 18 && <em>{b.title}</em>}
          </i>
        ))}
        <span className="nowmark" style={{ left: `${t.nowAt}%` }} />
      </div>
    </button>
  );
}
