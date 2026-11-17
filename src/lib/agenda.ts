import { invoke } from "@tauri-apps/api/core";
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

/** Forget the cached view (after adding or ticking a to-do). */
export const invalidate = () => {
  cache = null;
};
