import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { appName, dur, weekday, type StDay } from "../lib/screentime";

/** Today's programs by time, and the last 7 days as bars. Counted only while the user is active. */
export default function ScreenTimeView({ opened }: { opened: number }) {
  const [days, setDays] = useState<StDay[] | null>(null);
  useEffect(() => {
    invoke<StDay[]>("screentime_get", { days: 7, icons: true }).then(setDays).catch(() => setDays([]));
  }, [opened]);
  if (!days)
    return (
      <div className="view stv">
        <p className="empty center">Loading…</p>
      </div>
    );
  const today = days[0];
  const most = Math.max(1, ...days.map((d) => d.total));
  const top = today?.apps[0]?.[0].secs ?? 1;
  const week = [...days].reverse();
  return (
    <div className="view stv">
      <header>
        <b>Screen time</b>
        <span className="sp" />
        <small className="sttotal">{today ? `${dur(today.total)} today` : ""}</small>
      </header>
      {!today ? (
        <p className="empty center">Counting starts now. Only time you're active at the PC counts.</p>
      ) : (
        <>
          <div className="stweek">
            {week.map((d) => (
              <div key={d.date} className={`stday${d === today ? " now" : ""}`} title={`${d.date}: ${dur(d.total)}`}>
                <i style={{ height: `${Math.max(4, (d.total / most) * 100)}%` }} />
                <small>{weekday(d.date)}</small>
              </div>
            ))}
          </div>
          <div className="stapps">
            {today.apps.map(([a, icon]) => (
              <div className="stapp" key={a.exe} title={a.exe}>
                <span className="sticon">{icon ? <img src={icon} alt="" /> : null}</span>
                <span className="stname">{appName(a.name)}</span>
                <span className="stbar">
                  <i style={{ width: `${(a.secs / top) * 100}%` }} />
                </span>
                <small>{dur(a.secs)}</small>
              </div>
            ))}
          </div>
          <p className="stnote">Counted every 5 s while you're active; kept on this PC for 14 days.</p>
        </>
      )}
    </div>
  );
}
