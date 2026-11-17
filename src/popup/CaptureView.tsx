import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ymd } from "../lib/today";
import { invalidate } from "../lib/agenda";

const WHEN: { key: string; label: string; date: () => string }[] = [
  { key: "today", label: "Today", date: () => "" },
  {
    key: "tomorrow",
    label: "Tomorrow",
    date: () => {
      const d = new Date();
      d.setDate(d.getDate() + 1);
      return ymd(d);
    },
  },
  {
    key: "monday",
    label: "Next week",
    date: () => {
      const d = new Date();
      d.setDate(d.getDate() + (((8 - d.getDay()) % 7) || 7));
      return ymd(d);
    },
  },
];

/** Quick capture: a to-do straight onto the Today list. Enter adds and stays open for the next one. */
export default function CaptureView({ opened }: { opened: number }) {
  const [text, setText] = useState("");
  const [when, setWhen] = useState("today");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [added, setAdded] = useState<string[]>([]);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    setErr("");
    setWhen("today");
    setTimeout(() => input.current?.focus(), 60);
  }, [opened]);
  const add = () => {
    const name = text.trim();
    if (!name || busy) return;
    setBusy(true);
    setErr("");
    const for_date = WHEN.find((w) => w.key === when)!.date();
    invoke("agenda_add_todo", { name, forDate: for_date })
      .then(() => {
        invalidate();
        setAdded((a) => [name, ...a].slice(0, 5));
        setText("");
      })
      .catch((e) => setErr(String(e)))
      .finally(() => setBusy(false));
  };
  return (
    <div className="view capv">
      {
        <>
          <input
            ref={input}
            className="psearch"
            placeholder="Add a to-do…"
            value={text}
            maxLength={200}
            disabled={busy}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                add();
              }
            }}
          />
          <div className="cwhen">
            {WHEN.map((w) => (
              <button key={w.key} className={when === w.key ? "on" : ""} onClick={() => setWhen(w.key)}>
                {w.label}
              </button>
            ))}
            <button className="cadd" disabled={!text.trim() || busy} onClick={add}>
              {busy ? "Adding…" : "Add"}
            </button>
          </div>
          {err && <p className="note">{err}</p>}
          {added.length > 0 && (
            <ul className="cadded">
              {added.map((a, i) => (
                <li key={i}>✓ {a}</li>
              ))}
            </ul>
          )}
        </>
      }
    </div>
  );
}
