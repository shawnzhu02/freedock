import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { pollActive } from "../lib/active";

export type Site = { url: string; name: string; up: boolean; code: number; ms: number; error: string | null };

export const DEFAULT_SITES = "";

/** Checks the list every `everyMs` while visible (the backend caches for a minute). */
export function useSites(list: string, everyMs: number, key = 0, fresh = false) {
  const [s, setS] = useState<Site[] | null>(null);
  useEffect(() => {
    // Checks are slow (they wait on each site's timeout): an answer for the list before an edit, or
    // an older poll, must not land on top of a newer one.
    let alive = true;
    let seq = 0;
    const get = (f: boolean) => {
      const n = ++seq;
      invoke<Site[]>("sites_check", { list, fresh: f })
        .then((v) => alive && n === seq && setS(v))
        .catch(() => {});
    };
    get(fresh);
    const stop = pollActive(everyMs, () => get(false));
    return () => {
      alive = false;
      stop();
    };
  }, [list, everyMs, key, fresh]);
  return s;
}

/** Site status: a dot per site, red when one is down. */
export default function SitesCell({ settings, options }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  const list = String(options.list ?? DEFAULT_SITES);
  const s = useSites(list, 2 * 60_000);
  const down = s?.filter((x) => !x.up) ?? [];
  const tip = !s ? "Checking your sites…" : !s.length ? "Site status: add sites in the widget's settings" : down.length ? `Down: ${down.map((d) => d.name).join(", ")}` : `All ${s.length} sites up`;
  return (
    <button className={`cell stat ctl sitesc${down.length ? " bad" : ""}`} title={tip} onClick={(e) => openAt("sites", e, vertical)}>
      <span className="sdots" aria-hidden="true">
        {(s ?? list.split(",").filter((x) => x.trim()).slice(0, 8)).map((x, i) => (
          <i key={i} className={typeof x === "string" ? "wait" : x.up ? "" : "down"} />
        ))}
      </span>
      {!vertical && <b className="cval">{s ? (!s.length ? "Add sites" : down.length ? `${down.length} down` : "Up") : "…"}</b>}
    </button>
  );
}
