import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import {
  getSettings,
  onSettings,
  saveSettings,
  type Settings,
  type Style,
  type WidgetSlot,
} from "../settings";
import { GROUPS, WIDGETS, byId, type OptionField } from "../widgets/registry";
import { useReorder } from "../lib/useReorder";
import GlassCard from "../widgets/GlassCard";
import { activeMood, newMood, ownLook, patchMood } from "../lib/moods";
import { composing } from "../lib/ime";

const TABS = ["Look", "Widgets", "General", "About"] as const;
type Tab = (typeof TABS)[number];

const STYLES: { id: Style; name: string; swatch: string }[] = [
  {
    id: "glass",
    name: "Liquid glass",
    swatch:
      "linear-gradient(135deg, rgba(255,255,255,.55), rgba(255,255,255,.12))",
  },
  {
    id: "tinted",
    name: "Tinted",
    swatch: "linear-gradient(135deg, #8fb0ff, #4d6fd8)",
  },
  {
    id: "midnight",
    name: "Midnight",
    swatch: "linear-gradient(135deg, #2a3150, #0a0e1e)",
  },
  { id: "black", name: "Black", swatch: "#0b0b0d" },
  { id: "white", name: "White", swatch: "#f7f7f8" },
  {
    id: "clear",
    name: "Clear",
    swatch: "repeating-conic-gradient(#555 0 25%, #333 0 50%) 0 0 / 10px 10px",
  },
];
const ACCENTS = [
  "#ff453a",
  "#ff9f0a",
  "#ffd60a",
  "#30d158",
  "#64d2ff",
  "#0a84ff",
  "#bf5af2",
  "#ff375f",
];

/** What the rows need to show and apply "back to default". */
type Ctx = {
  s: Settings;
  update: (p: Partial<Settings>) => void;
  defaults: Settings | null;
};
const PanelCtx = createContext<Ctx | null>(null);

const same = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);

/** A small ↺ that puts the given settings back to their defaults; shown only when one differs. */
function Reset({ k }: { k: (keyof Settings)[] }) {
  const c = useContext(PanelCtx);
  const changed =
    !!c?.defaults && k.some((key) => !same(c.s[key], c.defaults![key]));
  return (
    <span className="reset-slot">
      {changed && (
        <button
          className="reset"
          title="Back to default"
          onClick={(e) => {
            e.preventDefault();
            c!.update(
              Object.fromEntries(
                k.map((key) => [key, c!.defaults![key]]),
              ) as Partial<Settings>,
            );
          }}
        >
          ↺
        </button>
      )}
    </span>
  );
}

/** The settings popup. Every change shows on the dock at once; it's written to disk 400 ms after the last one. */
export default function Panel() {
  const [s, setS] = useState<Settings | null>(null);
  const [defaults, setDefaults] = useState<Settings | null>(null);
  const [tab, setTab] = useState<Tab>("Look");
  const persist = useRef<number>(0);
  // When the panel last changed a setting. Rust echoes every save back (settings-changed); an echo of
  // an older save that lands mid-typing would undo the newest keystrokes, so echoes are ignored
  // briefly after a local edit.
  const lastEdit = useRef(0);
  // The fields edited here since the last write landed, and the panel's newest copy. A change made
  // elsewhere meanwhile (mood hotkey, dragging the dock) keeps its other fields instead of being
  // overwritten by the pending write.
  const touched = useRef(new Set<string>());
  const latest = useRef<Settings | null>(null);

  // A widget can open Settings at a tab and section ("General#weather"); see open_settings in lib.rs.
  useEffect(() => {
    const go = () =>
      invoke<string | null>("settings_goto")
        .then((at) => {
          if (!at) return;
          const [t, section] = at.split("#");
          if ((TABS as readonly string[]).includes(t)) setTab(t as Tab);
          if (section) setTimeout(() => document.getElementById(section)?.scrollIntoView({ block: "start", behavior: "smooth" }), 80);
        })
        .catch(() => {});
    go();
    const un = listen("settings-goto", go);
    return () => {
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    getSettings().then((x) => {
      latest.current = x;
      setS(x);
    });
    invoke<Settings>("default_settings").then(setDefaults);
    const off = onSettings((x) => {
      // Shortly after an edit here, the event may be the echo of that edit: keep what was edited.
      const mine = Date.now() - lastEdit.current <= 1500 && latest.current ? latest.current : null;
      const next = mine ? { ...x, ...Object.fromEntries([...touched.current].map((k) => [k, mine[k as keyof Settings]])) } : x;
      if (!mine) touched.current.clear();
      latest.current = next as Settings;
      setS(next as Settings);
    });
    // A shortcut field that's recording takes Escape for itself (it cancels the recording).
    const esc = (e: KeyboardEvent) =>
      e.key === "Escape" && !e.defaultPrevented && invoke("close_settings");
    window.addEventListener("keydown", esc);
    return () => {
      off.then((f) => f());
      window.removeEventListener("keydown", esc);
    };
  }, []);

  if (!s) return <div className="panel" />;

  const update = (patch: Partial<Settings>) => {
    const next = { ...(latest.current ?? s), ...patch };
    for (const k of Object.keys(patch)) touched.current.add(k);
    latest.current = next;
    lastEdit.current = Date.now();
    setS(next);
    saveSettings(next, false);
    clearTimeout(persist.current);
    persist.current = window.setTimeout(() => latest.current && saveSettings(latest.current, true), 400);
  };
  // Switching hands Rust the panel's latest settings and drops the pending write, which would
  // otherwise land after the switch and undo it.
  const switchMood = (id: string, from: Settings = latest.current ?? s) => {
    clearTimeout(persist.current);
    lastEdit.current = 0;
    touched.current.clear();
    invoke("set_mood", { id, current: from }).catch(() => {});
  };

  return (
    <PanelCtx.Provider value={{ s, update, defaults }}>
      <GlassCard
        className={`panel${s.style === "white" ? " light" : ""}`}
        settings={s}
        window="settings"
      >
        <header>
          <b>freedock</b>
          <nav>
            {TABS.map((t) => (
              <button
                key={t}
                className={t === tab ? "on" : ""}
                onClick={() => setTab(t)}
              >
                {t}
              </button>
            ))}
          </nav>
          <button
            className="x"
            title="Close (Esc)"
            onClick={() => invoke("close_settings")}
          >
            ×
          </button>
        </header>
        <section className="body">
          {tab === "Look" && <Look s={s} update={update} />}
          {tab === "Widgets" && (
            <>
              <MoodsBar s={s} update={update} switchMood={switchMood} />
              <Widgets s={s} update={update} />
            </>
          )}
          {tab === "General" && <General s={s} update={update} />}
          {tab === "About" && <About />}
        </section>
      </GlassCard>
    </PanelCtx.Provider>
  );
}

type TabProps = { s: Settings; update: (p: Partial<Settings>) => void };

/** Moods: saved layouts. The bar picks the mood being edited (switching the dock to it, so what you
 *  see is what you edit); the widget list below and the Look tab then change that mood. */
function MoodsBar({ s, update, switchMood }: TabProps & { switchMood: (id: string, from?: Settings) => void }) {
  const [renaming, setRenaming] = useState(false);
  const m = activeMood(s);
  const { dragging, item } = useReorder("moods", s.moods, (moods) => update({ moods }), () => {});
  return (
    <div className="moods">
      <div className="moodbar" role="tablist" aria-label="Moods">
        {s.moods.map((x, i) => (
          <button
            key={x.id}
            role="tab"
            aria-selected={x.id === s.active_mood}
            {...item(i)}
            data-grip
            className={`moodchip${x.id === s.active_mood ? " on" : ""}${dragging === i ? " dragging" : ""}`}
            title={x.id === s.active_mood ? "Editing this mood" : `Switch to ${x.name}`}
            onClick={() => x.id !== s.active_mood && switchMood(x.id)}
          >
            <span aria-hidden="true">{x.icon || "\u25cf"}</span> {x.name}
          </button>
        ))}
        <button
          className="moodchip add"
          title="New mood (a copy of this one)"
          aria-label="New mood"
          onClick={() => {
            const n = newMood(s);
            switchMood(n.id, { ...s, moods: [...s.moods, n] });
          }}
        >
          +
        </button>
      </div>
      {m && (
        <div className="moodedit">
          <input
            className="moodicon"
            aria-label="Mood icon"
            value={m.icon}
            maxLength={4}
            onChange={(e) => update(patchMood(s, m.id, { icon: e.target.value }))}
          />
          {renaming ? (
            <input
              autoFocus
              aria-label="Mood name"
              value={m.name}
              maxLength={24}
              onChange={(e) => update(patchMood(s, m.id, { name: e.target.value }))}
              onBlur={() => setRenaming(false)}
              onKeyDown={(e) => !composing(e) && e.key === "Enter" && setRenaming(false)}
            />
          ) : (
            <button className="move" onClick={() => setRenaming(true)}>
              Rename
            </button>
          )}
          <label className="ownlook" title="Its own style, glass, edge and show/hide; otherwise it shares the other moods' look">
            <Toggle on={!!m.look} onChange={(on) => update(ownLook(s, on))} /> Own look
          </label>
          <button
            className="move"
            disabled={s.moods.length < 2}
            title="Delete this mood"
            onClick={() => {
              // Switch to a neighbour first; the mood being left is then removed from the result.
              const other = s.moods.find((x) => x.id !== m.id);
              if (!other) return;
              const rest = { ...s, moods: s.moods.filter((x) => x.id !== m.id), active_mood: other.id, widgets: other.widgets, ...(other.look ?? s.shared_look ?? {}) };
              update(rest);
              switchMood(other.id, rest);
            }}
          >
            Delete
          </button>
        </div>
      )}
    </div>
  );
}

/** One setting. `k` names the settings it changes, for its "back to default" button. */
function Row({
  label,
  hint,
  k,
  children,
}: {
  label: string;
  hint?: string;
  k?: (keyof Settings)[];
  children: ReactNode;
}) {
  return (
    <div className="prow" role="group" aria-label={label}>
      <span>
        {label}
        {hint && <small>{hint}</small>}
      </span>
      <div className="ctl">
        {children}
        {k && <Reset k={k} />}
      </div>
    </div>
  );
}

function Seg<T extends string>({
  value,
  choices,
  onChange,
}: {
  value: T;
  choices: [T, string][];
  onChange: (v: T) => void;
}) {
  return (
    <div className="seg">
      {choices.map(([v, l]) => (
        <button
          key={v}
          className={v === value ? "on" : ""}
          onClick={() => onChange(v)}
        >
          {l}
        </button>
      ))}
    </div>
  );
}

function Slider({
  value,
  min,
  max,
  step,
  unit,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  step: number;
  unit?: string;
  onChange: (v: number) => void;
}) {
  return (
    <span className="slider">
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
      />
      <output>
        {step < 1 ? value.toFixed(2) : value}
        {unit}
      </output>
    </span>
  );
}

function Toggle({
  on,
  onChange,
}: {
  on: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <button
      className={`toggle${on ? " on" : ""}`}
      role="switch"
      aria-checked={on}
      onClick={() => onChange(!on)}
    />
  );
}

function Look({ s, update }: TabProps) {
  return (
    <>
      <h3 className="with-reset">
        Style <Reset k={["style"]} />
      </h3>
      <div className="styles">
        {STYLES.map((st) => (
          <button
            key={st.id}
            className={`style${s.style === st.id ? " on" : ""}`}
            onClick={() => update({ style: st.id })}
          >
            <i style={{ background: st.swatch }} />
            {st.name}
          </button>
        ))}
      </div>
      <h3>Bar</h3>
      <Row label="Shape" k={["shape"]}>
        <Seg
          value={s.shape}
          choices={[
            ["pill", "Pill"],
            ["full", "Full width"],
          ]}
          onChange={(shape) => update({ shape })}
        />
      </Row>
      <Row
        label="Show the dock"
        k={["visibility"]}
        hint={
          s.visibility === "smart"
            ? "tucks away while a window is maximized"
            : s.visibility === "autohide"
              ? "hidden until you push the edge"
              : "keeps its own strip of screen"
        }
      >
        <Seg
          value={s.visibility}
          choices={[
            ["always", "Always"],
            ["smart", "Smart"],
            ["autohide", "Auto-hide"],
          ]}
          onChange={(visibility) => update({ visibility })}
        />
      </Row>
      {s.visibility !== "always" && (
        <>
          <Row
            label="Open when within"
            k={["show_distance"]}
            hint="of the screen edge; bigger is easier to reach, but may pop up while you work near the edge"
          >
            <Slider value={s.show_distance} min={1} max={300} step={1} unit="px" onChange={(show_distance) => update({ show_distance })} />
          </Row>
          <Row label="Open after" k={["show_delay"]} hint="how long the pointer waits there first">
            <Slider value={s.show_delay} min={0} max={2000} step={10} unit="ms" onChange={(show_delay) => update({ show_delay })} />
          </Row>
          <Row
            label="Close when farther than"
            k={["hide_distance"]}
            hint="from the dock; bigger keeps it up while your pointer wanders nearby"
          >
            <Slider value={s.hide_distance} min={0} max={400} step={1} unit="px" onChange={(hide_distance) => update({ hide_distance })} />
          </Row>
          <Row label="Close after" k={["hide_delay"]} hint="how long the pointer stays away first">
            <Slider value={s.hide_delay} min={0} max={5000} step={10} unit="ms" onChange={(hide_delay) => update({ hide_delay })} />
          </Row>
          <Row label="Handle while hidden" k={["handle"]}>
            <Toggle on={s.handle} onChange={(handle) => update({ handle })} />
          </Row>
        </>
      )}
      <Row label="Position" k={["edge", "offset"]}>
        <Seg
          value={s.edge}
          choices={[
            ["bottom", "Bottom"],
            ["top", "Top"],
            ["left", "Left"],
            ["right", "Right"],
          ]}
          onChange={(edge) => update({ edge, offset: 0.5 })}
        />
      </Row>
      <Row label="Move by dragging" hint="or hold Alt and drag the dock">
        <button
          className="move"
          onClick={() => {
            invoke("edit_mode", { on: true });
            invoke("close_settings");
          }}
        >
          Move dock
        </button>
      </Row>
      <Row label="Height" k={["height"]}>
        <Slider
          value={s.height}
          min={40}
          max={76}
          step={1}
          unit="px"
          onChange={(height) => update({ height })}
        />
      </Row>
      <Row label="Gap to the edge" k={["margin"]}>
        <Slider
          value={s.margin}
          min={0}
          max={20}
          step={1}
          unit="px"
          onChange={(margin) => update({ margin })}
        />
      </Row>
      <Row label="Roundness" k={["radius"]}>
        <Slider
          value={s.radius}
          min={0}
          max={30}
          step={1}
          unit="px"
          onChange={(radius) => update({ radius })}
        />
      </Row>
      <Row label="Widest pill" k={["pill_width"]} hint="share of the screen">
        <Slider
          value={s.pill_width}
          min={0.3}
          max={1}
          step={0.01}
          onChange={(pill_width) => update({ pill_width })}
        />
      </Row>
      <Row label="Flyout animation" k={["popup_motion"]} hint="how flyouts and app previews appear">
        <select value={s.popup_motion ?? "warp"} onChange={(e) => update({ popup_motion: e.target.value as Settings["popup_motion"] })}>
          <option value="warp">Warp: grow out of the dock</option>
          <option value="fade">Fade</option>
          <option value="off">Off</option>
        </select>
      </Row>
      <h3>Glass</h3>
      <Row
        label="Live glass"
        k={["glass_live"]}
        hint={
          s.glass_live
            ? "shows the windows behind it; the dock stays out of screenshots and screen shares"
            : "shows the wallpaper"
        }
      >
        <Toggle
          on={s.glass_live}
          onChange={(glass_live) => update({ glass_live })}
        />
      </Row>
      <Row label="Tint" k={["glass_tint"]}>
        <Slider
          value={s.glass_tint}
          min={0}
          max={1}
          step={0.05}
          onChange={(glass_tint) => update({ glass_tint })}
        />
      </Row>
      <Row label="Blur" k={["glass_blur"]}>
        <Slider
          value={s.glass_blur}
          min={0}
          max={30}
          step={1}
          unit="px"
          onChange={(glass_blur) => update({ glass_blur })}
        />
      </Row>
      <Row label="Accent" k={["accent"]}>
        <div className="accents">
          {ACCENTS.map((c) => (
            <button
              key={c}
              className={s.accent === c ? "on" : ""}
              style={{ background: c }}
              onClick={() => update({ accent: c })}
            />
          ))}
          <input
            type="color"
            value={s.accent}
            onChange={(e) => update({ accent: e.target.value })}
            title="Any colour"
          />
        </div>
      </Row>
      <h3>Apps</h3>
      <Row label="Icon size" k={["icon_size"]}>
        <Slider
          value={s.icon_size}
          min={20}
          max={44}
          step={1}
          unit="px"
          onChange={(icon_size) => update({ icon_size })}
        />
      </Row>
      <Row label="Grow on hover" k={["magnify"]}>
        <Slider
          value={s.magnify}
          min={1}
          max={1.5}
          step={0.02}
          onChange={(magnify) => update({ magnify })}
        />
      </Row>
    </>
  );
}

function Widgets({ s, update }: TabProps) {
  const [open, setOpen] = useState<number | null>(null);
  const [adding, setAdding] = useState(false);
  const [find, setFind] = useState("");
  const q = find.trim().toLowerCase();
  const matches = (w: (typeof WIDGETS)[number]) => !q || `${w.name} ${w.blurb} ${w.group}`.toLowerCase().includes(q);
  // The open options panel is tracked by position, so it closes when widgets move or go away.
  const setWidgets = (widgets: WidgetSlot[]) => {
    setOpen(null);
    update({ widgets });
  };
  const patchSlot = (i: number, p: Partial<WidgetSlot>) =>
    update({ widgets: s.widgets.map((w, j) => (j === i ? { ...w, ...p } : w)) });
  // Each step of a drag is saved as it happens (update persists), so letting go has nothing to do.
  const { dragging, item } = useReorder("widgets", s.widgets, setWidgets, () => {});

  return (
    <>
      <p className="hint">
        Drag to reorder. Changes show on the dock right away.
      </p>
      <ul className="wlist">
        {s.widgets.map((slot, i) => {
          const def = byId(slot.id);
          const opts = { ...def?.defaults, ...slot.options };
          return (
            <li
              key={`${slot.id}-${i}`}
              {...item(i)}
              className={dragging === i ? "dragging" : ""}
            >
              <div className="wrow">
                <span className="grip" aria-hidden="true">
                  ⋮⋮
                </span>
                <span className="wname">
                  {def?.name ?? slot.id}
                  {!def?.component && <em>soon</em>}
                </span>
                {def?.fields && (
                  <button
                    className="gear"
                    title="Options"
                    onClick={() => setOpen(open === i ? null : i)}
                  >
                    ⚙
                  </button>
                )}
                <Toggle
                  on={slot.enabled}
                  onChange={(enabled) => patchSlot(i, { enabled })}
                />
                <button
                  className="rm"
                  title="Remove"
                  onClick={() =>
                    setWidgets(s.widgets.filter((_, j) => j !== i))
                  }
                >
                  ×
                </button>
              </div>
              {open === i && def?.fields && (
                <div className="wopts">
                  {def.fields.map((f) => (
                    <Field
                      key={f.key}
                      f={f}
                      value={opts[f.key]}
                      onChange={(v) =>
                        patchSlot(i, {
                          options: { ...slot.options, [f.key]: v },
                        })
                      }
                    />
                  ))}
                </div>
              )}
            </li>
          );
        })}
      </ul>
      <button className="add" onClick={() => setAdding(!adding)}>
        {adding ? "Done adding" : "+ Add widget"}
      </button>
      <DefaultWidgets />
      {adding && (
        <input
          className="wfind"
          autoFocus
          value={find}
          placeholder={`Search ${WIDGETS.filter((w) => w.component).length} widgets (weather, timer, git…)`}
          onChange={(e) => setFind(e.target.value)}
          onKeyDown={(e) => e.key === "Escape" && setFind("")}
        />
      )}
      {adding && q && !WIDGETS.some(matches) && <p className="wnone">No widget matches "{find}".</p>}
      {adding &&
        GROUPS.map((g) => {
          const items = WIDGETS.filter((w) => w.group === g && matches(w));
          if (!items.length) return null;
          return (
            <div key={g} className="picker">
              <h4>{g}</h4>
              {items.map((w) => {
                const have = !w.multi && s.widgets.some((x) => x.id === w.id);
                const ready = !!w.component;
                return (
                  <button
                    key={w.id}
                    className="pick"
                    disabled={have || !ready}
                    onClick={() =>
                      setWidgets([
                        ...s.widgets,
                        { id: w.id, enabled: true, options: null },
                      ])
                    }
                  >
                    <b>{w.name}</b>
                    <small>{w.blurb}</small>
                    {!ready ? <em>soon</em> : have ? <em>added</em> : null}
                  </button>
                );
              })}
            </div>
          );
        })}
    </>
  );
}

function Field({
  f,
  value,
  onChange,
}: {
  f: OptionField;
  value: unknown;
  onChange: (v: unknown) => void;
}) {
  if (f.type === "toggle") {
    return (
      <Row label={f.label}>
        <Toggle on={value === true} onChange={onChange} />
      </Row>
    );
  }
  if (f.type === "image") {
    return <ImageField label={f.label} value={typeof value === "string" ? value : ""} onChange={onChange} />;
  }
  if (f.type === "select") {
    return (
      <Row label={f.label}>
        <select
          value={String(value ?? "")}
          onChange={(e) => onChange(e.target.value)}
        >
          {f.choices.map(([v, l]) => (
            <option key={v} value={v}>
              {l}
            </option>
          ))}
        </select>
      </Row>
    );
  }
  return (
    <Row label={f.label}>
      <TextOption value={String(value ?? "")} placeholder={f.placeholder} onChange={onChange} />
    </Row>
  );
}

/** A widget's text option. It's saved on Enter or when the box loses focus, not on every key:
 *  options like Site status, Ping, Stocks and Headlines fetch what they're given, and a half-typed
 *  "mybank.co" is someone else's site. */
function TextOption({ value, placeholder, onChange }: { value: string; placeholder?: string; onChange: (v: unknown) => void }) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const commit = () => draft !== value && onChange(draft);
  return (
    <input
      type="text"
      value={draft}
      placeholder={placeholder}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
        if (e.key === "Escape" && draft !== value) {
          e.preventDefault();
          setDraft(value);
        }
      }}
    />
  );
}

/** Pick an image or GIF from disk; the dock keeps its own copy (stickers.rs). */
function ImageField({ label, value, onChange }: { label: string; value: string; onChange: (v: unknown) => void }) {
  const [src, setSrc] = useState<string | null>(null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    setSrc(null);
    if (value) invoke<string>("sticker_get", { name: value }).then(setSrc).catch((e) => setErr(String(e)));
  }, [value]);
  const pick = (file: File | undefined) => {
    if (!file) return;
    setErr("");
    if (file.size > 12 * 1024 * 1024) return setErr("That image is over 12 MB");
    setBusy(true);
    const r = new FileReader();
    r.onload = () =>
      invoke<string>("sticker_save", { data: String(r.result) })
        .then((name) => onChange(name))
        .catch((e) => setErr(String(e)))
        .finally(() => setBusy(false));
    r.onerror = () => {
      setBusy(false);
      setErr("Couldn't read that file");
    };
    r.readAsDataURL(file);
  };
  return (
    <Row label={label}>
      <span className="imgfield">
        {src && <img src={src} alt="" />}
        <button className="small" disabled={busy} onClick={() => input.current?.click()}>
          {busy ? "Saving…" : value ? "Change…" : "Choose…"}
        </button>
        {value && (
          <button className="small ghost" onClick={() => onChange("")}>
            Remove
          </button>
        )}
        <input ref={input} type="file" accept="image/gif,image/png,image/apng,image/webp,image/jpeg,image/svg+xml" hidden onChange={(e) => pick(e.target.files?.[0])} />
        {err && <em className="err">{err}</em>}
      </span>
    </Row>
  );
}

type Place = {
  name: string;
  admin1?: string;
  country?: string;
  latitude: number;
  longitude: number;
};

function General({ s, update }: TabProps) {
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [q, setQ] = useState("");
  const [places, setPlaces] = useState<Place[]>([]);
  useEffect(() => {
    invoke<boolean>("autostart_get")
      .then(setAutostart)
      .catch(() => setAutostart(false));
  }, []);
  // City search through Open-Meteo's free geocoder (no key, nothing personal sent beyond the typed name).
  useEffect(() => {
    if (q.trim().length < 2) return setPlaces([]);
    // Answers for an older query are dropped, so a slow one can't replace the newer list.
    let current = true;
    const t = setTimeout(() => {
      fetch(
        `https://geocoding-api.open-meteo.com/v1/search?count=5&name=${encodeURIComponent(q.trim())}`,
      )
        .then((r) => r.json())
        .then((j) => current && setPlaces(j.results ?? []))
        .catch(() => current && setPlaces([]));
    }, 300);
    return () => {
      current = false;
      clearTimeout(t);
    };
  }, [q]);

  return (
    <>
      <h3>Clock</h3>
      <Row label="Time format" k={["hour12"]}>
        <Seg
          value={s.hour12 === null ? "auto" : s.hour12 ? "12" : "24"}
          choices={[
            ["auto", "Auto"],
            ["12", "12 h"],
            ["24", "24 h"],
          ]}
          onChange={(v) => update({ hour12: v === "auto" ? null : v === "12" })}
        />
      </Row>
      <h3 id="weather">Weather location</h3>
      <Row label="City" k={["city", "lat", "lon"]} hint={s.city}>
        <input
          type="text"
          value={q}
          placeholder="Search a city"
          onChange={(e) => setQ(e.target.value)}
        />
      </Row>
      {places.length > 0 && (
        <ul className="places">
          {places.map((p) => (
            <li key={`${p.latitude},${p.longitude}`}>
              <button
                onClick={() => {
                  update({ city: p.name, lat: p.latitude, lon: p.longitude });
                  setQ("");
                }}
              >
                {p.name}
                <small>
                  {[p.admin1, p.country].filter(Boolean).join(", ")}
                </small>
              </button>
            </li>
          ))}
        </ul>
      )}
      <h3 id="calendar">Calendar and to-dos</h3>
      <AgendaSettings />
      <h3>Behaviour</h3>
      <Row
        label="Settings shortcut"
        k={["hotkey"]}
        hint="click, then press keys"
      >
        <KeyField value={s.hotkey} onChange={(hotkey) => update({ hotkey })} />
      </Row>
      <MonitorRows s={s} update={update} />
      <Row label="Windows key opens freedock Start" k={["win_opens_start"]} hint="a tap of Win alone; Win+R, Win+E and other shortcuts still work">
        <Toggle on={s.win_opens_start} onChange={(win_opens_start) => update({ win_opens_start })} />
      </Row>
      <Row label="Win+1-9 open dock apps" k={["win_number_keys"]} hint="like the Windows taskbar: the Nth app on the dock">
        <Toggle on={s.win_number_keys} onChange={(win_number_keys) => update({ win_number_keys })} />
      </Row>
      <Row label="Web search" k={["web_search"]} hint="where Start's 'Search the web' goes; {q} is the search">
        <input type="text" value={s.web_search} onChange={(e) => update({ web_search: e.target.value })} spellCheck={false} />
      </Row>
      <Row label="Window previews" k={["previews"]} hint="live thumbnails when the pointer rests on an open app">
        <Toggle on={s.previews} onChange={(previews) => update({ previews })} />
      </Row>
      <Row label="Next mood" k={["mood_hotkey"]} hint="switches to the next saved layout">
        <KeyField value={s.mood_hotkey} onChange={(mood_hotkey) => update({ mood_hotkey })} />
      </Row>
      <Row label="Pick a mood by number" k={["mood_number_keys"]} hint="the same keys with 1-9 instead of the letter (some apps use these)">
        <Toggle on={s.mood_number_keys} onChange={(mood_number_keys) => update({ mood_number_keys })} />
      </Row>
      <Row label="Hide for fullscreen games and video" k={["hide_fullscreen"]}>
        <Toggle
          on={s.hide_fullscreen}
          onChange={(hide_fullscreen) => update({ hide_fullscreen })}
        />
      </Row>
      <h3>Agents</h3>
      <Row
        label="Quick Ask"
        k={["ask_hotkey"]}
        hint="a one-off Claude chat; nothing is saved"
      >
        <KeyField
          value={s.ask_hotkey}
          onChange={(ask_hotkey) => update({ ask_hotkey })}
        />
      </Row>
      <Row
        label="Ask about the selection"
        k={["ask_selection_hotkey"]}
        hint="copies the selected text into Quick Ask"
      >
        <KeyField
          value={s.ask_selection_hotkey}
          onChange={(ask_selection_hotkey) => update({ ask_selection_hotkey })}
        />
      </Row>
      <Row
        label="Let Quick Ask use tools"
        k={["ask_tools"]}
        hint="off: it can only read files and search the web. On: it can run commands, edit files and use your MCP servers without asking, so a page or email you ask about could try to make it act"
      >
        <Toggle on={s.ask_tools} onChange={(ask_tools) => update({ ask_tools })} />
      </Row>
      <Row
        label="Snip & ask"
        k={["snip_hotkey"]}
        hint="drag over part of the screen, then ask Claude about it"
      >
        <KeyField
          value={s.snip_hotkey}
          onChange={(snip_hotkey) => update({ snip_hotkey })}
        />
      </Row>
      <Row
        label="Quick to-do"
        k={["capture_hotkey"]}
        hint="type a to-do; it lands on your Today list"
      >
        <KeyField
          value={s.capture_hotkey}
          onChange={(capture_hotkey) => update({ capture_hotkey })}
        />
      </Row>
      <Row label="Calculator" k={["calc_hotkey"]} hint="the answer shows as you type">
        <KeyField value={s.calc_hotkey} onChange={(calc_hotkey) => update({ calc_hotkey })} />
      </Row>
      <Row
        label="Open Claude sessions with"
        k={["claude_launch"]}
        hint="{dir} is the project folder"
      >
        <input
          type="text"
          value={s.claude_launch}
          onChange={(e) => update({ claude_launch: e.target.value })}
        />
      </Row>
      <Row label="Start with Windows">
        <Toggle
          on={autostart === true}
          onChange={(on) =>
            invoke("autostart_set", { on })
              .then(() => setAutostart(on))
              .catch(() => {})
          }
        />
      </Row>
    </>
  );
}

/** Puts the dock's widgets (order, on/off and options) back to the default set. */
function DefaultWidgets() {
  const c = useContext(PanelCtx);
  if (!c?.defaults) return null;
  // Each starter mood (Daily, Focus, Coding) restores its own starter set; other moods get Daily's.
  const starter = c.defaults.moods?.find((m) => m.id === c.s.active_mood)?.widgets ?? c.defaults.widgets;
  if (same(c.s.widgets, starter)) return null;
  return (
    <button
      className="add"
      onClick={() => c.update({ widgets: starter })}
    >
      Restore default widgets
    </button>
  );
}

type MonitorInfo = { id: string; label: string; primary: boolean };

/** Which monitor the dock is on (dragging it to another screen also moves it), and whether Windows'
 *  taskbar stays on the others. Only shown with more than one monitor. */
function MonitorRows({ s, update }: TabProps) {
  const [mons, setMons] = useState<MonitorInfo[]>([]);
  useEffect(() => {
    invoke<MonitorInfo[]>("monitors_list").then(setMons).catch(() => {});
  }, []);
  if (mons.length < 2) return null;
  return (
    <>
      <Row label="Show dock on" k={["monitor"]} hint="or drag the dock (Move dock) onto another screen">
        <select value={mons.some((m) => m.id === s.monitor) ? s.monitor : ""} onChange={(e) => update({ monitor: e.target.value })}>
          <option value="">Main display</option>
          {mons
            .filter((m) => !m.primary)
            .map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
        </select>
      </Row>
      <Row label="Hide Windows' taskbar on other displays" k={["hide_other_taskbars"]} hint="off: the other displays keep their own taskbar, set to auto-hide (Windows applies auto-hide to every display)">
        <Toggle on={s.hide_other_taskbars} onChange={(hide_other_taskbars) => update({ hide_other_taskbars })} />
      </Row>
    </>
  );
}

type AgendaStatus = { mcp: string | null; calendars: number; claude_servers: string[] };

/** Agenda sources: private calendar links (iCal) and an optional MCP server. Both are secrets: they
 *  go straight to the backend, which keeps them encrypted, and are never shown again. */
function AgendaSettings() {
  const [st, setSt] = useState<AgendaStatus | null>(null);
  const [links, setLinks] = useState("");
  const [url, setUrl] = useState("");
  const [pick, setPick] = useState("");
  const [note, setNote] = useState("");
  const load = () => invoke<AgendaStatus>("agenda_status").then(setSt).catch(() => {});
  useEffect(() => {
    load();
  }, []);
  const done = (msg: string) => () => {
    setNote(msg);
    load();
  };
  const fail = (e: unknown) => setNote(String(e));
  return (
    <>
      <Row
        label="Calendar links"
        hint={
          st?.calendars
            ? `${st.calendars} connected. Paste new links to replace them, or save an empty box to remove them.`
            : "Paste your calendar's private iCal link (Google: Settings > your calendar > Secret address in iCal format; Outlook: Publish calendar > ICS). One per line."
        }
      >
        <textarea className="links" rows={2} placeholder="https://… .ics" value={links} onChange={(e) => setLinks(e.target.value)} spellCheck={false} />
        <button
          className="move"
          onClick={() =>
            invoke<number>("calendars_set", { links })
              .then((n) => done(n ? `Saved ${n} calendar link${n > 1 ? "s" : ""}` : "Calendar links removed")())
              .then(() => setLinks(""))
              .catch(fail)
          }
        >
          Save
        </button>
      </Row>
      <Row
        label="MCP server"
        hint={
          st?.mcp
            ? `Connected: ${st.mcp}. Its events and to-dos join the agenda; Quick to-do adds there.`
            : "Optional: an MCP server with today_view / add_todo tools (see docs/MCP_AGENDA.md)."
        }
      >
        {st?.mcp ? (
          <button className="move" onClick={() => invoke("mcp_disconnect").then(done("Disconnected")).catch(fail)}>
            Disconnect
          </button>
        ) : (
          <>
            {!!st?.claude_servers.length && (
              <>
                <select value={pick} onChange={(e) => setPick(e.target.value)}>
                  <option value="">From Claude Code…</option>
                  {st.claude_servers.map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </select>
                <button className="move" disabled={!pick} onClick={() => invoke("mcp_connect", { fromClaude: pick }).then(done(`Connected ${pick}`)).catch(fail)}>
                  Connect
                </button>
              </>
            )}
            <input type="text" placeholder="or paste https:// link" value={url} onChange={(e) => setUrl(e.target.value)} spellCheck={false} />
            <button
              className="move"
              disabled={!url.trim()}
              onClick={() =>
                invoke("mcp_connect", { url })
                  .then(done("Connected"))
                  .then(() => setUrl(""))
                  .catch(fail)
              }
            >
              Connect
            </button>
          </>
        )}
      </Row>
      {note && <p className="hint agnote">{note}</p>}
    </>
  );
}

/** Every setting back to its default, after a second click to confirm. Calendar links, the MCP
 *  server and "Start with Windows" aren't settings and stay as they are. */
function ResetAll() {
  const c = useContext(PanelCtx);
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const t = setTimeout(() => setArmed(false), 4000);
    return () => clearTimeout(t);
  }, [armed]);
  if (!c?.defaults) return null;
  return (
    <button
      className={armed ? "danger" : ""}
      onClick={() => {
        if (!armed) return setArmed(true);
        setArmed(false);
        c.update({ ...c.defaults! });
      }}
    >
      {armed
        ? "Click again to reset everything"
        : "Reset all settings to default"}
    </button>
  );
}

function About() {
  const [v, setV] = useState("");
  useEffect(() => {
    getVersion().then(setV);
  }, []);
  return (
    <>
      <h3>freedock {v}</h3>
      <p className="hint">
        Your own dock. Right-click the dock, click the tray icon, or press the
        shortcut to open this panel.
      </p>
      <div className="actions">
        <button onClick={() => invoke("reload_dock")}>Reload dock</button>
        <ResetAll />
        <button className="danger" onClick={() => invoke("quit")}>
          Restore taskbar &amp; quit
        </button>
      </div>
    </>
  );
}

/** Click, then press a key combination (at least one modifier). */
function KeyField({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  const [recording, setRecording] = useState(false);
  const record = (e: React.KeyboardEvent) => {
    if (!recording) return;
    e.preventDefault();
    if (e.key === "Escape") return setRecording(false);
    if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;
    const mods = [
      e.ctrlKey && "Ctrl",
      e.altKey && "Alt",
      e.shiftKey && "Shift",
      e.metaKey && "Super",
    ].filter(Boolean);
    if (!mods.length) return;
    const key = e.code.startsWith("Key")
      ? e.code.slice(3)
      : e.code.startsWith("Digit")
        ? e.code.slice(5)
        : e.code;
    onChange([...mods, key].join("+"));
    setRecording(false);
  };
  return (
    <button
      className={`key${recording ? " rec" : ""}`}
      onClick={() => setRecording(true)}
      onKeyDown={record}
      onBlur={() => setRecording(false)}
    >
      {recording ? "Press keys…" : value || "None"}
    </button>
  );
}
