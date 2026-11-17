import { useEffect, useRef } from "react";
import { useKept } from "./kept";
import { invoke } from "@tauri-apps/api/core";
import { isActive, onWake } from "./active.ts";

export type NowPlaying = {
  title: string;
  artist: string;
  album: string;
  app: string;
  playing: boolean;
  position: number | null;
  duration: number | null;
  key: string;
  can_prev: boolean;
  can_next: boolean;
};

/** "3:07" / "1:02:09" from seconds. */
export const clock = (s: number) => {
  s = Math.max(0, Math.floor(s));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
};

/** The current media session, polled every `every` ms (slower while the window is hidden), with
 *  its cover art fetched once per track. */
export function useMedia(every = 2000) {
  // Kept across flyout opens, so the player draws at once instead of "Nothing is playing" first.
  const [now, setNow] = useKept<NowPlaying | null>("media.now", null);
  const [art, setArt] = useKept<string | null>("media.art", null);
  const key = useRef("");
  const seen = useRef("");
  const refresh = () =>
    invoke<NowPlaying | null>("media_now")
      .then((n) => {
        // Most polls change nothing; skip the re-render then.
        const j = JSON.stringify(n);
        if (j !== seen.current) {
          seen.current = j;
          setNow(n);
        }
        const k = n?.key ?? "";
        if (k !== key.current) {
          key.current = k;
          setArt(null);
          if (n) invoke<string | null>("media_art").then((a) => key.current === k && setArt(a)).catch(() => {});
        }
      })
      .catch(() => {});
  useEffect(() => {
    let t = 0;
    // Tucked away or hidden: nothing is asked for; coming back into view refreshes at once.
    const loop = () => {
      if (isActive()) refresh();
      t = window.setTimeout(loop, isActive() ? every : every * 4);
    };
    loop();
    const off = onWake(refresh);
    return () => {
      clearTimeout(t);
      off();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [every]);
  const control = (action: "toggle" | "next" | "prev") => {
    if (action === "toggle") setNow((n) => (n ? { ...n, playing: !n.playing } : n));
    invoke("media_control", { action }).finally(() => setTimeout(refresh, 350));
  };
  const seek = (seconds: number) => invoke("media_seek", { seconds }).finally(() => setTimeout(refresh, 300));
  return { now, art, control, seek, refresh };
}
