import { useEffect, useRef, useState } from "react";
import { useKept } from "../lib/kept";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { since } from "../lib/projects";

type Clip = { id: number; text: string; at: number };

/** Recent copies (this session only). Click or Enter copies one back; Ask sends it to Quick Ask. */
export default function ClipboardView({ opened }: { opened: number }) {
  const [clips, setClips] = useKept<Clip[]>("clipboard.clips", []);
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const [copied, setCopied] = useState<number | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const load = () => invoke<Clip[]>("clip_list").then(setClips).catch(() => {});
  useEffect(() => {
    setQ("");
    setSel(0);
    setCopied(null);
    load();
    setTimeout(() => input.current?.focus(), 60);
  }, [opened]);
  useEffect(() => {
    const off = listen("clips-changed", load);
    return () => {
      off.then((f) => f());
    };
  }, []);
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  const shown = clips.filter((c) => words.every((w) => c.text.toLowerCase().includes(w)));
  const now = Date.now() / 1000;
  const copy = (c: Clip) =>
    invoke("clip_copy", { id: c.id }).then(() => {
      setCopied(c.id);
      setTimeout(() => invoke("popup_close").catch(() => {}), 350);
    });
  return (
    <div className="view projv clipv">
      <header>
        <input
          ref={input}
          className="psearch"
          placeholder="Search copied text"
          value={q}
          onChange={(e) => {
            setQ(e.target.value);
            setSel(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") setSel((n) => Math.min(shown.length - 1, n + 1));
            else if (e.key === "ArrowUp") setSel((n) => Math.max(0, n - 1));
            else if (e.key === "Enter" && shown[sel]) copy(shown[sel]);
            else return;
            e.preventDefault();
          }}
        />
      </header>
      <div className="plist">
        {!clips.length && <p className="empty center">Copy some text and it shows up here. Nothing is saved to disk.</p>}
        {clips.length > 0 && !shown.length && <p className="empty center">Nothing matches.</p>}
        {shown.map((c, i) => (
          <div className={`prow crow${i === sel ? " sel" : ""}`} key={c.id} onMouseEnter={() => setSel(i)} onClick={() => copy(c)}>
            <div className="pmain">
              <span className="ctext">{c.text.length > 400 ? c.text.slice(0, 400) + "…" : c.text}</span>
              <small>
                <span>{copied === c.id ? "Copied" : since(c.at, now)}</span>
                {c.text.length > 120 && <span>{c.text.length.toLocaleString()} chars</span>}
              </small>
            </div>
            <div className="pacts">
              <button
                title="Ask Claude about this"
                onClick={(e) => {
                  e.stopPropagation();
                  invoke("clip_ask", { id: c.id });
                }}
              >
                Ask
              </button>
              <button
                title="Forget this one"
                onClick={(e) => {
                  e.stopPropagation();
                  invoke("clip_delete", { id: c.id }).then(load);
                }}
              >
                ✕
              </button>
            </div>
          </div>
        ))}
      </div>
      <footer className="phint cfoot">
        <span>Kept in memory only · private copies skipped</span>
        {clips.length > 0 && (
          <button className="link" onClick={() => invoke("clip_delete", { id: null }).then(load)}>
            Clear all
          </button>
        )}
      </footer>
    </div>
  );
}
