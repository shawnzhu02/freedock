import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { useToday } from "../widgets/Todos";
import { invalidate } from "../lib/agenda";
import { dueLabel, ymd, type TodayItem } from "../lib/today";

/** Local to-dos ("local:<id>") can be ticked and removed here; a connected MCP server's items are
 *  shown read-only (they're ticked off wherever that server lives). */
function Section({ title, items, now, changed }: { title: string; items: TodayItem[]; now: Date; changed: () => void }) {
  if (!items.length) return null;
  return (
    <section>
      <h4>{title}</h4>
      {items.map((t) => {
        const local = t.id.startsWith("local:");
        return (
          <div className="tdrow" key={t.id} title={t.meta ?? ""}>
            {local ? (
              <button
                className={`tdbox${t.due && dueLabel(t.due, now) === "Overdue" ? " late" : ""}`}
                title="Done"
                aria-label={`Mark ${t.title} done`}
                onClick={() => invoke("todo_toggle", { id: t.id }).then(changed)}
              />
            ) : (
              <i className={`tdbox${t.due && dueLabel(t.due, now) === "Overdue" ? " late" : ""}`} />
            )}
            <div className="fmeta">
              <b>{t.title}</b>
              {(t.course || (t.area && !local) || t.due) && (
                <small>{[t.course ?? (local ? "" : t.area), t.due ? dueLabel(t.due, now) : ""].filter(Boolean).join(" · ")}</small>
              )}
            </div>
            {local && (
              <button className="x" title="Delete" aria-label={`Delete ${t.title}`} onClick={() => invoke("todo_delete", { id: t.id }).then(changed)}>
                ×
              </button>
            )}
          </div>
        );
      })}
    </section>
  );
}

/** Today's to-dos and what's coming up: tick them off, delete them, or add one. */
export default function TodosView({ opened }: { opened: number }) {
  const [bump, setBump] = useState(0);
  const { v, err } = useToday(opened * 1000 + bump, true);
  const [text, setText] = useState("");
  const [note, setNote] = useState("");
  const box = useRef<HTMLInputElement>(null);
  const busy = useRef(false);
  const now = new Date();
  useEffect(() => box.current?.focus(), [opened]);
  const changed = () => {
    invalidate();
    setBump((b) => b + 1);
  };
  const add = () => {
    const name = text.trim();
    // A held or double Enter adds it once.
    if (!name || busy.current) return;
    busy.current = true;
    invoke("agenda_add_todo", { name, forDate: ymd(now) })
      .then(() => {
        setText("");
        setNote("");
        changed();
      })
      .catch((e) => setNote(String(e)))
      .finally(() => (busy.current = false));
  };
  const empty = v && !v.today.length && !v.soon.length && !v.school.length;
  return (
    <div className="view tdv">
      <header>
        <b>To-dos</b>
        <span className="sp" />
      </header>
      <input
        ref={box}
        className="psearch"
        placeholder="Add a to-do for today (Enter)"
        value={text}
        maxLength={300}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && add()}
      />
      {note && <p className="note">{note}</p>}
      {!v && !err && <p className="empty center">Loading…</p>}
      {err && <p className="note">{err}</p>}
      {empty && <p className="empty center">Nothing on the list.</p>}
      {v && !empty && (
        <div className="ghbody">
          <Section title="Today" items={v.today} now={now} changed={changed} />
          <Section title="School" items={v.school} now={now} changed={changed} />
          <Section title="Coming up" items={v.soon} now={now} changed={changed} />
        </div>
      )}
    </div>
  );
}
