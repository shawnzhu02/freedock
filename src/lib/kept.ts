// State that survives a flyout closing.
//
// Closing a flyout unmounts its view, so every open used to start from empty state and fill in piece
// by piece as each command answered: tiles popped in one by one and the window resized for each one,
// which flashed. `useKept` is a drop-in for `useState` whose value is kept in memory under a key: the
// next open draws the last known state at once, and a fresh value that equals the kept one changes
// nothing (no re-render, no resize).
//
//   const [audio, setAudio] = useKept<Audio | null>("quick.audio", null);
//
// Keys are global: prefix them with the view's name. Values must be JSON-like (they are compared by
// their JSON).
import { useCallback, useState } from "react";

const kept = new Map<string, unknown>();

/** The kept value for `key`, or `initial` if there is none yet. */
export function keptGet<T>(key: string, initial: T): T {
  return kept.has(key) ? (kept.get(key) as T) : initial;
}

/** Same data? Compared by JSON, so a freshly fetched copy of unchanged data counts as the same. */
export function same(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true;
  try {
    return JSON.stringify(a) === JSON.stringify(b);
  } catch {
    return false;
  }
}

/** Keeps `next` for `key` unless it equals `prev`. Returns what the state should now be: `prev` itself
 * when nothing changed, so React skips the render. */
export function keptSet<T>(key: string, prev: T, next: T): T {
  if (same(prev, next)) return prev;
  kept.set(key, next);
  return next;
}

/** Forgets every kept value (tests). */
export function keptClear() {
  kept.clear();
}

export function useKept<T>(key: string, initial: T): [T, (next: T | ((prev: T) => T)) => void] {
  const [value, setValue] = useState<T>(() => keptGet(key, initial));
  // A new key (other arguments) starts from what's kept for that key, not the old key's value.
  const [shownKey, setShownKey] = useState(key);
  if (shownKey !== key) {
    setShownKey(key);
    setValue(keptGet(key, initial));
  }
  const set = useCallback(
    (next: T | ((prev: T) => T)) =>
      setValue((prev) => keptSet(key, prev, typeof next === "function" ? (next as (p: T) => T)(prev) : next)),
    [key],
  );
  return [value, set];
}
