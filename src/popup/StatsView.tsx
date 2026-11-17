import { invoke } from "@tauri-apps/api/core";
import { bytes, engineLabel, gpuShort, sparkPath, speed, uptime, useGpu, useStats } from "../lib/stats";
import { heat } from "../widgets/Stats";
import { useSections } from "./Sections";

function Meter({ name, pct, right, sub }: { name: string; pct: number; right: string; sub?: string }) {
  return (
    <div className={`ubar ${heat(pct)}`}>
      <div className="uhead">
        <b>{name}</b>
        <span>{right}</span>
      </div>
      <div className="track">
        <i style={{ width: `${Math.min(100, Math.max(0, pct))}%` }} />
      </div>
      {sub && <small>{sub}</small>}
    </div>
  );
}

/** The System flyout: CPU with a one-minute graph, memory, network, drives and the busiest apps. */
export default function StatsView({ opened }: { opened?: number }) {
  const { s, history } = useStats(true);
  const { g } = useGpu();
  const mem = s?.mem_total ? (s.mem_used / s.mem_total) * 100 : 0;
  const { body, button } = useSections(
    "stats",
    !s
      ? []
      : [
          {
            id: "cpu",
            name: "CPU",
            hint: "load with a one-minute graph",
            node: (
              <div className={`cpu ${heat(s.cpu)}`}>
                <div className="uhead">
                  <b>CPU</b>
                  <span>
                    {Math.round(s.cpu)}% <small>· {s.cores} threads</small>
                  </span>
                </div>
                <svg viewBox="0 0 280 48" preserveAspectRatio="none" className="graph" aria-hidden="true">
                  <path d={sparkPath(history, 280, 48) + (history.length > 1 ? " L280 48 L0 48 Z" : "")} className="fill" />
                  <path d={sparkPath(history, 280, 48)} className="line" />
                </svg>
              </div>
            ),
          },
          {
            id: "gpu",
            name: "GPUs",
            hint: "load and VRAM per graphics card",
            node: (
              <>
                {g?.map((x) => (
                  <Meter
                    key={x.name}
                    name={`GPU · ${gpuShort(x.name)}`}
                    pct={x.load}
                    right={`${Math.round(x.load)}%`}
                    sub={[x.engine && x.load >= 1 ? engineLabel(x.engine) : "idle", x.mem_total ? `${bytes(x.mem_used)} of ${bytes(x.mem_total)} VRAM` : ""].filter(Boolean).join(" · ")}
                  />
                ))}
              </>
            ),
          },
          { id: "memory", name: "Memory", node: <Meter name="Memory" pct={mem} right={`${Math.round(mem)}%`} sub={`${bytes(s.mem_used)} of ${bytes(s.mem_total)} in use`} /> },
          {
            id: "network",
            name: "Network",
            hint: "download and upload speed",
            node: (
              <div className="netrow">
                <div>
                  <small>Download</small>
                  <b>{speed(s.down)}</b>
                </div>
                <div>
                  <small>Upload</small>
                  <b>{speed(s.up)}</b>
                </div>
              </div>
            ),
          },
          {
            id: "drives",
            name: "Drives",
            hint: "free space",
            node: (
              <>
                {s.disks.map((d) => (
                  <Meter key={d.root} name={`Drive ${d.root}`} pct={((d.total - d.free) / d.total) * 100} right={`${bytes(d.free)} free`} sub={`of ${bytes(d.total)}`} />
                ))}
              </>
            ),
          },
          {
            id: "apps",
            name: "Busiest apps",
            hint: "CPU and memory per app",
            node:
              s.apps.length > 0 ? (
                <div className="apps">
                  <div className="uhead">
                    <b>Busiest apps</b>
                    <span className="dim">CPU · memory</span>
                  </div>
                  {s.apps.slice(0, 6).map((a) => (
                    <div className="arow" key={a.name}>
                      <span className="aname" title={a.count > 1 ? `${a.count} processes` : undefined}>
                        {a.name}
                        {a.count > 1 && <i> ×{a.count}</i>}
                      </span>
                      <span className="acpu">{a.cpu.toFixed(1)}%</span>
                      <span className="amem">{bytes(a.mem)}</span>
                    </div>
                  ))}
                </div>
              ) : null,
          },
        ],
    opened,
  );
  if (!s) {
    return (
      <div className="view statsv">
        <p className="empty center">Reading…</p>
      </div>
    );
  }
  return (
    <div className="view statsv">
      <header>
        <b>System</b>
        {s.uptime > 0 && (
          <small className="dim" title="Time since Windows last started (restart, not shut down, if Fast Startup is on)">
            &nbsp;· up {uptime(s.uptime)}
          </small>
        )}
        <span className="sp" />
        {button}
        <button className="ghost" onClick={() => invoke("open_windows", { page: "taskmgr" })}>
          Task Manager
        </button>
      </header>
      <div className="ubody">{body}</div>
    </div>
  );
}
