import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../settings";

/** The mood switcher's flyout: every mood, the active one marked. Picking one switches at once. */
export default function MoodsView({ settings }: { settings: Settings }) {
  const pick = (id: string) => {
    invoke("set_mood", { id }).catch(() => {});
    invoke("popup_close");
  };
  return (
    <div className="view moodsv">
      <header>
        <b>Mood</b>
        <span className="sp" />
      </header>
      <div className="moodlist" role="listbox" aria-label="Moods">
        {settings.moods.map((m, i) => (
          <button
            key={m.id}
            role="option"
            aria-selected={m.id === settings.active_mood}
            className={`moodrow${m.id === settings.active_mood ? " on" : ""}`}
            onClick={() => pick(m.id)}
          >
            <span className="mi" aria-hidden="true">{m.icon || "\u25cf"}</span>
            <b>{m.name}</b>
            {i < 9 && settings.mood_number_keys && <small>{i + 1}</small>}
          </button>
        ))}
      </div>
      <button className="ghost moodedit" onClick={() => invoke("open_settings").then(() => invoke("popup_close"))}>
        Edit moods
      </button>
    </div>
  );
}
