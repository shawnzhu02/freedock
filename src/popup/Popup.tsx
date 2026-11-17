import type React from "react";
import { useKept } from "../lib/kept";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Md from "./md";
import Quick from "./Quick";
import Calendar from "./Calendar";
import Media from "./Media";
import UsageView from "./UsageView";
import StatsView from "./StatsView";
import { TransitView } from "../widgets/Transit";
import { STOPS } from "../lib/mbta";
import WeatherView from "./WeatherView";
import ProjectsView from "./ProjectsView";
import ClipboardView from "./ClipboardView";
import FocusView from "./FocusView";
import CaptureView from "./CaptureView";
import FilesView from "./FilesView";
import NotesView from "./NotesView";
import GitHubView from "./GitHubView";
import DockerView from "./DockerView";
import PowerView from "./PowerView";
import TodosView from "./TodosView";
import SitesView from "./SitesView";
import CalcView from "./CalcView";
import MoodsView from "./MoodsView";
import AppMenu from "./AppMenu";
import Thumbs from "./Thumbs";
import StartView from "./StartView";
import SessionsView from "./SessionsView";
import ScreenTimeView from "./ScreenTimeView";
import { docOf } from "./mcpDoc";
import type { TrayIcon } from "../widgets/Tray";
import GlassCard from "../widgets/GlassCard";
import { CurrencyView, NewsView, StocksView } from "./EverydayViews";
import { getSettings, onSettings, type Settings } from "../settings";

type Mode = { mode: "ask" | "claude" | "agent" | "tray" | "quick" | "calendar" | "media" | "usage" | "stats" | "weather" | "projects" | "clipboard" | "focus" | "capture" | "downloads" | "screenshots" | "recent" | "notes" | "github" | "screentime" | "docker" | "power" | "todos" | "sites" | "calc" | "sessions" | "transit" | "moods" | "appmenu" | "thumbs" | "start" | "stocks" | "currency" | "news"; prefill: string };

/** Flyouts that size the window to their content (the rest have a fixed size per mode). */
const FIT = ["tray", "quick", "calendar", "media", "usage", "stats", "weather", "projects", "clipboard", "focus", "capture", "downloads", "screenshots", "recent", "github", "screentime", "docker", "power", "todos", "sites", "calc", "sessions", "transit", "moods", "appmenu", "stocks", "currency", "news"];

/** Measures its content and asks for the window to be exactly that tall. */
/** When a flyout's height last changed (performance.now()): it is shown only once this has settled. */
let lastResize = 0;


/** One step of the open path, to popup.log (diagnostics for flyouts that don't appear). */
const trace = (msg: string) => invoke("popup_trace", { msg: `${msg} vis=${document.visibilityState}` }).catch(() => {});

function Fit({ children, opened, active }: { children: React.ReactNode; opened: number; active: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  const last = useRef(0);
  // Closed (parked): the view is emptied, and that must not shrink the window on its way out.
  const on = useRef(active);
  on.current = active;
  const send = () => {
    const el = ref.current;
    if (!el || !on.current) return;
    // +2 for the card's border. offsetHeight is the laid-out height, unaffected by the open animation's
    // scale (getBoundingClientRect would report the shrunken size mid-animation).
    const h = el.offsetHeight + 2;
    if (Math.abs(h - last.current) < 2) return;
    last.current = h;
    lastResize = performance.now();
    invoke("popup_fit", { height: h });
  };
  useLayoutEffect(() => {
    const ro = new ResizeObserver(send);
    ro.observe(ref.current!);
    return () => ro.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  // Every open first sizes the window for its mode, so even an unchanged height must be sent again.
  useLayoutEffect(() => {
    last.current = 0;
    send();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [opened]);
  return (
    <div className="fit" ref={ref}>
      {children}
    </div>
  );
}

/** The flyout window: Quick Ask, the Claude session launcher, the MCP agent, or a widget's flyout. Esc closes it. */
export default function Popup() {
  const [mode, setMode] = useState<Mode | null>(null);
  // Bumped on every open, so views can focus their input and refresh.
  const [opened, setOpened] = useState(0);
  const [parked, setParked] = useState(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  // The open animation: "pre" while the window is still invisible, "in" once it is revealed. Only an
  // open from hidden animates; moving from one app's preview to another keeps the card as it is.
  const [enter, setEnter] = useState<"pre" | "in">("in");
  const wasHidden = useRef(true);
  useEffect(() => {
    getSettings().then(setSettings);
    const offS = onSettings(setSettings);
    // A flyout opened before this page was listening: treat it as an open, so it reports ready and
    // Quick Ask takes its prefill.
    invoke<Mode | null>("popup_state").then((m) => {
      if (m) {
        setMode(m);
        setOpened((n) => n + 1);
      }
    });
    const off = listen<Mode>("popup-mode", (e) => {
      setMode(e.payload);
      setParked(false);
      if (wasHidden.current) setEnter("pre");
      wasHidden.current = false;
      setOpened((n) => n + 1);
    });
    // Hidden: unmount every view but Quick Ask, so nothing keeps polling behind a closed flyout.
    // Rust revealed the window (normally right after popup_ready; after 300 ms if the page was slow).
    const offRevealed = listen("popup-revealed", () => setEnter("in"));
    const offHide = listen("popup-hidden", () => {
      setParked(true);
      wasHidden.current = true;
    });
    const esc = (e: KeyboardEvent) => e.key === "Escape" && invoke("popup_close");
    window.addEventListener("keydown", esc);
    return () => {
      off.then((f) => f());
      offHide.then((f) => f());
      offRevealed.then((f) => f());
      offS.then((f) => f());
      window.removeEventListener("keydown", esc);
    };
  }, []);
  // Rust keeps the window invisible (cloaked) until the new flyout has painted, so the old flyout's
  // last frame never flashes. It waits at least two frames (lay out, then paint), then until its
  // height has stayed put for two frames (at most 260 ms), so a view still filling in grows unseen.
  // Rust shows it anyway after 300 ms.
  useEffect(() => {
    if (!opened) return;
    const start = performance.now();
    let frames = 0;
    let raf = 0;
    const tick = () => {
      const now = performance.now();
      const settled = now - lastResize >= 34;
      if (++frames >= 2 && (settled || now - start >= 260)) {
        trace(`ready +${Math.round(now - start)} frames=${frames}`);
        invoke("popup_ready").catch(() => {});
        setEnter("in");
        return;
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [opened, settings]);
  if (!mode || !settings) return null;
  const fits = FIT.includes(mode.mode);
  const m = parked ? "" : mode.mode;
  return (
    <GlassCard className={`card${fits ? " fits" : ""}`} settings={settings} window="ask" enter={enter}>
      {/* Kept mounted while hidden, so a Quick Ask conversation survives switching to another view and back. */}
      <div hidden={m !== "ask"} className="view">
        <Ask prefill={m === "ask" ? mode.prefill : ""} opened={m === "ask" ? opened : 0} />
      </div>
      {m === "claude" && <Claude opened={opened} />}
      {m === "agent" && <Agent opened={opened} />}
      {m === "notes" && <NotesView opened={opened} />}
      {m === "thumbs" && <Thumbs appKey={mode.prefill} opened={opened} fade={settings.popup_motion !== "off"} />}
      {m === "start" && <StartView opened={opened} />}
      {fits && (
        <Fit key={mode.mode} opened={opened} active={!parked}>
          {m === "tray" && <Tray opened={opened} />}
          {m === "quick" && <Quick opened={opened} />}
          {m === "calendar" && <Calendar opened={opened} />}
          {m === "media" && <Media opened={opened} />}
          {m === "usage" && <UsageView opened={opened} />}
          {m === "stats" && <StatsView opened={opened} />}
          {m === "weather" && <WeatherView settings={settings} opened={opened} />}
          {m === "projects" && <ProjectsView opened={opened} />}
          {m === "clipboard" && <ClipboardView opened={opened} />}
          {m === "focus" && <FocusView opened={opened} />}
          {m === "capture" && <CaptureView opened={opened} />}
          {(m === "downloads" || m === "screenshots" || m === "recent") && <FilesView kind={m} opened={opened} />}
          {m === "github" && <GitHubView opened={opened} />}
          {m === "docker" && <DockerView opened={opened} />}
          {m === "power" && <PowerView opened={opened} />}
          {m === "todos" && <TodosView opened={opened} />}
          {m === "sites" && <SitesView opened={opened} />}
          {m === "stocks" && <StocksView opened={opened} />}
          {m === "currency" && <CurrencyView opened={opened} />}
          {m === "news" && <NewsView opened={opened} />}
          {m === "calc" && <CalcView opened={opened} />}
          {m === "moods" && <MoodsView settings={settings} />}
          {m === "appmenu" && <AppMenu appKey={mode.prefill} opened={opened} />}
          {m === "sessions" && <SessionsView opened={opened} />}
          {m === "screentime" && <ScreenTimeView opened={opened} />}
          {m === "transit" && (() => {
            const stop = String(settings.widgets.find((w) => w.id === "transit")?.options?.stop || "place-pktrm");
            return <TransitView stop={stop} name={STOPS.find(([k]) => k === stop)?.[1] ?? stop} opened={opened} />;
          })()}
        </Fit>
      )}
    </GlassCard>
  );
}

// ------------------------------------------------------------------------------------------- Quick Ask

type Turn = { role: "you" | "claude"; text: string; tools: string[]; done: boolean; image?: string };

// One-click questions for a selection or a screenshot: [label, question].
const TEXT_PRESETS: [string, string][] = [
  ["Explain", "Explain this."],
  ["Summarize", "Summarize this in a few bullet points."],
  ["Fix the writing", "Fix the spelling, grammar and clarity of this text. Reply with only the corrected text."],
  ["Draft a reply", "Draft a short, friendly reply to this message in my voice. Reply with only the draft."],
  ["To English", "Translate this into English. Reply with only the translation."],
];
const IMAGE_PRESETS: [string, string][] = [
  ["Describe", "What's in this screenshot?"],
  ["Copy the text", "Transcribe all the text in this screenshot exactly. Reply with only the text."],
  ["What's wrong?", "This screenshot shows an error or a problem. What is going wrong, and how do I fix it?"],
];

function Ask({ prefill, opened }: { prefill: string; opened: number }) {
  const [turns, setTurns] = useState<Turn[]>([]);
  const [input, setInput] = useState("");
  const [selection, setSelection] = useState("");
  // A snip waiting to go with the next message (the PNG itself stays in Rust; this is the preview).
  const [image, setImage] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState("");
  const box = useRef<HTMLTextAreaElement>(null);
  const scroller = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!opened) return;
    if (prefill) setSelection(prefill);
    // A snip may be waiting (this view can mount after the snip's nudge was sent).
    invoke<string | null>("snip_preview").then((url) => url && setImage(url));
    setTimeout(() => box.current?.focus(), 30);
  }, [opened, prefill]);

  // Claude Code's stream-json events: text arrives in deltas; tool calls show as chips.
  const onLine = useCallback((line: string) => {
    let o: any;
    try {
      o = JSON.parse(line);
    } catch {
      return;
    }
    const patchLast = (f: (t: Turn) => Turn) =>
      setTurns((ts) => {
        const last = ts[ts.length - 1];
        if (!last || last.role !== "claude" || last.done) return ts;
        return [...ts.slice(0, -1), f(last)];
      });
    if (o.type === "stream_event") {
      const ev = o.event;
      if (ev?.type === "content_block_delta" && ev.delta?.type === "text_delta") patchLast((t) => ({ ...t, text: t.text + ev.delta.text }));
      if (ev?.type === "content_block_start" && ev.content_block?.type === "tool_use") {
        const name = String(ev.content_block.name ?? "tool").replace(/^mcp__[^_]+__/, "").replace(/^mcp__/, "");
        patchLast((t) => ({ ...t, tools: [...t.tools, name] }));
      }
      // A new message after a tool call: keep the text apart from what came before.
      if (ev?.type === "message_start") patchLast((t) => (t.text && !t.text.endsWith("\n\n") ? { ...t, text: t.text + "\n\n" } : t));
    } else if (o.type === "result") {
      patchLast((t) => ({ ...t, done: true, text: t.text.trim() || (o.is_error ? `Error: ${o.result ?? "failed"}` : t.text) }));
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    const offs = [
      listen<string>("ask-line", (e) => onLine(e.payload)),
      listen<string>("ask-err", (e) => setNote(e.payload.slice(0, 300))),
      listen("ask-image", () =>
        invoke<string | null>("snip_preview").then((url) => {
          setImage(url ?? "");
          setTimeout(() => box.current?.focus(), 30);
        }),
      ),
      listen("ask-exit", () => {
        setBusy(false);
        setNote("Claude stopped. Your next question starts a new conversation.");
        setTurns((ts) => ts.map((t) => ({ ...t, done: true })));
      }),
    ];
    return () => offs.forEach((o) => o.then((f) => f()));
  }, [onLine]);

  useLayoutEffect(() => {
    scroller.current?.scrollTo({ top: scroller.current.scrollHeight });
  }, [turns]);

  const send = (preset?: string, label?: string) => {
    const q = (preset ?? input).trim();
    if (busy || (!q && !selection && !image)) return;
    // The selection came from another app (maybe a web page): marked as data, not instructions. The tag
    // carries a random id per message, so text inside can't close the block by writing the tag itself.
    const id = Array.from(crypto.getRandomValues(new Uint8Array(8)), (b) => b.toString(16).padStart(2, "0")).join("");
    const fallback = image && !selection ? "What's in this screenshot?" : "Explain this.";
    const text = selection ? `<selected_text_${id}>\n${selection}\n</selected_text_${id}>\n\n${q || fallback}` : q || fallback;
    const withImage = !!image;
    setTurns((ts) => [...ts, { role: "you", text: label ?? (q || fallback), tools: [], done: true, image }, { role: "claude", text: "", tools: [], done: false }]);
    setInput("");
    setSelection("");
    setImage("");
    setNote("");
    setBusy(true);
    invoke("ask_send", { text, image: withImage }).catch((e) => {
      setBusy(false);
      setNote(String(e));
      setTurns((ts) => ts.map((t) => ({ ...t, done: true })));
    });
  };
  const reset = () => {
    invoke("ask_reset");
    setTurns([]);
    setBusy(false);
    setNote("");
    setSelection("");
    setImage("");
    invoke("snip_clear");
    box.current?.focus();
  };
  const lastAnswer = [...turns].reverse().find((t) => t.role === "claude" && t.text)?.text;

  return (
    <>
      <header>
        <b>Quick Ask</b>
        <span className="sub">nothing is saved</span>
        <span className="sp" />
        {lastAnswer && (
          <button className="ghost" onClick={() => navigator.clipboard.writeText(lastAnswer)}>
            Copy
          </button>
        )}
        <button className="ghost" onClick={reset} disabled={!turns.length && !selection && !image}>
          New
        </button>
      </header>
      <div className="scroll" ref={scroller}>
        {!turns.length && <p className="empty">Ask anything. Claude can use your tools and MCP servers. Ctrl+Alt+A asks about selected text.</p>}
        {turns.map((t, i) =>
          t.role === "you" ? (
            <div key={i} className="you">
              {t.image && <img className="youimg" src={t.image} alt="Screenshot" />}
              {t.text}
            </div>
          ) : (
            <div key={i} className="answer">
              {t.tools.length > 0 && (
                <div className="tools">
                  {t.tools.map((n, j) => (
                    <span key={j}>{n}</span>
                  ))}
                </div>
              )}
              {t.text ? <Md text={t.text} /> : !t.done && <span className="dots" />}
            </div>
          ),
        )}
        {note && <p className="note">{note}</p>}
      </div>
      {image && (
        <div className="sel simg">
          <img src={image} alt="Snip" />
          <span>Screenshot attached</span>
          <button
            className="x"
            title="Drop the screenshot"
            onClick={() => {
              setImage("");
              invoke("snip_clear");
            }}
          >
            ×
          </button>
        </div>
      )}
      {selection && (
        <div className="sel">
          <span>{selection.length > 160 ? selection.slice(0, 160) + "…" : selection}</span>
          <button className="x" title="Drop the selection" onClick={() => setSelection("")}>
            ×
          </button>
        </div>
      )}
      {(selection || image) && !input && !busy && (
        <div className="presets">
          {(image ? IMAGE_PRESETS : TEXT_PRESETS).map(([label, q]) => (
            <button key={label} className="chip" onClick={() => send(q, label)}>
              {label}
            </button>
          ))}
        </div>
      )}
      <div className="compose">
        <textarea
          ref={box}
          rows={2}
          value={input}
          placeholder={image ? "Ask about the screenshot (Enter = describe it)" : selection ? "Ask about the selection (Enter = explain it)" : turns.length ? "Follow up…" : "Ask Claude…"}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send();
            }
          }}
        />
        <button className="go" onClick={() => send()} disabled={busy}>
          {busy ? "…" : "Ask"}
        </button>
      </div>
    </>
  );
}

// -------------------------------------------------------------------------------------- Claude launcher

type Project = { dir: string; name: string; ago: number };

const ago = (s: number) =>
  s < 3600 ? `${Math.max(1, Math.round(s / 60))} min` : s < 86400 ? `${Math.round(s / 3600)} h` : `${Math.round(s / 86400)} d`;

// ------------------------------------------------------------------------------------ Background apps

/** The tray's icons, five to a row. Clicks go to the apps the way the taskbar would send them. */
function Tray({ opened }: { opened: number }) {
  const [icons, setIcons] = useKept<TrayIcon[] | null>("tray.icons", null);
  useEffect(() => {
    const load = () => invoke<TrayIcon[]>("tray_list").then(setIcons).catch(() => setIcons([]));
    load();
    const off = listen("tray-changed", load);
    return () => {
      off.then((f) => f());
    };
  }, [opened]);
  // A single click waits briefly so a double click isn't also sent as a click.
  const clickTimer = useRef(0);
  const act = (id: string, action: "left" | "right" | "double" | "middle") => invoke("tray_action", { id, action });
  if (icons && !icons.length) return <p className="empty trayempty">No background apps</p>;
  return (
    <div className="trayg">
      {(icons ?? []).map((i) => (
        <button
          key={i.id}
          title={i.tooltip}
          onClick={() => {
            clearTimeout(clickTimer.current);
            clickTimer.current = window.setTimeout(() => act(i.id, "left"), 220);
          }}
          onDoubleClick={() => {
            clearTimeout(clickTimer.current);
            act(i.id, "double");
          }}
          onContextMenu={(e) => {
            e.preventDefault();
            act(i.id, "right");
          }}
          onAuxClick={(e) => e.button === 1 && act(i.id, "middle")}
        >
          {i.icon ? <img src={i.icon} alt="" draggable={false} /> : <span>{i.tooltip.slice(0, 1)}</span>}
        </button>
      ))}
    </div>
  );
}

function Claude({ opened }: { opened: number }) {
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [q, setQ] = useState("");
  const [err, setErr] = useState("");
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    setQ("");
    setErr("");
    invoke<Project[]>("claude_projects").then(setProjects).catch(() => setProjects([]));
    setTimeout(() => input.current?.focus(), 30);
  }, [opened]);
  const shown = (projects ?? []).filter((p) => !q || (p.name + p.dir).toLowerCase().includes(q.toLowerCase()));
  const go = (dir: string) => invoke("claude_launch", { dir }).catch((e) => setErr(String(e)));
  return (
    <div className="view">
      <header>
        <b>New Claude session</b>
      </header>
      <div className="search">
        <input
          ref={input}
          value={q}
          placeholder="Filter, or paste a folder path"
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.key !== "Enter") return;
            if (/^[a-zA-Z]:\\/.test(q.trim())) go(q.trim());
            else if (shown[0]) go(shown[0].dir);
          }}
        />
      </div>
      <div className="scroll list">
        {projects === null && <p className="empty">Loading…</p>}
        {shown.map((p) => (
          <button key={p.dir} className="item" onClick={() => go(p.dir)} title={p.dir}>
            <b>{p.name}</b>
            <small>{p.dir}</small>
            <i>{ago(p.ago)}</i>
          </button>
        ))}
        {projects !== null && !shown.length && <p className="empty">No match. Paste a full folder path and press Enter.</p>}
        {err && <p className="note">{err}</p>}
      </div>
    </div>
  );
}

// ----------------------------------------------------------------------------------------- Agent (MCP)

export { docOf };

/** The connected MCP server: its `now` summary, add a to-do, search its notes (docs/MCP_AGENDA.md). */
function Agent({ opened }: { opened: number }) {
  const [name, setName] = useState<string | null | undefined>(undefined);
  const [doc, setDoc] = useState("");
  const [err, setErr] = useState("");
  const [todo, setTodo] = useState("");
  const [query, setQuery] = useState("");
  const [found, setFound] = useState("");
  const [flash, setFlash] = useState("");
  const [busy, setBusy] = useState(false);
  const todoBox = useRef<HTMLInputElement>(null);

  const load = useCallback(() => {
    setErr("");
    invoke<{ mcp: string | null }>("agenda_status").then((s) => setName(s.mcp)).catch(() => setName(null));
    invoke<string>("mcp_call", { tool: "now", args: {} })
      .then((t) => setDoc(docOf(t).replace(/^# Now\s*/, "")))
      .catch((e) => setErr(String(e)));
  }, []);
  useEffect(() => {
    load();
    setFound("");
    setFlash("");
    setTimeout(() => todoBox.current?.focus(), 30);
  }, [opened, load]);

  const add = () => {
    const item = todo.trim();
    if (!item) return;
    setBusy(true);
    invoke<string>("mcp_call", { tool: "add_todo", args: { name: item } })
      .then(() => {
        setTodo("");
        setFlash(`Added: ${item}`);
        load();
      })
      .catch((e) => setFlash(String(e)))
      .finally(() => setBusy(false));
  };
  const search = () => {
    const q = query.trim();
    if (!q) return;
    setBusy(true);
    invoke<string>("mcp_call", { tool: "search_memory", args: { query: q, limit: 6 } })
      .then((t) => setFound(docOf(t)))
      .catch((e) => setFound(String(e)))
      .finally(() => setBusy(false));
  };

  return (
    <div className="view">
      <header>
        <b>{name || "Agent"}</b>
        <span className="sp" />
        <button className="ghost" onClick={load}>
          Refresh
        </button>
        <button className="ghost" onClick={() => invoke("popup_open", { mode: "ask", anchor: null })}>
          Ask…
        </button>
      </header>
      <div className="row2">
        <input
          ref={todoBox}
          value={todo}
          placeholder="Add a to-do (Enter)"
          disabled={busy}
          onChange={(e) => setTodo(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
        />
        <input
          value={query}
          placeholder="Search (Enter)"
          disabled={busy}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && search()}
        />
      </div>
      {flash && <p className="note ok">{flash}</p>}
      <div className="scroll">
        {found && (
          <div className="found">
            <button className="x" title="Close results" onClick={() => setFound("")}>
              ×
            </button>
            <Md text={found} />
          </div>
        )}
        {name === null ? (
          <p className="note">No MCP server is connected. Connect one in Settings, Agenda.</p>
        ) : err ? (
          <p className="note">{err}</p>
        ) : doc ? (
          <Md text={doc} />
        ) : (
          <span className="dots" />
        )}
      </div>
    </div>
  );
}
