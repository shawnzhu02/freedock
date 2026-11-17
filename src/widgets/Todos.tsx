import { useEffect, useState } from "react";
import { useKept } from "../lib/kept";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { todayView } from "../lib/agenda";
import type { TodayView } from "../lib/today";
import { isActive } from "../lib/active";

/** The agenda's today view for a cell, refreshed every 5 minutes while visible. */
export function useToday(key = 0, fresh = false) {
  const [v, setV] = useKept<TodayView | null>("today.view", null);
  const [err, setErr] = useState("");
  useEffect(() => {
    const get = (f: boolean) =>
      todayView(f)
        .then((x) => (setV(x), setErr("")))
        .catch((e) => setErr(String(e?.message ?? e)));
    get(fresh);
    const t = window.setInterval(() => isActive() && get(false), 5 * 60_000);
    return () => clearInterval(t);
  }, [key, fresh]);
  return { v, err };
}

/** A checklist glyph (our own drawing). */
function ListGlyph() {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" className="pglyph">
      <path d="M4 6.5l1.6 1.6L8.5 5M4 12.5l1.6 1.6 2.9-3.1M4 18.5l1.6 1.6 2.9-3.1M11.5 7h8.5M11.5 13h8.5M11.5 19h8.5" />
    </svg>
  );
}

/** To-dos: how many things are on today's list (plus anything due today); click for the list. */
export default function TodosCell({ settings }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  const { v, err } = useToday();
  const today = new Date();
  const ymd = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;
  const n = v ? v.today.length + v.school.filter((s) => s.due && s.due <= ymd).length : 0;
  return (
    <button
      className="cell stat ctl todos"
      title={err ? `To-dos: ${err}` : v ? `${n} on today's list, ${v.soon.length} coming up` : "To-dos"}
      onClick={(e) => openAt("todos", e, vertical)}
    >
      <ListGlyph />
      {!vertical && <b className="cval">{v ? n : "…"}</b>}
    </button>
  );
}
