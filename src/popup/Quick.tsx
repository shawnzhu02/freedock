import { Fragment, useCallback, useEffect, useRef, useState, type ReactElement } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useKept } from "../lib/kept";
import { getSettings, saveSettings } from "../settings";
import { useMedia } from "../lib/media";
import { useSections } from "./Sections";
import { useReorder } from "../lib/useReorder";

type Audio = { volume: number; muted: boolean; device: string };
type Net = { ssid: string; signal: number; secure: boolean; connected: boolean; saved: boolean };
type Radios = { wifi: boolean | null; bluetooth: boolean | null };
type Power = { percent: number | null; charging: boolean; plugged: boolean; secs_left: number | null };
type Extra = {
  airplane: boolean | null;
  hotspot: { on: boolean; clients: number; name: string } | null;
  night_light: boolean | null;
  project: string | null;
  energy_saver: boolean | null;
  captions: boolean;
  magnifier: boolean;
  narrator: boolean;
  keyboard: boolean;
};
type Device = { id: string; name: string; default: boolean };
type AppVol = { name: string; key: string; volume: number; muted: boolean; active: boolean };

/** Every tile the flyout knows, in the default order. */
export const ALL_TILES = ["wifi", "bluetooth", "airplane", "hotspot", "night", "dark", "energy", "project", "access", "captions"];
const TILE_NAMES: Record<string, string> = {
  wifi: "Wi-Fi",
  bluetooth: "Bluetooth",
  airplane: "Airplane mode",
  hotspot: "Mobile hotspot",
  night: "Night light",
  dark: "Dark mode",
  energy: "Energy saver",
  project: "Project",
  access: "Accessibility",
  captions: "Live captions",
};
const PROJECT: [string, string, string][] = [
  ["pc", "PC screen only", "Only this screen"],
  ["duplicate", "Duplicate", "The same picture on both"],
  ["extend", "Extend", "One desktop across both"],
  ["second", "Second screen only", "Only the other screen"],
];

/** The saved tile list, keeping only known tiles, without repeats. */
export function cleanTiles(list: string[] | undefined | null): string[] {
  const out: string[] = [];
  for (const t of list ?? ALL_TILES) if (ALL_TILES.includes(t) && !out.includes(t)) out.push(t);
  return out;
}


const svg = (d: string) => (
  <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
    <path d={d} fill="currentColor" />
  </svg>
);
const ICONS: Record<string, ReactElement> = {
  airplane: svg("M21 16v-2l-8-5V3.5a1.5 1.5 0 0 0-3 0V9l-8 5v2l8-2.5V19l-2 1.5V22l3.5-1 3.5 1v-1.5L13 19v-5.5z"),
  hotspot: svg("M12 11a2 2 0 1 0 0 4 2 2 0 0 0 0-4zm-6 2a6 6 0 0 1 12 0h-2a4 4 0 0 0-8 0zm-4 0a10 10 0 0 1 20 0h-2a8 8 0 0 0-16 0z"),
  night: svg("M10 4a8 8 0 1 0 10 10A7 7 0 0 1 10 4zm8-2 1 2 2 1-2 1-1 2-1-2-2-1 2-1z"),
  dark: svg("M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"),
  energy: svg("M17 8C8 10 5.9 16.2 3.8 21.3l1.9.7.9-2.3c.5.2 1 .3 1.4.3C19 20 22 3 22 3c-1 2-8 2.3-13 3.3S2 11.5 2 13.5s1.8 3.8 1.8 3.8C7 8 17 8 17 8z"),
  project: svg("M3 5h12v9H3zm2 2v5h8V7zm4 9h2v2H7v-2zm8-7h4v9h-8v-2h6V9z"),
  access: svg("M12 2a2 2 0 1 1 0 4 2 2 0 0 1 0-4zm9 7h-6v13h-2v-6h-2v6H9V9H3V7h18z"),
  captions: svg("M19 4H5a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2zm-8 7H9.5v-.5h-2v3h2V13H11v1a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1zm7 0h-1.5v-.5h-2v3h2V13H18v1a1 1 0 0 1-1 1h-3a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1z"),
};

function Tile({
  on,
  icon,
  name,
  sub,
  title,
  onClick,
  onMore,
  moreTitle,
}: {
  on?: boolean;
  icon: ReactElement;
  name: string;
  sub: string;
  title?: string;
  onClick?: () => void;
  onMore?: () => void;
  moreTitle?: string;
}) {
  return (
    <div className={`tile${on ? " on" : ""}`}>
      <button className="main" onClick={onClick} title={title}>
        {icon}
        <span>
          <b>{name}</b>
          <small>{sub}</small>
        </span>
      </button>
      {onMore && (
        <button className="more" title={moreTitle} onClick={onMore}>
          ›
        </button>
      )}
    </div>
  );
}

/** Microphone: mute toggle and the device name (both default mics, so calls hear nothing). */
function MicRow({ opened }: { opened: number }) {
  const [mic, setMic] = useKept<Audio | null | undefined>("quick.mic", undefined);
  useEffect(() => {
    invoke<Audio | null>("quick_mic").then(setMic).catch(() => setMic(null));
  }, [opened]);
  if (!mic) return null;
  return (
    <button
      className={`microw${mic.muted ? " muted" : ""}`}
      title={mic.muted ? "Unmute the microphone" : "Mute the microphone"}
      onClick={() => {
        const muted = !mic.muted;
        setMic({ ...mic, muted });
        invoke("quick_set_mic", { muted }).catch(() => invoke<Audio | null>("quick_mic").then(setMic));
      }}
    >
      <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true">
        <path d="M12 14a3 3 0 0 0 3-3V5a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.9V21h2v-3.1A7 7 0 0 0 19 11z" fill="currentColor" />
        {mic.muted && <path d="M4 3l17 17" stroke="currentColor" strokeWidth="2" />}
      </svg>
      <span>
        <b>{mic.muted ? "Microphone muted" : "Microphone on"}</b>
        <small>{mic.device}</small>
      </span>
    </button>
  );
}

/** A one-line player: art, title and artist, previous / play-pause / next. */
function MediaMini() {
  const { now, art, control } = useMedia(2500);
  if (!now) return null;
  return (
    <div className="mmini">
      {art ? <img src={art} alt="" /> : <i className="noart" />}
      <span>
        <b>{now.title || "Playing"}</b>
        <small>{[now.artist, now.app].filter(Boolean).join(" · ")}</small>
      </span>
      <button className="icon" title="Previous" disabled={!now.can_prev} onClick={() => control("prev")}>
        ⏮︎
      </button>
      <button className="icon" title={now.playing ? "Pause" : "Play"} onClick={() => control("toggle")}>
        {now.playing ? "⏸︎" : "▶︎"}
      </button>
      <button className="icon" title="Next" disabled={!now.can_next} onClick={() => control("next")}>
        ⏭︎
      </button>
    </div>
  );
}

/** Sound: pick the output, and one volume per app. */
function SoundPage({ back }: { back: () => void }) {
  const [msg, note] = useState("");
  const [devices, setDevices] = useKept<Device[] | null>("quick.devices", null);
  const [apps, setApps] = useKept<AppVol[] | null>("quick.apps", null);
  const loadApps = useCallback(() => invoke<AppVol[]>("quick_apps").then(setApps).catch(() => setApps([])), []);
  const loadDevices = useCallback(() => invoke<Device[]>("quick_devices").then(setDevices).catch(() => setDevices([])), []);
  useEffect(() => {
    loadDevices();
    loadApps();
    const t = window.setInterval(loadApps, 2500);
    return () => clearInterval(t);
  }, [loadApps, loadDevices]);
  const sendApp = useThrottle((v: { key: string; volume: number }) => invoke("quick_set_app", { key: v.key, volume: v.volume, muted: null }).catch(() => {}), 60);
  const pick = (d: Device) => {
    if (d.default) return;
    setDevices((l) => l?.map((x) => ({ ...x, default: x.id === d.id })) ?? l);
    invoke("quick_set_device", { id: d.id })
      .catch((e) => note(String(e)))
      .finally(() => {
        loadDevices();
        setTimeout(loadApps, 500);
      });
  };
  return (
    <div className="view quick">
      <header>
        <button className="x back" title="Back" onClick={back}>
          ‹
        </button>
        <b>Sound</b>
      </header>
      <div className="scroll qpage">
        <h5>Output</h5>
        {devices === null && <span className="dots" />}
        {devices?.length === 0 && <p className="empty">No speakers or headphones found.</p>}
        {devices?.map((d) => (
          <button key={d.id} className={`opt${d.default ? " on" : ""}`} onClick={() => pick(d)}>
            <i className="radio" />
            <span>{d.name}</span>
          </button>
        ))}
        <h5>Volume mixer</h5>
        {apps?.length === 0 && <p className="empty">No apps are using sound.</p>}
        {apps?.map((a) => (
          <div className={`qrow app${a.active ? " live" : ""}`} key={a.key}>
            <button
              className="icon"
              title={a.muted ? `Unmute ${a.name}` : `Mute ${a.name}`}
              onClick={() => {
                setApps((l) => l?.map((x) => (x.key === a.key ? { ...x, muted: !a.muted } : x)) ?? l);
                invoke("quick_set_app", { key: a.key, volume: null, muted: !a.muted }).catch((e) => note(String(e)));
              }}
            >
              <Speaker muted={a.muted} level={a.volume} />
            </button>
            <span className="aname" title={a.active ? "Playing now" : ""}>
              {a.name}
            </span>
            <input
              type="range"
              min={0}
              max={100}
              value={a.volume}
              onChange={(e) => {
                const v = Number(e.target.value);
                setApps((l) => l?.map((x) => (x.key === a.key ? { ...x, volume: v } : x)) ?? l);
                sendApp({ key: a.key, volume: v });
              }}
            />
            <output>{a.volume}</output>
          </div>
        ))}
      </div>
      {msg && <p className="note">{msg}</p>}
      <footer className="qfoot">
        <button className="link" onClick={() => openWin("sound")}>
          More sound settings
        </button>
      </footer>
    </div>
  );
}

const openWin = (page: string) => invoke("open_windows", { page }).then(() => invoke("popup_close"));

/** Signal strength as 1-4 bars. */
export const bars = (signal: number) => (signal >= 75 ? 4 : signal >= 50 ? 3 : signal >= 25 ? 2 : 1);

/** "2 h 05 min" from seconds. */
export const hm = (secs: number) => {
  const h = Math.floor(secs / 3600);
  const m = Math.round((secs % 3600) / 60);
  return h ? `${h} h ${String(m).padStart(2, "0")} min` : `${m} min`;
};

function WifiGlyph({ level = 4 }: { level?: number }) {
  return (
    <svg viewBox="0 0 20 16" width="18" height="15" aria-hidden="true">
      {[
        "M1.5 5.2a12 12 0 0 1 17 0",
        "M4.3 8.2a8 8 0 0 1 11.4 0",
        "M7.1 11.1a4 4 0 0 1 5.8 0",
      ].map((d, i) => (
        <path key={d} d={d} fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" opacity={3 - i < level ? 1 : 0.28} />
      ))}
      <circle cx="10" cy="14" r="1.4" fill="currentColor" />
    </svg>
  );
}

function BtGlyph() {
  return (
    <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
      <path d="M4 4.5l7 6-3.5 3V2.5l3.5 3-7 6" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function Speaker({ muted, level }: { muted: boolean; level: number }) {
  return (
    <svg viewBox="0 0 18 16" width="17" height="15" aria-hidden="true">
      <path d="M2 6h3l4-3.5v11L5 10H2z" fill="currentColor" />
      {muted ? (
        <path d="M12 5.5l4.5 5M16.5 5.5L12 10.5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      ) : (
        <>
          {level > 0 && <path d="M12 5.2a4 4 0 0 1 0 5.6" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />}
          {level > 50 && <path d="M14.3 3a7 7 0 0 1 0 10" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />}
        </>
      )}
    </svg>
  );
}

function Sun() {
  return (
    <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
      <circle cx="8" cy="8" r="3" fill="currentColor" />
      <path d="M8 1v2M8 13v2M1 8h2M13 8h2M3 3l1.4 1.4M11.6 11.6L13 13M3 13l1.4-1.4M11.6 4.4L13 3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

/** Calls `fn` at most every `ms` while a slider moves, and always with the last value. */
function useThrottle<T>(fn: (v: T) => void, ms: number) {
  const last = useRef(0);
  const pending = useRef<number>(0);
  return (v: T) => {
    clearTimeout(pending.current);
    const wait = last.current + ms - Date.now();
    if (wait <= 0) {
      last.current = Date.now();
      fn(v);
    } else {
      pending.current = window.setTimeout(() => {
        last.current = Date.now();
        fn(v);
      }, wait);
    }
  };
}

/** Our own quick settings: Wi-Fi and Bluetooth, volume, brightness and battery. */
export default function Quick({ opened }: { opened: number }) {
  const [audio, setAudio] = useKept<Audio | null>("quick.audio", null);
  const [radios, setRadios] = useKept<Radios | null>("quick.radios", null);
  const [nets, setNets] = useKept<Net[] | null>("quick.nets", null);
  const [bright, setBright] = useKept<number | null | undefined>("quick.bright", undefined);
  const [power, setPower] = useKept<Power | null>("quick.power", null);
  const [showNets, setShowNets] = useState(false);
  const [note, setNote] = useState("");
  const [joining, setJoining] = useState("");
  const [extra, setExtra] = useKept<Extra | null>("quick.extra", null);
  const [tiles, setTiles] = useKept<string[]>("quick.tiles", ALL_TILES);
  const [editing, setEditing] = useState(false);
  const [page, setPage] = useState<"" | "sound" | "project" | "access">("");
  const loadExtra = useCallback(() => invoke<Extra>("quick_extra").then(setExtra).catch(() => setExtra((x) => x ?? ({} as Extra))), []);
  useEffect(() => {
    setPage("");
    setEditing(false);
    loadExtra();
    getSettings()
      .then((s) => setTiles(cleanTiles(s.quick_tiles)))
      .catch(() => {});
  }, [opened, loadExtra]);
  const { dragging: dragTile, item: tileItem } = useReorder("tiles", tiles, setTiles, (t) => saveTiles(t));
  const saveTiles = (next: string[]) => {
    setTiles(next);
    getSettings()
      .then((s) => saveSettings({ ...s, quick_tiles: next }, true))
      .catch((e) => setNote(String(e)));
  };
  /** Switch one of the extra tiles; reads the real state back afterwards. */
  const flip = (what: string, on?: boolean, mode?: string) => {
    setNote("");
    invoke("quick_set_extra", { what, on: on ?? null, mode: mode ?? null })
      .catch((e) => setNote(String(e)))
      .finally(() => setTimeout(loadExtra, what === "hotspot" || what === "airplane" || what === "project" ? 1500 : 500));
  };
  const setX = (patch: Partial<Extra>) => setExtra((x) => (x ? { ...x, ...patch } : x));

  const loadNets = useCallback((scan: boolean) => {
    invoke<Net[]>("quick_networks", { scan }).then(setNets).catch(() => setNets([]));
  }, []);
  const loadRadios = useCallback(() => invoke<Radios>("quick_radios").then(setRadios).catch(() => setRadios((r) => r ?? { wifi: null, bluetooth: null })), []);
  // undefined: not known yet; null: this PC has no dark-mode switch we can use.
  const [dark, setDark] = useKept<boolean | null | undefined>("quick.dark", undefined);
  const [link, setLink] = useKept("quick.link", "");
  useEffect(() => {
    invoke<boolean | null>("quick_dark").then(setDark).catch(() => setDark(null));
    invoke<string>("net_link").then(setLink).catch(() => {});
  }, [opened]);
  const toggleDark = () => {
    if (dark == null) return;
    setDark(!dark);
    invoke("quick_set_dark", { dark: !dark }).catch(() => invoke<boolean | null>("quick_dark").then(setDark));
  };
  useEffect(() => {
    setShowNets(false);
    setNote("");
    invoke<Audio | null>("quick_audio").then(setAudio).catch(() => {});
    invoke<Power>("power").then(setPower).catch(() => {});
    invoke<number | null>("quick_brightness").then(setBright).catch(() => setBright(null));
    loadRadios();
    loadNets(false);
  }, [opened, loadNets, loadRadios]);

  const sendVolume = useThrottle((v: number) => invoke("quick_set_audio", { volume: v, muted: null }).catch(() => {}), 60);
  const sendBright = useThrottle((v: number) => invoke("quick_set_brightness", { level: v }).catch(() => {}), 350);

  const connected = nets?.find((n) => n.connected);
  const toggleRadio = (kind: "wifi" | "bluetooth", on: boolean) => {
    setRadios((r) => (r ? { ...r, [kind]: on } : r));
    invoke("quick_set_radio", { kind, on })
      .catch((e) => setNote(String(e)))
      .finally(() => {
        loadRadios();
        if (kind === "wifi") setTimeout(() => loadNets(false), 1500);
      });
  };
  const join = (n: Net) => {
    if (!n.saved) return openWin("wifi");
    setJoining(n.ssid);
    invoke("quick_connect", { ssid: n.connected ? null : n.ssid })
      .catch((e) => setNote(String(e)))
      .finally(() => {
        setTimeout(() => {
          setJoining("");
          loadNets(false);
        }, 2500);
      });
  };

  const onOff = (v: boolean | null | undefined) => (v ? "On" : "Off");
  // A tile whose state hasn't come back yet keeps its place (drawn off, without details) instead of
  // popping in later: tiles arriving one by one re-laid out the grid and resized the window each time.
  const waiting = (id: string) =>
    id === "wifi" || id === "bluetooth" ? radios === null : id === "dark" ? dark === undefined : extra === null;
  const shown = (id: string): ReactElement | null =>
    waiting(id) ? <Tile icon={ICONS[id] ?? <span />} name={TILE_NAMES[id]} sub="" /> : tile(id);
  const tile = (id: string): ReactElement | null => {
    switch (id) {
      case "wifi":
        return radios?.wifi === null ? null : (
          <Tile
            on={!!radios?.wifi}
            icon={<WifiGlyph level={connected ? bars(connected.signal) : 4} />}
            name="Wi-Fi"
            sub={radios?.wifi === false ? (link === "ethernet" ? "Off · on Ethernet" : "Off") : connected?.ssid ?? (link === "ethernet" ? "Using Ethernet" : "Not connected")}
            title="Wi-Fi on or off"
            onClick={() => toggleRadio("wifi", !radios?.wifi)}
            onMore={() => setShowNets(true)}
            moreTitle="Networks"
          />
        );
      case "bluetooth":
        return radios?.bluetooth == null ? null : (
          <Tile
            on={radios.bluetooth}
            icon={<BtGlyph />}
            name="Bluetooth"
            sub={onOff(radios.bluetooth)}
            title="Bluetooth on or off"
            onClick={() => toggleRadio("bluetooth", !radios.bluetooth)}
            onMore={() => openWin("bluetooth")}
            moreTitle="Bluetooth devices"
          />
        );
      case "airplane":
        return extra?.airplane == null ? null : (
          <Tile
            on={extra.airplane}
            icon={ICONS.airplane}
            name="Airplane mode"
            sub={onOff(extra.airplane)}
            title="Turns off Wi-Fi, Bluetooth and other radios"
            onClick={() => {
              const on = !extra.airplane;
              setX({ airplane: on });
              flip("airplane", on);
              setTimeout(loadRadios, 1500);
            }}
          />
        );
      case "hotspot":
        return extra?.hotspot == null ? null : (
          <Tile
            on={extra.hotspot.on}
            icon={ICONS.hotspot}
            name="Mobile hotspot"
            sub={extra.hotspot.on ? `${extra.hotspot.clients} connected` : "Off"}
            title={extra.hotspot.name ? `Shares this PC's internet as "${extra.hotspot.name}"` : "Share this PC's internet"}
            onClick={() => {
              const on = !extra.hotspot!.on;
              setX({ hotspot: { ...extra.hotspot!, on } });
              flip("hotspot", on);
            }}
            onMore={() => openWin("hotspot")}
            moreTitle="Hotspot name and password"
          />
        );
      case "night":
        return extra?.night_light == null ? null : (
          <Tile
            on={extra.night_light}
            icon={ICONS.night}
            name="Night light"
            sub={onOff(extra.night_light)}
            title="Warmer colours to help you sleep"
            onClick={() => {
              const on = !extra.night_light;
              setX({ night_light: on });
              flip("night_light", on);
            }}
            onMore={() => openWin("nightlight")}
            moreTitle="Night light settings"
          />
        );
      case "dark":
        return dark === null ? null : <Tile on={dark} icon={ICONS.dark} name="Dark mode" sub={onOff(dark)} title="Dark or light theme for Windows and apps" onClick={toggleDark} />;
      case "energy":
        return extra?.energy_saver == null ? null : (
          <Tile
            on={extra.energy_saver}
            icon={ICONS.energy}
            name="Energy saver"
            sub={extra.energy_saver ? "On" : "Off · settings"}
            title="Windows only lets its own settings switch this"
            onClick={() => openWin("battery")}
          />
        );
      case "project":
        return (
          <Tile
            icon={ICONS.project}
            name="Project"
            sub={PROJECT.find(([k]) => k === extra?.project)?.[1] ?? "Display mode"}
            title="PC screen only, duplicate, extend or second screen only"
            onClick={() => setPage("project")}
          />
        );
      case "access": {
        const on = [extra?.magnifier && "Magnifier", extra?.narrator && "Narrator", extra?.keyboard && "Keyboard"].filter(Boolean) as string[];
        return <Tile on={on.length > 0} icon={ICONS.access} name="Accessibility" sub={on.length ? on.join(", ") : "Magnifier, Narrator…"} onClick={() => setPage("access")} />;
      }
      case "captions":
        return (
          <Tile
            on={!!extra?.captions}
            icon={ICONS.captions}
            name="Live captions"
            sub={onOff(extra?.captions)}
            title="Captions for any audio on this PC"
            onClick={() => {
              setX({ captions: !extra?.captions });
              flip("captions");
            }}
            onMore={() => openWin("captions")}
            moreTitle="Live captions settings"
          />
        );
    }
    return null;
  };

  const { body, button: customize } = useSections(
    "quick",
    [
      { id: "tiles", name: "Tiles", hint: "Edit tiles chooses and orders them", node:     editing ? (
            <>
              <p className="hint">Drag to reorder · − hides a tile</p>
              <div className="tiles editing">
                {tiles.map((id, i) => (
                  <div
                    key={id}
                    className={`tedit${dragTile === i ? " dragging" : ""}`}
                    {...tileItem(i)}
                  >
                    {tile(id) ?? <Tile icon={ICONS[id] ?? <span />} name={TILE_NAMES[id]} sub="Not on this PC" />}
                    <button className="rm" title={`Hide ${TILE_NAMES[id]}`} onClick={() => saveTiles(tiles.filter((t) => t !== id))}>
                      −
                    </button>
                  </div>
                ))}
              </div>
              {ALL_TILES.some((t) => !tiles.includes(t)) && (
                <div className="addtiles">
                  {ALL_TILES.filter((t) => !tiles.includes(t)).map((t) => (
                    <button key={t} className="chip" onClick={() => saveTiles([...tiles, t])}>
                      + {TILE_NAMES[t]}
                    </button>
                  ))}
                </div>
              )}
            </>
          ) : (
            <div className="tiles">{tiles.map((id) => <Fragment key={id}>{shown(id)}</Fragment>)}</div>
          ) },
      { id: "volume", name: "Volume", hint: "the › opens outputs and the per-app mixer", node: (
          <>
          {audio && (
            <div className="qrow">
              <button
                className="icon"
                title={audio.muted ? "Unmute" : "Mute"}
                onClick={() => {
                  const muted = !audio.muted;
                  setAudio({ ...audio, muted });
                  invoke("quick_set_audio", { volume: null, muted });
                }}
              >
                <Speaker muted={audio.muted} level={audio.volume} />
              </button>
              <input
                type="range"
                min={0}
                max={100}
                value={audio.volume}
                onChange={(e) => {
                  const v = Number(e.target.value);
                  setAudio({ ...audio, volume: v });
                  sendVolume(v);
                }}
              />
              <output>{audio.volume}</output>
              <button className="icon go" title="Output and volume mixer" onClick={() => setPage("sound")}>
                ›
              </button>
            </div>
          )}
          {audio?.device && (
            <button className="device" onClick={() => setPage("sound")} title="Output and volume mixer">
              {audio.device}
            </button>
          )}
          </>
        ) },
      { id: "mic", name: "Microphone", hint: "mute for calls", node: <MicRow opened={opened} /> },
      { id: "brightness", name: "Brightness", node:     typeof bright === "number" && (
            <div className="qrow">
              <span className="icon">
                <Sun />
              </span>
              <input
                type="range"
                min={0}
                max={100}
                value={bright}
                onChange={(e) => {
                  const v = Number(e.target.value);
                  setBright(v);
                  sendBright(v);
                }}
              />
              <output>{bright}</output>
            </div>
          ) },
      { id: "media", name: "Now playing", hint: "what's playing, with play/pause", node: <MediaMini />, hiddenByDefault: true },
      { id: "battery", name: "Battery", node:     power?.percent != null && (
            <button className="battery" onClick={() => openWin("battery")} title="Battery settings">
              <span className="bigbatt">
                <i style={{ width: `${power.percent}%` }} className={power.percent <= 20 && !power.plugged ? "low" : ""} />
              </span>
              <b>{power.percent}%</b>
              <small>
                {power.charging ? "Charging" : power.plugged ? "Plugged in" : power.secs_left ? `${hm(power.secs_left)} left` : "On battery"}
              </small>
            </button>
          ) },
    ],
    opened,
  );

  if (page === "sound") return <SoundPage back={() => setPage("")} />;

  if (page === "project") {
    return (
      <div className="view quick">
        <header>
          <button className="x back" title="Back" onClick={() => setPage("")}>
            ‹
          </button>
          <b>Project</b>
        </header>
        <div className="qpage">
          {PROJECT.map(([k, name, sub]) => (
            <button
              key={k}
              className={`opt${extra?.project === k ? " on" : ""}`}
              onClick={() => {
                if (extra?.project === k) return;
                setX({ project: k });
                flip("project", undefined, k);
              }}
            >
              <i className="radio" />
              <span>
                <b>{name}</b>
                <small>{sub}</small>
              </span>
            </button>
          ))}
        </div>
        {note && <p className="note">{note}</p>}
        <footer className="qfoot">
          <button className="link" onClick={() => openWin("display")}>
            Display settings
          </button>
        </footer>
      </div>
    );
  }

  if (page === "access") {
    const tools: [keyof Extra, string, string, string][] = [
      ["magnifier", "magnifier", "Magnifier", "Zoom in on the screen"],
      ["narrator", "narrator", "Narrator", "Reads the screen aloud"],
      ["keyboard", "keyboard", "On-screen keyboard", "Type with the mouse"],
      ["captions", "captions", "Live captions", "Captions for any audio"],
    ];
    return (
      <div className="view quick">
        <header>
          <button className="x back" title="Back" onClick={() => setPage("")}>
            ‹
          </button>
          <b>Accessibility</b>
        </header>
        <div className="tiles">
          {tools.map(([field, key, name, sub]) => (
            <Tile
              key={key}
              on={!!extra?.[field]}
              icon={ICONS[key === "captions" ? "captions" : "access"]}
              name={name}
              sub={extra?.[field] ? "On" : sub}
              onClick={() => {
                setX({ [field]: !extra?.[field] } as Partial<Extra>);
                flip(key);
              }}
            />
          ))}
        </div>
        {note && <p className="note">{note}</p>}
        <footer className="qfoot">
          <button className="link" onClick={() => openWin("accessibility")}>
            Accessibility settings
          </button>
        </footer>
      </div>
    );
  }

  if (showNets) {
    return (
      <div className="view quick">
        <header>
          <button className="x back" title="Back" onClick={() => setShowNets(false)}>
            ‹
          </button>
          <b>Wi-Fi</b>
          <span className="sp" />
          <button className="ghost" onClick={() => loadNets(true)}>
            Scan
          </button>
        </header>
        <div className="scroll nets">
          {nets === null && <span className="dots" />}
          {nets?.length === 0 && <p className="empty">No networks found.</p>}
          {nets?.map((n) => (
            <button key={n.ssid} className={`net${n.connected ? " on" : ""}`} onClick={() => join(n)} title={n.saved ? "" : "Not saved: opens Windows' Wi-Fi settings to join"}>
              <WifiGlyph level={bars(n.signal)} />
              <span>
                <b>{n.ssid}</b>
                <small>
                  {joining === n.ssid ? "Working…" : n.connected ? "Connected · click to disconnect" : n.saved ? "Saved" : n.secure ? "Secured" : "Open"}
                </small>
              </span>
              {n.secure && <i title="Secured">🔒︎</i>}
            </button>
          ))}
        </div>
        {note && <p className="note">{note}</p>}
        <footer className="qfoot">
          <button className="link" onClick={() => openWin("wifi")}>
            More Wi-Fi settings
          </button>
        </footer>
      </div>
    );
  }

  return (
    <div className="view quick">
      {body}
      {note && <p className="note">{note}</p>}
      <span className="sp" />
      <footer className="qfoot">
        <button className="link" onClick={() => setEditing(!editing)}>
          {editing ? "Done" : "Edit tiles"}
        </button>
        {customize}
        <button className="link" onClick={() => openWin("settings")}>
          Settings
        </button>
      </footer>
    </div>
  );
}
