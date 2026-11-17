import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { describe, getWeather, hm, localIso, nextSun, rainSoon, use12h, type Kind, type Weather } from "../lib/weather";
import { isActive } from "../lib/active";

/** Our own weather glyphs, drawn to sit with the dock's line icons. */
export function WxIcon({ kind, day = true, size = 22 }: { kind: Kind; day?: boolean; size?: number }) {
  const sun = (
    <g className="wx-sun">
      <circle cx="12" cy="12" r="4.2" />
      {[0, 45, 90, 135, 180, 225, 270, 315].map((a) => (
        <line key={a} x1="12" y1="3.2" x2="12" y2="5.4" transform={`rotate(${a} 12 12)`} />
      ))}
    </g>
  );
  const moon = <path className="wx-moon" d="M15.5 4.5a7.5 7.5 0 1 0 4 12.8A6.2 6.2 0 0 1 15.5 4.5z" />;
  const cloud = (dx = 0, dy = 0) => (
    <path className="wx-cloud" transform={`translate(${dx} ${dy})`} d="M7.2 19h9.6a3.9 3.9 0 0 0 .4-7.8 5.3 5.3 0 0 0-10.1 1.5A3.2 3.2 0 0 0 7.2 19z" />
  );
  const drops = (n: number, snow = false) =>
    Array.from({ length: n }, (_, i) =>
      snow ? (
        <circle key={i} className="wx-snow" cx={8 + i * 4} cy={21.5} r="1.1" />
      ) : (
        <line key={i} className="wx-rain" x1={9 + i * 3.5} y1="20" x2={8 + i * 3.5} y2="23" />
      ),
    );
  let body;
  switch (kind) {
    case "clear":
      body = day ? sun : moon;
      break;
    case "partly":
      body = (
        <>
          <g transform="translate(-3 -3) scale(.8)">{day ? sun : moon}</g>
          {cloud(2, 1)}
        </>
      );
      break;
    case "cloudy":
      body = cloud(0, 0);
      break;
    case "fog":
      body = (
        <g className="wx-fog">
          <line x1="4" y1="9" x2="20" y2="9" />
          <line x1="2.5" y1="13" x2="18.5" y2="13" />
          <line x1="5" y1="17" x2="21" y2="17" />
        </g>
      );
      break;
    case "drizzle":
      body = (
        <>
          {cloud(0, -3)}
          {drops(2)}
        </>
      );
      break;
    case "rain":
      body = (
        <>
          {cloud(0, -3)}
          {drops(3)}
        </>
      );
      break;
    case "snow":
      body = (
        <>
          {cloud(0, -3)}
          {drops(3, true)}
        </>
      );
      break;
    case "storm":
      body = (
        <>
          {cloud(0, -3)}
          <path className="wx-bolt" d="M12.5 15.5l-2.5 4h2.6l-1.6 3.5 4.2-5h-2.6l1.4-2.5z" />
        </>
      );
      break;
  }
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} className="wx" aria-hidden="true">
      {body}
    </svg>
  );
}

/** The forecast for the city in settings, refreshed every 15 minutes. */
export function useWeather(lat: number, lon: number, unit: "f" | "c") {
  const [w, setW] = useState<Weather | null>(null);
  const [err, setErr] = useState("");
  useEffect(() => {
    let alive = true;
    const load = () =>
      getWeather(lat, lon, unit)
        .then((x) => alive && (setW(x), setErr("")))
        .catch((e) => alive && setErr(String(e?.message ?? e)));
    load();
    const t = window.setInterval(() => isActive() && load(), 5 * 60_000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [lat, lon, unit]);
  return { w, err };
}

export const unitOf = (o: Record<string, unknown>): "f" | "c" => (o.unit === "c" ? "c" : "f");

/** True until the user has picked a city in Settings. */
export const noCity = (s: { city: string; lat: number; lon: number }) => !s.city && s.lat === 0 && s.lon === 0;

export default function WeatherCell(props: WidgetProps) {
  if (noCity(props.settings)) {
    return (
      <button className="cell weather" title="Pick your city in Settings" onClick={() => invoke("open_settings", { at: "General#weather" })}>
        <WxIcon kind="cloudy" />
        <span className="wtext">
          <small>Set city</small>
        </span>
      </button>
    );
  }
  return <WeatherReady {...props} />;
}

function WeatherReady({ settings, options }: WidgetProps) {
  const unit = unitOf(options);
  const { w, err } = useWeather(settings.lat, settings.lon, unit);
  const vertical = settings.edge === "left" || settings.edge === "right";
  const d = w ? describe(w.code) : null;
  const today = w?.days[0];
  const soon = w && options.rainAlert !== false ? rainSoon(w) : null;
  const soonText = soon ? `${soon.kind === "snow" ? "Snow" : "Rain"} likely around ${hm(soon.time, use12h(settings.hour12))} (${soon.chance}%)` : "";
  return (
    <button
      className={`cell weather${soon ? " wet" : ""}`}
      title={err ? `Weather: ${err}` : w && d ? `${settings.city}: ${d.label}, feels like ${Math.round(w.feels)}°${soon ? `. ${soonText}` : ""}` : settings.city}
      onClick={(e) => openAt("weather", e, vertical)}
    >
      {w && d ? <WxIcon kind={d.kind} day={w.day} /> : <WxIcon kind="cloudy" />}
      {soon && (
        <span className="wsoon" aria-label={soonText}>
          <svg viewBox="0 0 16 16" width="11" height="11" aria-hidden="true">
            <path d="M8 2a6 6 0 0 1 6 6H2a6 6 0 0 1 6-6zM8 8v4.5a1.5 1.5 0 0 1-3 0" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </span>
      )}
      <span className="wtext">
        <b>{w ? `${Math.round(w.temp)}°` : "…"}</b>
        {!vertical && options.range !== false && today && (
          <small>
            {Math.round(today.max)}° / {Math.round(today.min)}°
          </small>
        )}
      </span>
    </button>
  );
}

/** Next sunrise or sunset, with a small sun glyph. */
export function SunCell(props: WidgetProps) {
  return noCity(props.settings) ? <WeatherCell {...props} /> : <SunReady {...props} />;
}

function SunReady({ settings }: WidgetProps) {
  const { w } = useWeather(settings.lat, settings.lon, "f");
  const vertical = settings.edge === "left" || settings.edge === "right";
  const [now, setNow] = useState(localIso());
  useEffect(() => {
    const t = window.setInterval(() => setNow(localIso()), 60_000);
    return () => clearInterval(t);
  }, []);
  const next = w ? nextSun(w.days, now) : null;
  return (
    <button className="cell weather sun" title={next ? `${next.what === "sunrise" ? "Sunrise" : "Sunset"} at ${hm(next.at, use12h(settings.hour12))}` : "Sunrise & sunset"} onClick={(e) => openAt("weather", e, vertical)}>
      <svg viewBox="0 0 24 24" width="22" height="22" className="wx" aria-hidden="true">
        <g className="wx-sun">
          <path d="M6.5 16a5.5 5.5 0 0 1 11 0" />
          <line x1="3" y1="16" x2="21" y2="16" />
          <line x1="12" y1="5" x2="12" y2="7.5" />
          <line x1="5.2" y1="9.2" x2="6.9" y2="10.9" />
          <line x1="18.8" y1="9.2" x2="17.1" y2="10.9" />
        </g>
        <path className="wx-arrow" d={next?.what === "sunrise" ? "M12 22v-3.5m-2 1.6 2-2 2 2" : "M12 18.5V22m-2-1.6 2 2 2-2"} />
      </svg>
      {!vertical && (
        <span className="wtext">
          <small>{next ? (next.what === "sunrise" ? "Sunrise" : "Sunset") : "Sun"}</small>
          <b>{next ? hm(next.at, use12h(settings.hour12)) : "…"}</b>
        </span>
      )}
    </button>
  );
}
