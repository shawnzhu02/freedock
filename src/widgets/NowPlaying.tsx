import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { useMedia } from "../lib/media";

export function PlayGlyph({ playing, size = 14 }: { playing: boolean; size?: number }) {
  return (
    <svg viewBox="0 0 16 16" width={size} height={size} aria-hidden="true">
      {playing ? <path d="M4 2.5h2.6v11H4zM9.4 2.5H12v11H9.4z" fill="currentColor" /> : <path d="M4.5 2.4l9 5.6-9 5.6z" fill="currentColor" />}
    </svg>
  );
}

export function SkipGlyph({ back = false, size = 14 }: { back?: boolean; size?: number }) {
  return (
    <svg viewBox="0 0 16 16" width={size} height={size} aria-hidden="true" style={back ? { transform: "scaleX(-1)" } : undefined}>
      <path d="M2.5 3l7 5-7 5zM10.5 3h2.4v10h-2.4z" fill="currentColor" />
    </svg>
  );
}

/** What's playing (any app using Windows' media controls) with play/pause; a click on the title
 *  opens the full player flyout. */
export default function NowPlaying({ settings, options }: WidgetProps) {
  const { now, art, control } = useMedia(2000);
  const vertical = settings.edge === "left" || settings.edge === "right";
  if (!now) {
    if (options.hideWhenIdle !== false) return null;
    return (
      <div className="cell media idle" title="Nothing is playing">
        <PlayGlyph playing={false} />
      </div>
    );
  }
  return (
    <div className={`cell group media${now.playing ? " playing" : ""}`}>
      <button className="mmain" title={`${now.title}${now.artist ? ` · ${now.artist}` : ""} (${now.app})`} onClick={(e) => openAt("media", e, vertical)}>
        {art ? <img src={art} alt="" draggable={false} /> : <span className="noart">♪</span>}
        {!vertical && (
          <span className="mtext">
            <b>{now.title}</b>
            <small>{now.artist || now.app}</small>
          </span>
        )}
      </button>
      <button className="mbtn" title={now.playing ? "Pause" : "Play"} onClick={() => control("toggle")}>
        <PlayGlyph playing={now.playing} />
      </button>
      {!vertical && options.skip !== false && now.can_next && (
        <button className="mbtn" title="Next" onClick={() => control("next")}>
          <SkipGlyph />
        </button>
      )}
    </div>
  );
}
