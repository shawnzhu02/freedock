import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { bytes } from "../lib/stats";
import { isActive } from "../lib/active";

/** First non-empty line of the notes, for the cell. */
export function firstLine(text: string): string {
  return (text.split(/\r?\n/).find((l) => l.trim()) ?? "").replace(/^[#>*\-\s\[\]x]+/i, "").trim();
}

/** Notes: a scratch pad in a flyout; the cell can show the first line. */
export function NotesCell({ settings, options }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  const [line, setLine] = useState("");
  useEffect(() => {
    const get = () => invoke<string>("notes_get").then((t) => setLine(firstLine(t))).catch(() => {});
    get();
    // The flyout saves as the user types; refresh when the dock comes back into view.
    const t = window.setInterval(() => isActive() && get(), 10_000);
    return () => clearInterval(t);
  }, []);
  const preview = options.preview !== false && !vertical && line;
  return (
    <button className={`cell projects notes${preview ? " withtext" : ""}`} title={line ? `Notes: ${line}` : "Notes"} onClick={(e) => openAt("notes", e, vertical)}>
      <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" className="pglyph">
        <path d="M6 3.5h9l3.5 3.5v13a.5.5 0 0 1-.5.5H6a.5.5 0 0 1-.5-.5V4a.5.5 0 0 1 .5-.5z" />
        <path d="M9 10h6M9 13.5h6M9 17h3.5" />
      </svg>
      {preview && <span className="nline">{line}</span>}
    </button>
  );
}

type Bin = { bytes: number; items: number };

/** Recycle Bin: how much is in it; a click opens it, Empty empties it (Windows asks first). */
export function RecycleCell({ settings }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  const [bin, setBin] = useState<Bin | null>(null);
  const load = () => invoke<Bin | null>("recycle_bin").then(setBin).catch(() => {});
  useEffect(() => {
    load();
    const t = window.setInterval(() => isActive() && load(), 60_000);
    return () => clearInterval(t);
  }, []);
  const full = !!bin && bin.items > 0;
  return (
    <div className="cell group recycle">
      <button
        className="rmain"
        title={bin ? (full ? `Recycle Bin: ${bin.items} item${bin.items === 1 ? "" : "s"}, ${bytes(bin.bytes)}` : "Recycle Bin is empty") : "Recycle Bin"}
        onClick={() => invoke("open_windows", { page: "recycle" })}
      >
        <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" className="pglyph">
          <path d="M4.5 6.5h15M9.5 6.5V4.5h5v2M6.5 6.5l1 13a1 1 0 0 0 1 1h7a1 1 0 0 0 1-1l1-13" />
          {full && <path d="M10 10.5v6.5M14 10.5v6.5" />}
        </svg>
        {!vertical && full && <span className="rsize">{bytes(bin!.bytes)}</span>}
      </button>
      {full && !vertical && (
        <button className="rempty" title="Empty the Recycle Bin (Windows asks first)" onClick={() => invoke("recycle_empty").finally(load)}>
          Empty
        </button>
      )}
    </div>
  );
}
