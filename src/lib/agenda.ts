import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import type { TodayView } from "./today.ts";

// Today's agenda (calendar links, local to-dos, and the MCP server if one is connected; see
// src-tauri/src/agenda.rs), read once and shared by every cell in a window.
const TTL = 60_000;
let cache: { at: number; p: Promise<TodayView> } | null = null;

/** The merged today view, at most a minute old. `fresh` skips the cache. */
export function todayView(fresh = false): Promise<TodayView> {
  if (!fresh && cache && Date.now() - cache.at < TTL) return cache.p;
  const p = invoke<TodayView>("agenda_today").then((v) => ({
    events: v.events ?? [],
    today: v.today ?? [],
    soon: v.soon ?? [],
    school: v.school ?? [],
    errors: v.errors ?? [],
  }));
  cache = { at: Date.now(), p };
  p.catch(() => {
    if (cache?.p === p) cache = null;
  });
  return p;
}

/** Forget the cached view (after adding or ticking a to-do), here and in every other window: each window
 *  keeps its own cache, so without the event the dock's to-do count stays old for minutes. */
export const invalidate = () => {
  cache = null;
  emit("agenda-changed").catch(() => {});
};

/** Calls `f` when another window changed the agenda (see `invalidate`). Returns an unsubscribe. */
export function onAgendaChanged(f: () => void): () => void {
  const off = listen("agenda-changed", () => {
    cache = null;
    f();
  });
  return () => {
    off.then((u) => u()).catch(() => {});
  };
}
