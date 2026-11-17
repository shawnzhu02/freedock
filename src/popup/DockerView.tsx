import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useDocker, type Container } from "../widgets/Docker";

function Row({ c, busy, act }: { c: Container; busy: boolean; act: (c: Container, a: string) => void }) {
  const up = c.state === "running";
  return (
    <div className="dkrow" title={`${c.image}${c.ports ? `\n${c.ports}` : ""}`}>
      <i className={`ghdot ${up ? "ok" : c.state === "restarting" ? "wait" : ""}`} />
      <div className="fmeta">
        <b>{c.name}</b>
        <small>
          {c.image} · {c.status}
        </small>
      </div>
      <div className="dkact">
        {up ? (
          <>
            <button className="ghost" disabled={busy} onClick={() => act(c, "restart")} title="Restart">
              ↻
            </button>
            <button className="ghost" disabled={busy} onClick={() => act(c, "stop")} title="Stop">
              ■
            </button>
          </>
        ) : (
          <button className="ghost" disabled={busy} onClick={() => act(c, "start")} title="Start">
            ▶
          </button>
        )}
      </div>
    </div>
  );
}

/** Docker containers: running first, with start, stop and restart. */
export default function DockerView({ opened }: { opened: number }) {
  const [d, setD] = useDocker(10_000, opened);
  const [busy, setBusy] = useState("");
  const [err, setErr] = useState("");
  const act = (c: Container, action: string) => {
    setBusy(c.id);
    setErr("");
    invoke("docker_act", { id: c.id, action })
      .catch((e) => setErr(String(e)))
      .then(() => invoke<{ containers: Container[]; error: string | null }>("docker_list"))
      .then((x) => x && setD(x))
      .finally(() => setBusy(""));
  };
  const running = d?.containers.filter((c) => c.state === "running").length ?? 0;
  return (
    <div className="view dkv">
      <header>
        <b>Docker</b>
        {d && !d.error && <small className="ghlogin">{running} running</small>}
        <span className="sp" />
      </header>
      {!d && <p className="empty center">Loading…</p>}
      {d?.error && <p className="note">{d.error}</p>}
      {err && <p className="note">{err}</p>}
      {d && !d.error && (
        <div className="ghbody">
          {d.containers.length ? d.containers.map((c) => <Row key={c.id} c={c} busy={busy === c.id} act={act} />) : <p className="empty">No containers.</p>}
        </div>
      )}
    </div>
  );
}
