import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { press } from "../lib/press";
import { DEFAULT_SITES, useSites } from "../widgets/Sites";
import type { Settings } from "../settings";

/** Each site with its status and response time; click to open it. Re-checked on open. */
export default function SitesView({ opened }: { opened: number }) {
  const [list, setList] = useState<string | null>(null);
  useEffect(() => {
    invoke<Settings>("get_settings")
      .then((s) => setList(String(s.widgets.find((w) => w.id === "sites")?.options?.list ?? DEFAULT_SITES)))
      .catch(() => setList(DEFAULT_SITES));
  }, [opened]);
  return list === null ? <div className="view stsv" /> : <Inner list={list} opened={opened} />;
}

function Inner({ list, opened }: { list: string; opened: number }) {
  const s = useSites(list, 30_000, opened, true);
  const open = (url: string) => invoke("open_link", { url }).then(() => invoke("popup_close"));
  return (
    <div className="view stsv">
      <header>
        <b>Sites</b>
        <span className="sp" />
        {s && <small className="ghlogin">{s.filter((x) => x.up).length} of {s.length} up</small>}
      </header>
      {!s && <p className="empty center">Checking…</p>}
      {s && !s.length && <p className="empty center">Add sites in the widget's settings.</p>}
      {s && s.length > 0 && (
        <div className="ghbody">
          {s.map((x) => (
            <div className="strow" key={x.url} {...press(() => open(x.url))} title={x.url}>
              <i className={`ghdot ${x.up ? "ok" : "fail"}`} />
              <div className="fmeta">
                <b>{x.name}</b>
                <small>{x.code ? `HTTP ${x.code}` : (x.error ?? "No answer")}</small>
              </div>
              <span className="ms">{x.code ? `${x.ms} ms` : ""}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
