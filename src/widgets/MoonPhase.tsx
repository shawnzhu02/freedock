import { useEffect, useState } from "react";
import { pollActive } from "../lib/active";
import { moonAt, type Moon } from "../lib/moon";
import type { WidgetProps } from "./registry";

/** Moon phase: tonight's moon, drawn to scale, with the phase name and how much of it is lit.
 *  Computed on the PC (lib/moon.ts), no network. */

/** The lit part of a moon of radius r centred at (r, r): a half disc on the lit side, closed by an
 *  ellipse whose width follows the phase. Waxing moons are lit on the right (as seen from the north). */
function litPath(m: Moon, r: number): string {
  const rx = Math.abs(1 - 2 * m.lit) * r;
  const side = m.waxing ? 1 : 0; // which half of the disc is lit
  const bulge = m.lit > 0.5 ? side : 1 - side; // gibbous: the ellipse bulges into the dark half
  return `M${r} 0 A${r} ${r} 0 0 ${side} ${r} ${2 * r} A${rx} ${r} 0 0 ${bulge} ${r} 0Z`;
}

export function MoonGlyph({ moon, size = 18 }: { moon: Moon; size?: number }) {
  const r = size / 2;
  return (
    <svg className="moonglyph" viewBox={`0 0 ${size} ${size}`} width={size} height={size} aria-hidden="true">
      <circle cx={r} cy={r} r={r - 0.5} fill="currentColor" opacity=".18" />
      {moon.lit > 0.005 && <path d={litPath(moon, r)} fill="#f6f1d8" />}
    </svg>
  );
}

export default function MoonPhaseCell({ settings, options }: WidgetProps) {
  const [moon, setMoon] = useState(() => moonAt(Date.now()));

  useEffect(() => {
    const load = () => setMoon(moonAt(Date.now()));
    load();
    return pollActive(30 * 60_000, load); // the phase barely moves in half an hour
  }, []);

  const vertical = settings.edge === "left" || settings.edge === "right";
  const pct = Math.round(moon.lit * 100);
  return (
    <button className="cell stat moon" title={`${moon.name}, ${pct}% lit`} aria-label={`${moon.name}, ${pct}% lit`}>
      <MoonGlyph moon={moon} size={vertical ? 20 : 18} />
      {!vertical && options.label !== false && (
        <span className="stext">
          <small>{moon.name}</small>
          <b>{pct}%</b>
        </span>
      )}
    </button>
  );
}
