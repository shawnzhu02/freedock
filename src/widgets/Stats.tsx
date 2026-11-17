import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { bytes, engineLabel, gpuShort, sparkPath, speed, useGpu, useStats, type Stats } from "../lib/stats";

/** The busier a reading, the warmer its colour: calm under 60%, amber to 85%, red above. */
export const heat = (p: number) => (p >= 85 ? "max" : p >= 60 ? "high" : "ok");

function Cell({ settings, label, value, sub, pctv, spark }: WidgetProps & { label: string; value: string; sub?: string; pctv?: number; spark?: number[] }) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  return (
    <button className={`cell stat ${pctv !== undefined ? heat(pctv) : ""}`} title={`${label}: ${value}${sub ? ` (${sub})` : ""}`} onClick={(e) => openAt("stats", e, vertical)}>
      {spark && spark.length > 1 && !vertical && (
        <svg className="spark" viewBox="0 0 40 22" width="40" height="22" preserveAspectRatio="none" aria-hidden="true">
          <path d={sparkPath(spark.slice(-20), 40, 22)} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" />
        </svg>
      )}
      <span className="stext">
        <small>{label}</small>
        <b>{value}</b>
      </span>
    </button>
  );
}

const memPct = (s: Stats) => (s.mem_total ? (s.mem_used / s.mem_total) * 100 : 0);

export function CpuCell(p: WidgetProps) {
  const { s, history } = useStats();
  return <Cell {...p} label="CPU" value={s ? `${Math.round(s.cpu)}%` : "…"} pctv={s?.cpu} spark={p.options.graph === false ? undefined : history} />;
}

export function RamCell(p: WidgetProps) {
  const { s } = useStats();
  const pc = s ? memPct(s) : 0;
  const value = !s ? "…" : p.options.show === "gb" ? bytes(s.mem_used) : `${Math.round(pc)}%`;
  return <Cell {...p} label="Memory" value={value} sub={s ? `${bytes(s.mem_used)} of ${bytes(s.mem_total)}` : undefined} pctv={pc} />;
}

export function NetCell(p: WidgetProps) {
  const { s } = useStats();
  const vertical = p.settings.edge === "left" || p.settings.edge === "right";
  return (
    <button className="cell stat net" title={s ? `Down ${speed(s.down)}, up ${speed(s.up)}` : "Network"} onClick={(e) => openAt("stats", e, vertical)}>
      <span className="nrow">
        <i>↓</i>
        {s ? speed(s.down) : "…"}
      </span>
      <span className="nrow">
        <i>↑</i>
        {s ? speed(s.up) : "…"}
      </span>
    </button>
  );
}

export function StorageCell(p: WidgetProps) {
  const { s } = useStats();
  const want = String(p.options.drive ?? "C:");
  const d = s?.disks.find((x) => x.root.toUpperCase() === want.toUpperCase()) ?? s?.disks[0];
  const used = d ? ((d.total - d.free) / d.total) * 100 : 0;
  return <Cell {...p} label={d ? `${d.root} free` : "Storage"} value={d ? bytes(d.free) : "…"} sub={d ? `of ${bytes(d.total)}` : undefined} pctv={used} />;
}

export function GpuCell(p: WidgetProps) {
  const { g, history } = useGpu();
  const which = p.options.which === "busiest" ? [...(g ?? [])].sort((a, b) => b.load - a.load)[0] : g?.[0];
  const show = String(p.options.show ?? "load");
  if (g && !which) return <Cell {...p} label="GPU" value="none" />;
  const value = !which ? "…" : show === "vram" && which.mem_total ? bytes(which.mem_used) : `${Math.round(which.load)}%`;
  const sub = which
    ? `${which.name}: ${Math.round(which.load)}%${which.engine ? ` (${engineLabel(which.engine)})` : ""}${which.mem_total ? `, ${bytes(which.mem_used)} of ${bytes(which.mem_total)} VRAM` : ""}`
    : undefined;
  const pctv = show === "vram" && which?.mem_total ? (which.mem_used / which.mem_total) * 100 : which?.load;
  return (
    <Cell
      {...p}
      label={which && (g?.length ?? 0) > 1 ? gpuShort(which.name) : "GPU"}
      value={value}
      sub={sub}
      pctv={pctv}
      spark={p.options.graph === false || p.options.which === "busiest" ? undefined : history}
    />
  );
}

export function TopCell(p: WidgetProps) {
  const { s } = useStats(true);
  const top = s?.apps[0];
  return <Cell {...p} label="Busiest" value={top ? `${top.name} ${Math.round(top.cpu)}%` : "…"} pctv={top?.cpu} />;
}
