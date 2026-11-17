import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { evaluate, show } from "../lib/calc";

type Line = { expr: string; value: number };

// Kept while the dock runs (the popup window stays alive between opens); never saved.
let history: Line[] = [];

/** Calculator: the answer updates as you type; Enter keeps it in the list, a click copies a result. */
export default function CalcView({ opened }: { opened: number }) {
  const [expr, setExpr] = useState("");
  const [lines, setLines] = useState<Line[]>(history);
  const [copied, setCopied] = useState(-1);
  const box = useRef<HTMLInputElement>(null);
  useEffect(() => {
    setLines(history);
    requestAnimationFrame(() => box.current?.focus());
  }, [opened]);
  const ans = lines[0]?.value ?? 0;
  let result = "";
  let err = "";
  if (expr.trim()) {
    try {
      result = show(evaluate(expr, ans));
    } catch (e) {
      err = (e as Error).message;
    }
  }
  const keep = () => {
    if (!expr.trim() || err) return;
    history = [{ expr, value: evaluate(expr, ans) }, ...history].slice(0, 30);
    setLines(history);
    setExpr("");
  };
  const copy = (v: number, i: number) => {
    navigator.clipboard
      .writeText(String(Number(v.toPrecision(12))))
      .then(() => {
        setCopied(i);
        window.setTimeout(() => setCopied(-1), 1200);
      })
      .catch(() => {});
  };
  return (
    <div className="view calcv">
      <div className="calcin">
        <input
          ref={box}
          value={expr}
          placeholder="15% of 80, 2^10, sqrt(2)…"
          spellCheck={false}
          onChange={(e) => setExpr(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") keep();
            if (e.key === "Escape") invoke("popup_close");
            if (e.key === "ArrowUp" && !expr && lines[0]) setExpr(lines[0].expr);
          }}
        />
        <div className={`calcout${err ? " err" : ""}`}>{err ? (expr.trim().length > 1 ? "…" : "") : result ? `= ${result}` : ""}</div>
      </div>
      {lines.length > 0 && (
        <div className="calclist">
          {lines.map((l, i) => (
            <div className="calcrow" key={i} onClick={() => copy(l.value, i)} title="Click to copy the result">
              <small>{l.expr}</small>
              <b>{copied === i ? "Copied" : show(l.value)}</b>
            </div>
          ))}
        </div>
      )}
      <p className="calcnote">Enter keeps a result · "ans" is the last one · ↑ edits it</p>
    </div>
  );
}
