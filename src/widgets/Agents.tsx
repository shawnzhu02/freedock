import { useEffect, useState, type MouseEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { todayView } from "../lib/agenda";
import { nextMeeting } from "../lib/today";
import { useNow } from "./useNow";
import type { WidgetProps } from "./registry";
import { pollActive } from "../lib/active";

/** Open the agents' popup centred on the clicked cell. */
export function openAt(mode: string, e: MouseEvent<HTMLElement>, vertical: boolean) {
  const r = e.currentTarget.getBoundingClientRect();
  invoke("popup_open", { mode, anchor: vertical ? r.top + r.height / 2 : r.left + r.width / 2 });
}

const vertical = (s: WidgetProps["settings"]) => s.edge === "left" || s.edge === "right";

/** Quick Ask: a one-off Claude conversation that isn't saved. */
export function AskCell({ settings }: WidgetProps) {
  return (
    <button className="cell agent" title={`Quick Ask (${settings.ask_hotkey})`} onClick={(e) => openAt("ask", e, vertical(settings))}>
      <svg viewBox="0 0 20 20" width="19" height="19" aria-hidden="true">
        <path d="M10 2.5l1.6 4.4 4.4 1.6-4.4 1.6L10 14.5l-1.6-4.4L4 8.5l4.4-1.6z" fill="currentColor" />
        <path d="M15.5 12.5l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7z" fill="currentColor" opacity=".7" />
      </svg>
    </button>
  );
}

/** Start Claude Code in a recent project. */
export function ClaudeCell({ settings }: WidgetProps) {
  return (
    <button className="cell agent" title="New Claude session" onClick={(e) => openAt("claude", e, vertical(settings))}>
      <svg viewBox="0 0 20 20" width="19" height="19" aria-hidden="true">
        <rect x="2" y="3.5" width="16" height="13" rx="3" fill="none" stroke="currentColor" strokeWidth="1.6" />
        <path d="M5.5 8l2.5 2-2.5 2M10 12.5h4" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </button>
  );
}

/** Agenda: what's next today from your calendars; a click opens the calendar with today's events. */
export function AgendaCell({ settings }: WidgetProps) {
  const now = useNow();
  const [events, setEvents] = useState<Parameters<typeof nextMeeting>[0] | undefined>(undefined);
  useEffect(() => {
    const get = () =>
      todayView()
        .then((v) => setEvents(v.events))
        .catch(() => setEvents(undefined));
    get();
    const stop = pollActive(5 * 60_000, () => get());
    return () => stop();
  }, []);
  const v = vertical(settings);
  const next = events ? nextMeeting(events, now) : null;
  const label = events === undefined ? "Agenda" : next ? `${next.event.time} · ${next.event.title}` : "Nothing else today";
  return (
    <button className="cell agenda" title={next?.event.place ? `${label} · ${next.event.place}` : label} onClick={(e) => openAt("calendar", e, v)}>
      <small>Next</small>
      {!v && <b>{label}</b>}
    </button>
  );
}

/** Agent: the connected MCP server's flyout (what's next, add a to-do, search). */
export function AgentCell({ settings }: WidgetProps) {
  const v = vertical(settings);
  return (
    <button className="cell agenda" title="Your MCP agent: what's next, add a to-do, search" onClick={(e) => openAt("agent", e, v)}>
      <small>Agent</small>
      {!v && <b>Ask · Add · Search</b>}
    </button>
  );
}
