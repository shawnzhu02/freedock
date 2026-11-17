import { useEffect, useState } from "react";
import { useKept } from "../lib/kept";
import { press } from "../lib/press";
import { invoke } from "@tauri-apps/api/core";
import { since } from "../lib/projects";
import { bytes } from "../lib/stats";

type Item = {
  name: string;
  path: string;
  size: number;
  modified: number;
  dir: boolean;
};

function Thumb({ kind, item }: { kind: string; item: Item }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    invoke<string | null>("files_thumb", { kind, path: item.path })
      .then((s) => alive && setSrc(s))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [kind, item.path, item.modified]);
  return (
    <div className="fthumb">
      {src ? <img src={src} alt="" draggable={false} /> : null}
    </div>
  );
}

/** The newest files in Downloads or Screenshots: click opens, the folder button shows it in Explorer. */
const TITLE = {
  downloads: "Downloads",
  screenshots: "Screenshots",
  recent: "Recent",
};

export default function FilesView({
  kind,
  opened,
}: {
  kind: "downloads" | "screenshots" | "recent";
  opened: number;
}) {
  const [items, setItems] = useKept<Item[] | null>(`files.${kind}`, null);
  const [err, setErr] = useState("");
  useEffect(() => {
    setErr("");
    invoke<Item[]>("files_list", { kind })
      .then(setItems)
      .catch((e) => setErr(String(e)));
  }, [kind, opened]);
  const act = (path: string, how: string) =>
    invoke("files_open", { kind, path, how })
      .then(() => invoke("popup_close"))
      .catch((e) => setErr(String(e)));
  const now = Date.now() / 1000;
  const grid = kind === "screenshots";
  return (
    <div className="view filesv">
      <header>
        <b>{TITLE[kind]}</b>
        <span className="sp" />
        <button className="ghost" onClick={() => act("", "folder")}>
          Open folder
        </button>
      </header>
      {err && <p className="note">{err}</p>}
      {items && !items.length && (
        <p className="empty center">Nothing here yet.</p>
      )}
      <div className={grid ? "fgrid" : "flist"}>
        {(items ?? []).map((it) => (
          <div
            className="fitem"
            key={it.path}
            title={it.name}
            {...press(() => act(it.path, "open"))}
          >
            <Thumb kind={kind} item={it} />
            <div className="fmeta">
              <b>{it.name}</b>
              <small>
                {since(it.modified, now)}
                {!it.dir && kind !== "recent" && ` · ${bytes(it.size)}`}
              </small>
            </div>
            {kind !== "recent" && (
              <button
                className="freveal"
                title="Show in folder"
                onClick={(e) => {
                  e.stopPropagation();
                  act(it.path, "reveal");
                }}
              >
                ↗
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
