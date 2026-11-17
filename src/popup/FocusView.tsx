import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { clock } from "../lib/media";
import { FocusRing, secsLeft, useFocus } from "../widgets/Focus";

const PRESETS: { kind: "focus" | "break"; min: number; name: string }[] = [
  { kind: "focus", min: 25, name: "Focus 25" },
  { kind: "focus", min: 50, name: "Focus 50" },
  { kind: "break", min: 5, name: "Break 5" },
  { kind: "break", min: 15, name: "Break 15" },
];

/** Start a focus session, a break or a timer; pause, resume or stop the running one. */
export default function FocusView({ opened }: { opened: number }) {
  const f = useFocus();
  const [label, setLabel] = useState("");
  const [mins, setMins] = useState("10");
  const [err, setErr] = useState("");
  useEffect(() => setErr(""), [opened]);
  const start = (kind: string, minutes: number, l = label) =>
    invoke("focus_start", { kind, minutes, label: l.trim() || null }).catch((e) => setErr(String(e)));
  if (!f) return <div className="view focusv" />;
  const running = !!f.kind;
  const justDone = !running && f.done_at && Date.now() - f.done_at < 10 * 60_000;
  return (
    <div className="view focusv">
      <div className="fbig">
        <FocusRing f={f} size={132} stroke={2.2} />
        <div className="fcenter">
          {running ? (
            <>
              <b>{clock(secsLeft(f))}</b>
              <small>{f.label || (f.kind === "focus" ? "Focus" : f.kind === "break" ? "Break" : "Timer")}{f.paused_left ? " · paused" : ""}</small>
            </>
          ) : justDone ? (
            <>
              <b className="fdone">Done</b>
              <small>Time's up</small>
            </>
          ) : (
            <>
              <b>{f.today}</b>
              <small>{f.today === 1 ? "session today" : "sessions today"}</small>
            </>
          )}
        </div>
      </div>
      {running ? (
        <div className="fctl">
          <button onClick={() => invoke("focus_pause")}>{f.paused_left ? "Resume" : "Pause"}</button>
          <button onClick={() => invoke("focus_stop")}>Stop</button>
        </div>
      ) : (
        <>
          {justDone && f.today > 0 && <p className="fnote">{f.today} focus session{f.today === 1 ? "" : "s"} today.</p>}
          <input className="psearch" placeholder="What are you working on? (optional)" value={label} maxLength={40} onChange={(e) => setLabel(e.target.value)} />
          <div className="fpresets">
            {PRESETS.map((p) => (
              <button key={p.name} className={p.kind} onClick={() => start(p.kind, p.min)}>
                {p.name}
              </button>
            ))}
          </div>
          <div className="ftimer">
            <span>Timer</span>
            <input type="number" min={1} max={1440} value={mins} onChange={(e) => setMins(e.target.value)} />
            <span>min</span>
            <button onClick={() => start("timer", Number(mins))}>Start</button>
          </div>
        </>
      )}
      {err && <p className="note">{err}</p>}
    </div>
  );
}
