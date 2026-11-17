import { useEffect, useState } from "react";
import { useKept } from "../lib/kept";
import { invoke } from "@tauri-apps/api/core";
import { FIVE_HOURS, SEVEN_DAYS, ago, level, pct, resetIn, runsOutAt, type Usage, type UsageWindow } from "../lib/usage";

function Bar({ name, w, now, length }: { name: string; w: UsageWindow | null; now: number; length: number }) {
  const out = runsOutAt(w, length, now);
  return (
    <div className={`ubar ${level(w)}`}>
      <div className="uhead">
        <b>{name}</b>
        <span>{w ? `${pct(w)}%` : "—"}</span>
      </div>
      <div className="track">
        <i style={{ width: `${w ? Math.min(100, w.utilization * 100) : 0}%` }} />
      </div>
      <small>{w ? `Resets ${resetIn(w.resets_at, now, undefined)}` : "Not reported"}</small>
      {out && <small className="uout">At this pace you'll hit it {resetIn(out, now, undefined)}</small>}
    </div>
  );
}

/** Claude usage details: both limits with reset times, and a refresh. */
export default function UsageView({ opened }: { opened: number }) {
  const [u, setU] = useKept<Usage | null>("usage.u", null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);
  const [now, setNow] = useState(() => Date.now() / 1000);
  const load = (maxAge: number) => {
    setBusy(true);
    invoke<Usage>("claude_usage", { maxAge })
      .then((x) => {
        setU(x);
        setErr("");
      })
      .catch((e) => setErr(String(e)))
      .finally(() => {
        setBusy(false);
        setNow(Date.now() / 1000);
      });
  };
  useEffect(() => load(5 * 60), [opened]);
  return (
    <div className="view usagev">
      <header>
        <b>Claude usage</b>
        <span className="sp" />
        <button className="ghost" disabled={busy} onClick={() => load(0)}>
          {busy ? "Checking…" : "Refresh"}
        </button>
      </header>
      <div className="ubody">
        {err && <p className="note">{err}</p>}
        <Bar name="5-hour limit" w={u?.five_hour ?? null} now={now} length={FIVE_HOURS} />
        <Bar name="Weekly limit" w={u?.seven_day ?? null} now={now} length={SEVEN_DAYS} />
        {/* Always one line tall (a no-break space when empty), so the card doesn't resize as it loads. */}
        <p className="umeta">{u ? `Updated ${ago(u.at, now)} · from Claude Code` : busy ? "Asking Claude Code…" : " "}</p>
      </div>
    </div>
  );
}
