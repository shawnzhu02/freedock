import { useEffect, useState } from "react";
import { clock, useMedia } from "../lib/media";
import { PlayGlyph, SkipGlyph } from "../widgets/NowPlaying";

/** The player flyout: cover, title, a seekable timeline (when the app reports one) and controls. */
export default function Media({ opened }: { opened: number }) {
  const { now, art, control, seek, refresh } = useMedia(1000);
  // The position moves on between polls while playing.
  const [tick, setTick] = useState(0);
  const [drag, setDrag] = useState<number | null>(null);
  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [opened]);
  useEffect(() => {
    setTick(0);
    if (!now?.playing) return;
    const t = window.setInterval(() => setTick((n) => n + 1), 1000);
    return () => clearInterval(t);
  }, [now?.position, now?.playing]);

  if (!now) {
    return (
      <div className="view player">
        <p className="empty center">Nothing is playing.</p>
      </div>
    );
  }
  const pos = drag ?? Math.min((now.position ?? 0) + tick, now.duration ?? 0);
  return (
    <div className="view player">
      <div className="cover">{art ? <img src={art} alt="" draggable={false} /> : <span>♪</span>}</div>
      <div className="ptext">
        <b title={now.title}>{now.title}</b>
        <span>{[now.artist, now.album].filter(Boolean).join(" · ") || now.app}</span>
        <small>{now.app}</small>
      </div>
      {now.duration ? (
        <div className="timeline">
          <input
            type="range"
            min={0}
            max={now.duration}
            step={1}
            value={pos}
            onChange={(e) => setDrag(Number(e.target.value))}
            onPointerUp={() => {
              if (drag !== null) seek(drag);
              setDrag(null);
            }}
            // Arrow keys, Page Up/Down, Home and End move the slider without a pointer: seek when the key is let go.
            onKeyUp={() => {
              if (drag !== null) seek(drag);
              setDrag(null);
            }}
          />
          <div className="times">
            <span>{clock(pos)}</span>
            <span>-{clock(now.duration - pos)}</span>
          </div>
        </div>
      ) : null}
      <div className="pcontrols">
        <button title="Previous" disabled={!now.can_prev} onClick={() => control("prev")}>
          <SkipGlyph back size={18} />
        </button>
        <button className="big" title={now.playing ? "Pause" : "Play"} onClick={() => control("toggle")}>
          <PlayGlyph playing={now.playing} size={22} />
        </button>
        <button title="Next" disabled={!now.can_next} onClick={() => control("next")}>
          <SkipGlyph size={18} />
        </button>
      </div>
    </div>
  );
}
