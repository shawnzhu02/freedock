// "Customize" for flyouts with many parts: drag sections into the user's order and switch any of them
// off. Saved per flyout in settings.layouts (see lib/layout.ts).
import { Fragment, useEffect, useState, type ReactNode } from "react";
import { getSettings, saveSettings } from "../settings";
import { resolve, serialize } from "../lib/layout";
import { useReorder } from "../lib/useReorder";

export type Section = { id: string; name: string; node: ReactNode; hint?: string; hiddenByDefault?: boolean };

/** Last saved layout per flyout, so a reopened flyout draws in the user's order straight away. */
const cache: Record<string, string[] | null> = {};

async function persist(flyout: string, list: string[] | null) {
  const s = await getSettings();
  const layouts = { ...(s.layouts ?? {}) };
  if (list) layouts[flyout] = list;
  else delete layouts[flyout];
  await saveSettings({ ...s, layouts }, true);
}

export function useSections(flyout: string, sections: Section[], opened?: unknown) {
  const [saved, setSaved] = useState<string[] | null | undefined>(cache[flyout]);
  const [editing, setEditing] = useState(false);
  const [err, setErr] = useState("");
  useEffect(() => {
    setEditing(false);
    getSettings()
      .then((s) => {
        const l = s.layouts?.[flyout] ?? null;
        cache[flyout] = l;
        setSaved(l);
      })
      .catch(() => {});
  }, [flyout, opened]);

  const all = sections.map((s) => s.id);
  const { order, hidden } = resolve(saved, all, sections.filter((s) => s.hiddenByDefault).map((s) => s.id));
  const byId = Object.fromEntries(sections.map((s) => [s.id, s]));
  const { dragging, item } = useReorder(
    `sections-${flyout}`,
    order,
    (o) => {
      cache[flyout] = serialize(o, hidden);
      setSaved(cache[flyout]);
    },
    (o) => save(o, hidden),
  );
  const save = (o: string[], h: Set<string>) => {
    const list = serialize(o, h);
    cache[flyout] = list;
    setSaved(list);
    persist(flyout, list).catch((e) => setErr(String(e)));
  };

  const body = editing ? (
    <div className="secedit">
      <p className="hint">Drag to reorder · switch sections on or off</p>
      {order.map((id, i) => (
        <div
          key={id}
          className={`secrow${dragging === i ? " dragging" : ""}${hidden.has(id) ? " off" : ""}`}
          {...item(i)}
        >
          <i className="grip" aria-hidden="true">
            ⋮⋮
          </i>
          <span>
            <b>{byId[id].name}</b>
            {byId[id].hint && <small>{byId[id].hint}</small>}
          </span>
          <button
            className={`switch${hidden.has(id) ? "" : " on"}`}
            role="switch"
            aria-checked={!hidden.has(id)}
            title={hidden.has(id) ? "Show" : "Hide"}
            onClick={() => {
              const h = new Set(hidden);
              if (h.has(id)) h.delete(id);
              else h.add(id);
              save(order, h);
            }}
          >
            <i />
          </button>
        </div>
      ))}
      <button
        className="link reset"
        onClick={() => {
          cache[flyout] = null;
          setSaved(null);
          persist(flyout, null).catch((e) => setErr(String(e)));
        }}
      >
        Reset to default
      </button>
      {err && <p className="note">{err}</p>}
    </div>
  ) : (
    <>
      {order
        .filter((id) => !hidden.has(id))
        .map((id) => (
          <Fragment key={id}>{byId[id].node}</Fragment>
        ))}
    </>
  );

  const button = (
    <button className={`ghost custom${editing ? " on" : ""}`} title="Choose and order what this flyout shows" onClick={() => setEditing(!editing)}>
      {editing ? "Done" : "Customize"}
    </button>
  );
  return { body, button, editing, setEditing };
}
