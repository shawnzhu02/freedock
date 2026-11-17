import { useEffect, useState } from "react";

/** The current time, re-rendering on the minute (or the second when asked), never more often. */
export function useNow(everySecond = false) {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    let t: number;
    const tick = () => {
      const d = new Date();
      setNow(d);
      const wait = everySecond ? 1000 - d.getMilliseconds() : 60_000 - (d.getSeconds() * 1000 + d.getMilliseconds());
      t = window.setTimeout(tick, wait + 30);
    };
    tick();
    return () => clearTimeout(t);
  }, [everySecond]);
  return now;
}
