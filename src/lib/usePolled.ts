// Poll a backend command while the dock is showing (see active.ts). Used by the everyday widgets.
import { useEffect, useRef, useState } from "react";
import { useKept } from "./kept";
import { invoke } from "@tauri-apps/api/core";
import { isActive, onWake } from "./active";

/** Call `cmd` now and every `everyMs` while the dock is showing; returns the latest answer. A failed
 *  call keeps the last good answer (offline for a minute shouldn't blank the widget). */
export function usePolled<T>(cmd: string, args: Record<string, unknown>, everyMs: number): { data: T | null; error: string | null } {
  const key = JSON.stringify(args);
  // Kept per command and arguments, so a reopened flyout or remounted cell draws the last data at once.
  const [data, setData] = useKept<T | null>(`polled.${cmd}.${key}`, null);
  const [error, setError] = useState<string | null>(null);
  const latest = useRef(key);
  latest.current = key;
  // New arguments (another feed, other symbols): the old error doesn't apply to them.
  const shown = useRef(key);
  if (shown.current !== key) {
    shown.current = key;
    if (error !== null) setError(null);
  }
  useEffect(() => {
    let alive = true;
    const load = () =>
      invoke<T>(cmd, JSON.parse(key))
        .then((d) => {
          if (alive && latest.current === key) {
            setData(d);
            setError(null);
          }
        })
        .catch((e) => alive && latest.current === key && setError(String(e)));
    load();
    const t = window.setInterval(() => isActive() && load(), everyMs);
    const off = onWake(load);
    return () => {
      alive = false;
      clearInterval(t);
      off();
    };
  }, [cmd, key, everyMs]);
  return { data, error };
}
