import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

const ICONS: Record<string, ReactNode> = {
  lock: <path d="M7 11V8a5 5 0 0 1 10 0v3M5.5 11h13v9.5h-13zM12 15v2.5" />,
  sleep: <path d="M19.5 14.5A8 8 0 1 1 9.5 4.5a6.5 6.5 0 0 0 10 10z" />,
  signout: <path d="M14 4.5H6.5v15H14M10.5 12h10M17 8.5l3.5 3.5-3.5 3.5" />,
  restart: <path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5v4.5H15" />,
  shutdown: <path d="M12 3.5v8M7.2 6.3a7.5 7.5 0 1 0 9.6 0" />,
};

const LABELS: Record<string, [string, string]> = {
  lock: ["Lock", ""],
  sleep: ["Sleep", ""],
  signout: ["Sign out", "Click again to sign out"],
  restart: ["Restart", "Click again to restart"],
  shutdown: ["Shut down", "Click again to shut down"],
};

/** Lock and sleep go straight away; sign out, restart and shut down need a second click within 4 s. */
export default function PowerView({ opened }: { opened: number }) {
  const [armed, setArmed] = useState("");
  const [err, setErr] = useState("");
  useEffect(() => {
    setArmed("");
    setErr("");
  }, [opened]);
  useEffect(() => {
    if (!armed) return;
    const t = window.setTimeout(() => setArmed(""), 4000);
    return () => clearTimeout(t);
  }, [armed]);
  const press = (a: string) => {
    if (LABELS[a][1] && armed !== a) {
      setArmed(a);
      return;
    }
    setArmed("");
    invoke("power_action", { action: a }).catch((e) => setErr(String(e)));
  };
  const btn = (a: string, wide = false) => (
    <button key={a} className={`pwbtn${armed === a ? " arm" : ""}${wide ? " wide" : ""}`} onClick={() => press(a)}>
      <svg viewBox="0 0 24 24" aria-hidden="true">
        {ICONS[a]}
      </svg>
      {armed === a ? LABELS[a][1] : LABELS[a][0]}
    </button>
  );
  return (
    <div className="view pwv">
      <header>
        <b>Power</b>
      </header>
      {err && <p className="note">{err}</p>}
      <div className="pwgrid">
        {btn("lock")}
        {btn("sleep")}
        {btn("restart")}
        {btn("shutdown")}
        {btn("signout", true)}
      </div>
    </div>
  );
}
