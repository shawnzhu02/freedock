import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { filter, since, type Project } from "../lib/projects";

const ACTIONS: { how: string; label: string; title: string }[] = [
  { how: "code", label: "Code", title: "Open in VS Code" },
  { how: "terminal", label: "Term", title: "Open a terminal here" },
  { how: "claude", label: "Claude", title: "Start Claude Code here" },
  { how: "explorer", label: "Files", title: "Show in Explorer" },
];

/** Recent repos in ~/Programming. Type to filter; Enter opens the top one in VS Code. */
export default function ProjectsView({ opened }: { opened: number }) {
  const [list, setList] = useState<Project[] | null>(null);
  const [q, setQ] = useState("");
  const [err, setErr] = useState("");
  const [sel, setSel] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    setQ("");
    setSel(0);
    setErr("");
    invoke<Project[]>("projects_list").then(setList).catch((e) => setErr(String(e)));
    setTimeout(() => input.current?.focus(), 60);
  }, [opened]);
  const shown = filter(list ?? [], q).slice(0, 40);
  const now = Date.now() / 1000;
  const open = (p: Project, how: string) => {
    setErr("");
    invoke("project_open", { path: p.path, how })
      .then(() => invoke("popup_close").catch(() => {}))
      .catch((e) => setErr(String(e)));
  };
  return (
    <div className="view projv">
      <header>
        <input
          ref={input}
          className="psearch"
          placeholder="Find a project"
          value={q}
          onChange={(e) => {
            setQ(e.target.value);
            setSel(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") setSel((n) => Math.min(shown.length - 1, n + 1));
            else if (e.key === "ArrowUp") setSel((n) => Math.max(0, n - 1));
            else if (e.key === "Enter" && shown[sel]) open(shown[sel], e.ctrlKey ? "claude" : e.shiftKey ? "terminal" : "code");
            else return;
            e.preventDefault();
          }}
        />
      </header>
      {err && <p className="note">{err}</p>}
      <div className="plist">
        {!list && !err && <p className="empty center">Looking…</p>}
        {list && !shown.length && <p className="empty center">{list.length ? "No project matches." : "No git repos found. freedock looks in Programming, Projects, code, dev, repos, source\repos and Documents\GitHub in your user folder."}</p>}
        {shown.map((p, i) => (
          <div className={`prow${i === sel ? " sel" : ""}`} key={p.path} onMouseEnter={() => setSel(i)} onDoubleClick={() => open(p, "code")}>
            <div className="pmain">
              <b>{p.name}</b>
              <small>
                {p.branch && <span className="br">{p.branch}</span>}
                <span>git {since(p.touched, now)}</span>
                {p.claude && <span className="cl">Claude {since(p.claude, now)}</span>}
              </small>
            </div>
            <div className="pacts">
              {ACTIONS.map((a) => (
                <button key={a.how} title={a.title} onClick={() => open(p, a.how)}>
                  {a.label}
                </button>
              ))}
              {p.web && (
                <button title="Open on GitHub" onClick={() => open(p, "github")}>
                  Web
                </button>
              )}
            </div>
          </div>
        ))}
      </div>
      <footer className="phint">Enter: VS Code · Shift+Enter: terminal · Ctrl+Enter: Claude</footer>
    </div>
  );
}
