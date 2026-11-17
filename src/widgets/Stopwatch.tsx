import { useState } from "react";
import type { WidgetProps } from "./registry";
import { useNow } from "./useNow";
import { clock } from "../lib/clock";

/** Running since (ms epoch) and time banked from earlier runs. Module-level so a re-render or a
 *  settings change doesn't lose a running stopwatch. */
const sw = { since: null as number | null, banked: 0 };

/** Stopwatch: click to start or pause; ↺ resets once paused. */
export default function Stopwatch({ settings }: WidgetProps) {
  const [, bump] = useState(0);
  const running = sw.since !== null;
  const now = useNow(running).getTime();
  const elapsed = (sw.banked + (running ? Math.max(0, now - sw.since!) : 0)) / 1000;
  const vertical = settings.edge === "left" || settings.edge === "right";
  const toggle = () => {
    if (sw.since === null) sw.since = Date.now();
    else {
      sw.banked += Date.now() - sw.since;
      sw.since = null;
    }
    bump((n) => n + 1);
  };
  const reset = (e: React.MouseEvent) => {
    e.stopPropagation();
    sw.since = null;
    sw.banked = 0;
    bump((n) => n + 1);
  };
  return (
    <div className={`cell group stat ctl swatch${running ? " on" : ""}`}>
      <button className="swmain" title={running ? "Pause" : elapsed ? "Resume" : "Start the stopwatch"} onClick={toggle}>
        <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true" className="pglyph">
          <circle cx="12" cy="13.5" r="7.5" />
          <path d="M10 2.5h4M12 2.5v3.5M12 13.5V9.5M18.2 6.6l1.3-1.3" />
        </svg>
        {(!vertical || elapsed > 0) && <b className="cval">{vertical ? clock(elapsed).split(":").slice(-2).join(":") : clock(elapsed)}</b>}
      </button>
      {!running && elapsed > 0 && !vertical && (
        <button className="swreset" title="Reset" onClick={reset}>
          ↺
        </button>
      )}
    </div>
  );
}
