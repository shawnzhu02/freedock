import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { useNow } from "./useNow";
import { todayView } from "../lib/agenda";
import { countdown, joinLink, nextMeeting, startingNow, type TodayEvent } from "../lib/today";
import { pollActive } from "../lib/active";

// Meetings whose start already paused the music (module-level so a re-mount doesn't pause twice).
const paused = new Set<string>();

/** The next meeting today with a countdown, and Join once it's close. A click opens the calendar. */
export default function Meeting({ settings, options }: WidgetProps) {
  const now = useNow();
  const [events, setEvents] = useState<TodayEvent[] | null>(null);
  useEffect(() => {
    const get = () => todayView().then((v) => setEvents(v.events)).catch(() => setEvents(null));
    get();
    const stop = pollActive(5 * 60_000, () => get());
    return () => stop();
  }, []);
  const vertical = settings.edge === "left" || settings.edge === "right";
  const length = Number(options.length ?? 60);
  const next = events ? nextMeeting(events, now, length) : null;
  const startKey = options.pauseMedia ? startingNow(next, now) : null;
  useEffect(() => {
    if (!startKey || paused.has(startKey)) return;
    paused.add(startKey);
    invoke<{ playing: boolean } | null>("media_now")
      .then((m) => m?.playing && invoke("media_control", { action: "pause" }))
      .catch(() => {});
  }, [startKey]);
  if (!next) {
    if (options.hideWhenFree !== false) return null;
    return (
      <button className="cell meeting" onClick={(e) => openAt("calendar", e, vertical)} title="No more meetings today">
        <small>Meetings</small>
        {!vertical && <b>Free</b>}
      </button>
    );
  }
  const link = joinLink(next.event.place);
  const showJoin = link && next.state !== "later";
  return (
    <div className={`cell group meeting ${next.state}`}>
      <button className="mmain" onClick={(e) => openAt("calendar", e, vertical)} title={`${next.event.time} · ${next.event.title}${next.event.place ? ` · ${next.event.place}` : ""}`}>
        <small>{countdown(next)}</small>
        {!vertical && <b>{next.event.title}</b>}
      </button>
      {showJoin && (
        <button className="join pill" title={`Join (${link})`} onClick={() => invoke("open_link", { url: link })}>
          Join
        </button>
      )}
    </div>
  );
}
