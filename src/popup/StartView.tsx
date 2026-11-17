import { useEffect, useMemo, useRef, useState } from "react";
import { useKept } from "../lib/kept";
import { invoke } from "@tauri-apps/api/core";
import { AppIcon } from "../widgets/Apps";
import { useSections } from "./Sections";
import { useReorder } from "../lib/useReorder";
import { SETTINGS_PAGES, rankName } from "../lib/start";

// freedock's own Start menu (src-tauri/src/startmenu.rs). Type to search the way Windows' Start
// does: apps, programs and files from the Windows Search index, and the Settings index, ranked by
// name and by how often you start them (startmenu::win::find). Otherwise pinned apps, recent ones and
// every app A-Z, in the order the user picks (Customize). The footer has the user, Settings, File
// Explorer, power, and Windows' own Start.

type StartApp = { name: string; id: string };
type Info = { pins: string[]; recent: string[]; user: string };
type FileItem = { name: string; path: string; dir: boolean };
/** One search result from start_find: kind is "app", "setting", or a file kind ("folder"...). */
type Found = { name: string; target: string; kind: string; place: string | null };
type Finds = { apps: Found[]; settings: Found[]; files: Found[]; settings_ready: boolean };
type Hit = { key: string; group: string; label: string; sub: string; icon: string | null; target: string | null; run: (admin?: boolean) => void };

const NONE: Finds = { apps: [], settings: [], files: [], settings_ready: true };
/** Windows' Start groups, in its order. */
const FILE_GROUPS: [string, string][] = [["folder", "Folders"], ["program", "Programs"], ["document", "Documents"], ["photo", "Photos"], ["video", "Videos"], ["music", "Music"]];

const open = (target: string, admin = false) => invoke("start_open", { target, admin }).catch(() => {});
/** "C:\Users\alex\Programs\ThrottleStop" -> "Programs\ThrottleStop": enough to tell places apart. */
const shortPlace = (p: string | null) => (p ? p.split("\\").filter(Boolean).slice(-2).join("\\") : "");

export default function StartView({ opened }: { opened: number }) {
  const [apps, setApps] = useKept<StartApp[]>("start.apps", []);
  const [info, setInfo] = useKept<Info>("start.info", { pins: [], recent: [], user: "" });
  const [files, setFiles] = useKept<FileItem[]>("start.files", []);
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const box = useRef<HTMLInputElement>(null);
  const loadInfo = () => invoke<Info>("start_state").then(setInfo).catch(() => {});
  useEffect(() => {
    setQ("");
    setSel(0);
    invoke<StartApp[]>("start_apps").then(setApps).catch(() => {});
    invoke<FileItem[]>("files_list", { kind: "recent" }).then(setFiles).catch(() => {});
    loadInfo();
    setTimeout(() => box.current?.focus(), 30);
  }, [opened]);

  const byId = useMemo(() => new Map(apps.map((a) => [a.id, a])), [apps]);
  const pinned = info.pins.map((id) => byId.get(id)).filter((a): a is StartApp => !!a);
  const recentApps = info.recent.map((id) => byId.get(id)).filter((a): a is StartApp => !!a).slice(0, 4);
  const pin = (id: string, on: boolean) => invoke("start_pin", { id, on }).then(loadInfo).catch(() => {});

  // Search in two steps, like Windows' Start: the instant part (Start Menu apps, Settings) on every
  // keystroke, then the Windows Search index (programs, folders, documents...) once typing pauses.
  // A late answer for an older query is dropped.
  const [finds, setFinds] = useState<Finds>(NONE);
  const asked = useRef(0);
  useEffect(() => {
    const t = q.trim();
    const me = ++asked.current;
    if (!t) return setFinds(NONE);
    invoke<Finds>("start_find", { q: t, index: false })
      .then((f) =>
        setFinds((prev) => {
          if (asked.current !== me) return prev;
          // Keep indexed programs that still match until the index answers, so they don't blink.
          const kept = prev.apps.filter((a) => a.place && rankName(a.name, t) !== null && !f.apps.some((b) => b.target === a.target));
          const files = prev.files.filter((x) => x.name.toLowerCase().includes(t.toLowerCase()));
          return { ...f, apps: [...f.apps, ...kept], files };
        }),
      )
      .catch(() => {});
    const timer = window.setTimeout(() => {
      invoke<Finds>("start_find", { q: t, index: true })
        .then((f) => asked.current === me && setFinds(f))
        .catch(() => {});
    }, 120);
    return () => clearTimeout(timer);
  }, [q]);

  // One flat list in Windows' groups, so the arrow keys and Enter walk it top to bottom.
  const hits: Hit[] = useMemo(() => {
    const t = q.trim();
    if (!t) return [];
    const toHit = (f: Found, group: string): Hit => ({
      key: `${f.kind}:${f.target}`,
      group,
      label: f.name,
      sub: f.kind === "app" ? (f.place ? `App · ${shortPlace(f.place)}` : "App") : f.kind === "setting" ? "Settings" : shortPlace(f.place),
      icon: f.kind === "setting" ? null : f.target,
      target: f.kind === "setting" ? null : f.target,
      run: (admin) => open(f.target, admin),
    });
    // Until Windows' Settings index is built (once, a second or so), the short built-in list.
    const settings: Found[] = finds.settings_ready
      ? finds.settings
      : SETTINGS_PAGES.map((p) => [rankName(`${p.name} ${p.words}`, t), p] as const)
          .filter(([r]) => r !== null)
          .sort((x, y) => x[0]! - y[0]!)
          .slice(0, 4)
          .map(([, p]) => ({ name: p.name, target: p.uri, kind: "setting", place: null }));
    // A downloaded program (kind "program", listed with the files) is never the best match.
    const all = [...finds.apps, ...settings, ...finds.files.filter((f) => f.kind !== "program")];
    const best = all[0];
    const out: Hit[] = best ? [toHit(best, "Best match")] : [];
    const rest = (list: Found[], group: string) => list.filter((f) => f !== best).forEach((f) => out.push(toHit(f, group)));
    rest(finds.apps, "Apps");
    rest(settings, "Settings");
    for (const [kind, group] of FILE_GROUPS) rest(finds.files.filter((f) => f.kind === kind).slice(0, 4), group);
    const tail = (key: string, label: string, sub: string, run: () => void): Hit => ({ key, group: "Search", label, sub, icon: null, target: null, run });
    out.push(tail("files", `Search files for "${t}"`, "File Explorer", () => invoke("start_search", { q: t, web: false })));
    out.push(tail("web", `Search the web for "${t}"`, "Browser", () => invoke("start_search", { q: t, web: true })));
    return out;
  }, [q, finds]);
  useEffect(() => setSel(0), [q]);

  // The order shown while a pin is dragged; it follows the saved list again after each reload.
  const [order, setOrder] = useState(pinned);
  const pinKey = pinned.map((a) => a.id).join("|");
  useEffect(() => setOrder(pinned), [pinKey]); // eslint-disable-line react-hooks/exhaustive-deps
  const { dragging, item } = useReorder("startpins", order, setOrder, (final) => {
    invoke("start_pins_order", { ids: final.map((a) => a.id) }).then(loadInfo).catch(() => {});
  });

  // All apps A-Z, grouped by first letter; the letter row jumps to a group.
  const groups = useMemo(() => {
    const g = new Map<string, StartApp[]>();
    for (const a of apps) {
      const c = a.name[0]?.toUpperCase() ?? "#";
      const k = /[A-Z]/.test(c) ? c : "#";
      g.set(k, [...(g.get(k) ?? []), a]);
    }
    return [...g.entries()];
  }, [apps]);
  const list = useRef<HTMLDivElement>(null);

  const appButton = (a: StartApp, cls: string, extra?: Record<string, unknown>) => (
    <button
      key={a.id}
      className={cls}
      title={a.name}
      onClick={() => open(a.id)}
      onContextMenu={(e) => {
        e.preventDefault();
        pin(a.id, !info.pins.includes(a.id));
      }}
      {...extra}
    >
      <AppIcon src={a.id} name={a.name} />
      <span>{a.name}</span>
    </button>
  );

  const { body, button: customize } = useSections(
    "start",
    [
      {
        id: "pinned",
        name: "Pinned",
        hint: "right-click any app to pin or unpin it; drag to reorder",
        node: (
          <section className="stsec">
            <h4>Pinned</h4>
            {pinned.length ? (
              <div className="stgrid">
                {order.map((a, i) => appButton(a, `stapp${dragging === i ? " dragging" : ""}`, { ...item(i), "data-grip": "" }))}
              </div>
            ) : (
              <p className="empty">Right-click an app below to pin it here.</p>
            )}
          </section>
        ),
      },
      {
        id: "recent",
        name: "Recent",
        hint: "apps opened from here and recent files",
        node:
          recentApps.length || files.length ? (
            <section className="stsec">
              <h4>Recent</h4>
              <div className="strecent">
                {recentApps.map((a) => appButton(a, "strow"))}
                {files.slice(0, 4).map((f) => (
                  <button key={f.path} className="strow" title={f.name} onClick={() => open(f.path)}>
                    <span className="stfile" aria-hidden="true">{f.dir ? "\u{1F4C1}" : "\u{1F4C4}"}</span>
                    <span>{f.name.replace(/\.lnk$/i, "")}</span>
                  </button>
                ))}
              </div>
            </section>
          ) : null,
      },
      {
        id: "all",
        name: "All apps",
        node: (
          <section className="stsec">
            <h4>All apps</h4>
            <div className="stletters" aria-label="Jump to letter">
              {groups.map(([k]) => (
                <button key={k} onClick={() => list.current?.querySelector(`[data-letter="${k}"]`)?.scrollIntoView({ block: "start" })}>
                  {k}
                </button>
              ))}
            </div>
            <div className="stall" ref={list}>
              {groups.map(([k, items]) => (
                <div key={k}>
                  <h5 data-letter={k}>{k}</h5>
                  {items.map((a) => appButton(a, "strow"))}
                </div>
              ))}
            </div>
          </section>
        ),
      },
    ],
    opened,
  );

  const [power, setPower] = useState(false);
  const [sure, setSure] = useState("");
  const act = (a: string, ask: boolean) => {
    if (ask && sure !== a) return setSure(a);
    invoke("power_action", { action: a }).catch(() => {});
  };

  return (
    <div className="view start">
      <input
        ref={box}
        className="psearch stsearch"
        placeholder="Search apps, settings and files"
        aria-label="Search apps, settings and files"
        value={q}
        onChange={(e) => setQ(e.target.value)}
        onKeyDown={(e) => {
          if (!hits.length) return;
          if (e.key === "ArrowDown") (e.preventDefault(), setSel((s) => Math.min(hits.length - 1, s + 1)));
          if (e.key === "ArrowUp") (e.preventDefault(), setSel((s) => Math.max(0, s - 1)));
          // Ctrl+Shift+Enter runs the app as administrator, as in Windows' Start.
          if (e.key === "Enter") hits[sel]?.run(e.ctrlKey && e.shiftKey);
        }}
      />
      <div className="stbody">
        {q.trim() ? (
          <div className="sthits" role="listbox" aria-label="Results">
            {hits.map((h, i) => (
              <div key={h.key} role="presentation">
                {(i === 0 || hits[i - 1].group !== h.group) && <h5 className="stgroup">{h.group}</h5>}
                <button
                  role="option"
                  aria-selected={i === sel}
                  className={`sthit${i === sel ? " on" : ""}${h.group === "Best match" ? " best" : ""}`}
                  title={h.target && !h.target.startsWith("shell:") ? `${h.target}\nRight-click: open file location · Ctrl+Shift+Enter: run as administrator` : h.label}
                  onMouseEnter={() => setSel(i)}
                  onClick={() => h.run()}
                  onContextMenu={(e) => {
                    // Right-click a file or program: open its location, as Windows' Start offers.
                    if (!h.target || h.target.startsWith("shell:")) return;
                    e.preventDefault();
                    invoke("start_reveal", { path: h.target }).catch(() => {});
                  }}
                >
                  {h.icon ? <AppIcon src={h.icon} name={h.label} /> : <span className="stfile" aria-hidden="true">{h.sub === "Settings" ? "⚙" : h.sub === "Browser" ? "\u{1F310}" : "\u{1F50D}"}</span>}
                  <span className="sthl">{h.label}</span>
                  <small>{h.sub}</small>
                </button>
              </div>
            ))}
          </div>
        ) : (
          body
        )}
      </div>
      <footer className="stfoot">
        <span className="stuser" title={info.user}>
          <i aria-hidden="true">{info.user.slice(0, 1).toUpperCase()}</i>
          {info.user}
        </span>
        <span className="sp" />
        {!q.trim() && customize}
        <button className="ghost" title="Windows' own Start" onClick={() => invoke("start_windows")}>
          Windows Start
        </button>
        <button className="stico" title="Settings" aria-label="Settings" onClick={() => open("ms-settings:")}>
          {"⚙"}
        </button>
        <button className="stico" title="File Explorer" aria-label="File Explorer" onClick={() => invoke("start_open", { target: "shell:AppsFolder\\Microsoft.Windows.Explorer" })}>
          {"\u{1F4C1}"}
        </button>
        <button className={`stico${power ? " on" : ""}`} title="Power" aria-label="Power" onClick={() => (setPower(!power), setSure(""))}>
          {"⏻"}
        </button>
        {power && (
          <div className="stpower" role="menu">
            <button role="menuitem" onClick={() => act("lock", false)}>Lock</button>
            <button role="menuitem" onClick={() => act("sleep", false)}>Sleep</button>
            <button role="menuitem" onClick={() => act("signout", true)}>{sure === "signout" ? "Sign out?" : "Sign out"}</button>
            <button role="menuitem" onClick={() => act("restart", true)}>{sure === "restart" ? "Restart now?" : "Restart"}</button>
            <button role="menuitem" onClick={() => act("shutdown", true)}>{sure === "shutdown" ? "Shut down now?" : "Shut down"}</button>
          </div>
        )}
      </footer>
    </div>
  );
}
