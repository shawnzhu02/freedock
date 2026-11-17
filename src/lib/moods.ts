// Moods (saved dock layouts) as the UI edits them. Switching itself happens in Rust (set_mood), which
// stores the current widgets and look back into the mood being left; see src-tauri/src/settings.rs.
import type { Look, Mood, Settings, WidgetSlot } from "../settings";

/** React keys for the dock's cells: a widget id plus which copy of it this is. A widget that two
 *  moods share keeps its key (and so its state) when switching, wherever it sits in the row. */
export function slotKeys(widgets: WidgetSlot[]): string[] {
  const seen = new Map<string, number>();
  return widgets.map((w) => {
    const n = seen.get(w.id) ?? 0;
    seen.set(w.id, n + 1);
    return `${w.id}#${n}`;
  });
}

export const lookOf = (s: Settings): Look => ({ style: s.style, glass_live: s.glass_live, edge: s.edge, visibility: s.visibility });

/** A new mood copying the current widgets, with an id no other mood has. */
export function newMood(s: Settings, name = "New mood"): Mood {
  let n = s.moods.length + 1;
  while (s.moods.some((m) => m.id === `mood-${n}`)) n++;
  return { id: `mood-${n}`, name, icon: "\u2728", widgets: s.widgets.map((w) => ({ ...w })), look: null };
}

/** The settings patch that gives the active mood its own look (on) or puts it back on the shared
 *  look (off, which also brings the shared style, glass, edge and visibility back). */
export function ownLook(s: Settings, on: boolean): Partial<Settings> {
  const moods = s.moods.map((m) => (m.id === s.active_mood ? { ...m, look: on ? lookOf(s) : null } : m));
  if (on) return { moods, shared_look: s.shared_look ?? lookOf(s) };
  return { moods, ...(s.shared_look ?? {}) };
}

/** Remove a mood; the active one can't be removed (switch away first), nor the last one. */
export function removeMood(s: Settings, id: string): Partial<Settings> | null {
  if (id === s.active_mood || s.moods.length < 2) return null;
  return { moods: s.moods.filter((m) => m.id !== id) };
}

export const patchMood = (s: Settings, id: string, p: Partial<Mood>): Partial<Settings> => ({
  moods: s.moods.map((m) => (m.id === id ? { ...m, ...p } : m)),
});

export const activeMood = (s: Settings): Mood | undefined => s.moods.find((m) => m.id === s.active_mood);
