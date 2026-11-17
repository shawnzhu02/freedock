import { useEffect, useMemo, useState } from "react";
import { useKept } from "../lib/kept";
import { useSections } from "./Sections";
import { invoke } from "@tauri-apps/api/core";
import { todayView } from "../lib/agenda";
import { byDue, dueLabel, joinLink, nextMeeting, ymd, type TodayItem, type TodayView } from "../lib/today";

/** The weeks of a month as rows of 7 dates (null for padding), weeks starting on `firstDay` (0 = Sun). */
export function monthGrid(year: number, month: number, firstDay = 0): (Date | null)[][] {
  const first = new Date(year, month, 1);
  const days = new Date(year, month + 1, 0).getDate();
  const lead = (first.getDay() - firstDay + 7) % 7;
  const cells: (Date | null)[] = Array(lead).fill(null);
  for (let d = 1; d <= days; d++) cells.push(new Date(year, month, d));
  while (cells.length % 7) cells.push(null);
  const rows: (Date | null)[][] = [];
  for (let i = 0; i < cells.length; i += 7) rows.push(cells.slice(i, i + 7));
  return rows;
}

const label = (i: TodayItem) => (i.course ? `${i.course} · ${i.title}` : i.title);

/** A month calendar. Days with schoolwork or to-dos due get a dot; clicking a day lists them. Under
 *  the month: today's events from your calendars (with Join) and what's due. */
export default function Calendar({ opened }: { opened: number }) {
  const [view, setView] = useState(() => new Date());
  const [data, setData] = useKept<TodayView | null>("calendar.data", null);
  const [err, setErr] = useState("");
  const [picked, setPicked] = useState<string | null>(null);
  const today = new Date();
  useEffect(() => {
    setView(new Date());
    setPicked(null);
    todayView(true)
      .then((v) => {
        setData(v);
        setErr(v.errors?.length ? `Couldn't load part of your agenda: ${v.errors.join("; ")}` : "");
      })
      .catch((e) => setErr(`Couldn't load your calendar: ${e}`));
  }, [opened]);
  const rows = useMemo(() => monthGrid(view.getFullYear(), view.getMonth()), [view]);
  const weekdays = useMemo(() => Array.from({ length: 7 }, (_, i) => new Date(2023, 0, 1 + i).toLocaleDateString(undefined, { weekday: "narrow" })), []);
  const due = useMemo(() => byDue([...(data?.school ?? []), ...(data?.today ?? []), ...(data?.soon ?? [])]), [data]);
  const shift = (n: number) => setView((v) => new Date(v.getFullYear(), v.getMonth() + n, 1));
  const todayKey = ymd(today);
  const next = data ? nextMeeting(data.events, today) : null;

  // What's due: the picked day, or the next week (overdue first).
  const week = ymd(new Date(today.getFullYear(), today.getMonth(), today.getDate() + 7));
  const dueList: [string, TodayItem[]][] = picked
    ? [[picked, due.get(picked) ?? []]]
    : [...due.entries()].filter(([d]) => d <= week).sort(([a], [b]) => a.localeCompare(b));

  const { body, button } = useSections(
    "calendar",
    [
      {
        id: "month",
        name: "Month",
        hint: "click a day to see what's due",
        node: (
            <div className="grid">
              {weekdays.map((d, i) => (
                <i key={i}>{d}</i>
              ))}
              {rows.flat().map((d, i) => {
                if (!d) return <span key={i} />;
                const k = ymd(d);
                const n = due.get(k)?.length ?? 0;
                return (
                  <button
                    key={i}
                    className={`${k === todayKey ? "today" : ""}${k === picked ? " picked" : ""}`}
                    title={n ? due.get(k)!.map(label).join("\n") : ""}
                    onClick={() => setPicked(picked === k ? null : k)}
                  >
                    {d.getDate()}
                    {n > 0 && <em className={k < todayKey ? "late" : ""} />}
                  </button>
                );
              })}
            </div>
        ),
      },
      {
        id: "today",
        name: "Today's events",
        hint: "from your calendar links, with Join for calls",
        node: (
          <>
            {!picked && (
              <>
                <h4>Today</h4>
                {data === null && !err ? (
                  <span className="dots" />
                ) : data?.events.length ? (
                  data.events.map((e, i) => {
                    const link = joinLink(e.place);
                    const isNext = next?.event === e;
                    return (
                      <div key={i} className={`ev${isNext ? " next" : ""}`}>
                        <time>{e.time}</time>
                        <span>
                          {e.title}
                          {e.place && <small>{e.place}</small>}
                        </span>
                        {link && (
                          <button className="joinbtn" onClick={() => invoke("open_link", { url: link }).then(() => invoke("popup_close"))}>
                            Join
                          </button>
                        )}
                      </div>
                    );
                  })
                ) : (
                  <p className="empty">Nothing scheduled. (Connect a calendar with Calendars… below.)</p>
                )}
              </>
            )}
          </>
        ),
      },
      {
        id: "due",
        name: "Due this week",
        hint: "schoolwork and to-dos with dates",
        node: (
          <>
            {(picked || dueList.length > 0) && (
              <h4>{picked ? `Due ${new Date(picked + "T12:00").toLocaleDateString(undefined, { weekday: "long", month: "short", day: "numeric" })}` : "Due this week"}</h4>
            )}
            {dueList.flatMap(([d, items]) =>
              items.map((i) => (
                <div key={i.id} className={`ev due${d < todayKey ? " late" : ""}`}>
                  <time>{dueLabel(d, today)}</time>
                  <span>{label(i)}</span>
                </div>
              )),
            )}
            {picked && !due.get(picked)?.length && <p className="empty">Nothing due.</p>}
          </>
        ),
      },
    ],
    opened,
  );
  return (
    <div className="view cal">
      <div className="calhead">
        <div>
          <small>{today.toLocaleDateString(undefined, { weekday: "long" })}</small>
          <b>{today.toLocaleDateString(undefined, { month: "long", day: "numeric" })}</b>
        </div>
        <span className="sp" />
        <button className="x" title="Previous month" onClick={() => shift(-1)}>
          ‹
        </button>
        <button className="month" title="Back to this month" onClick={() => setView(new Date())}>
          {view.toLocaleDateString(undefined, { month: "short", year: "numeric" })}
        </button>
        <button className="x" title="Next month" onClick={() => shift(1)}>
          ›
        </button>
      </div>
      <div className="scroll agenda calbody">
        {err && <p className="note">{err}</p>}
        {body}
      </div>
      <footer className="qfoot">
        {button}
        <button className="link" title="Connect your calendars (iCal links) or an MCP server" onClick={() => invoke("open_settings", { at: "General#calendar" }).then(() => invoke("popup_close"))}>
          Calendars…
        </button>
        <button className="link" onClick={() => invoke("open_windows", { page: "notifications" }).then(() => invoke("popup_close"))}>
          Notifications
        </button>
      </footer>
    </div>
  );
}
