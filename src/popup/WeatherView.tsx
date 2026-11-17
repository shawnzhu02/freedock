import type { Settings } from "../settings";
import { useKept } from "../lib/kept";
import { describe, hm, use12h } from "../lib/weather";
import { unitOf, useWeather, WxIcon } from "../widgets/Weather";
import { useSections, type Section } from "./Sections";
import { useEffect } from "react";
import { aqiBand, getAir, uvLabel, type Air } from "../lib/air";

const dayName = (date: string, i: number) =>
  i === 0 ? "Today" : new Date(`${date}T12:00`).toLocaleDateString(undefined, { weekday: "short" });

/** The weather flyout: now, the next 24 hours and the week, with sunrise and sunset; Customize
 * picks and orders the parts. */
export default function WeatherView({ settings, opened }: { settings: Settings; opened?: number }) {
  const slot = settings.widgets.find((w) => w.id === "weather");
  const unit = unitOf(slot?.options ?? {});
  const { w, err } = useWeather(settings.lat, settings.lon, unit);
  const h12 = use12h(settings.hour12);
  const [air, setAir] = useKept<Air | null>("weather.air", null);
  useEffect(() => {
    getAir(settings.lat, settings.lon).then(setAir).catch(() => {});
  }, [settings.lat, settings.lon, opened]);
  const sections: Section[] = [];
  if (w) {
    const d = describe(w.code);
    const lo = Math.min(...w.days.map((x) => x.min));
    const hi = Math.max(...w.days.map((x) => x.max));
    const span = Math.max(1, hi - lo);
    const today = w.days[0];
    sections.push(
      {
        id: "now",
        name: "Now",
        hint: "temperature, feels like, wind and humidity",
        node: (
          <div className="wnow">
            <WxIcon kind={d.kind} day={w.day} size={54} />
            <div className="wbig">
              <b>{Math.round(w.temp)}°</b>
              <span>{d.label}</span>
            </div>
            <div className="wmeta">
              <b>{settings.city}</b>
              <span>Feels like {Math.round(w.feels)}°</span>
              <span>
                {Math.round(w.wind)} {unit === "f" ? "mph" : "km/h"} · {Math.round(w.humidity)}% humidity
              </span>
            </div>
          </div>
        ),
      },
      {
        id: "hours",
        name: "Hourly",
        hint: "the next 24 hours",
        node: (
          <div
            className="hours"
            onWheel={(e) => {
              // A plain wheel scrolls the hours sideways.
              if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) e.currentTarget.scrollLeft += e.deltaY;
            }}
          >
            {w.hours.map((x, i) => (
              <div className="hour" key={x.time}>
                <small>{i === 0 ? "Now" : hm(x.time, h12).replace(":00", "")}</small>
                <WxIcon kind={describe(x.code).kind} day={x.day} size={20} />
                <b>{Math.round(x.temp)}°</b>
                {x.rain >= 20 && <i>{x.rain}%</i>}
              </div>
            ))}
          </div>
        ),
      },
      {
        id: "days",
        name: "This week",
        hint: "highs, lows and rain chance",
        node: (
          <div className="days">
            {w.days.map((x, i) => (
              <div className="day" key={x.date}>
                <span className="dname">{dayName(x.date, i)}</span>
                <WxIcon kind={describe(x.code).kind} size={18} />
                <i className="drain">{x.rain >= 20 ? `${x.rain}%` : ""}</i>
                <span className="dmin">{Math.round(x.min)}°</span>
                <span className="drange">
                  <em style={{ left: `${((x.min - lo) / span) * 100}%`, right: `${100 - ((x.max - lo) / span) * 100}%` }} />
                </span>
                <span className="dmax">{Math.round(x.max)}°</span>
              </div>
            ))}
          </div>
        ),
      },
      {
        id: "air",
        name: "Air quality and UV",
        hint: "US AQI, fine particles, UV index",
        node: air ? (
          <div className="airrow" title="US Air Quality Index from Open-Meteo">
            <b className={`aqi ${aqiBand(air.aqi).tone}`}>{air.aqi}</b>
            <span>
              <b>Air {aqiBand(air.aqi).label.toLowerCase()}</b>
              <small>PM2.5 {air.pm25.toFixed(1)} µg/m³</small>
            </span>
            <span>
              <b>UV {air.uv.toFixed(0)}</b>
              <small>{uvLabel(air.uv)}</small>
            </span>
          </div>
        ) : null,
      },
      {
        id: "sun",
        name: "Sunrise and sunset",
        node: today ? (
          <div className="suns">
            <span>Sunrise {hm(today.sunrise, h12)}</span>
            <span>Sunset {hm(today.sunset, h12)}</span>
          </div>
        ) : null,
      },
    );
  }
  const { body, button } = useSections("weather", sections, opened);
  if (!w) {
    return (
      <div className="view weatherv">
        <p className="empty center">{err ? `Couldn't load the weather: ${err}` : "Loading…"}</p>
      </div>
    );
  }
  return (
    <div className="view weatherv">
      {body}
      <footer className="qfoot">
        {button}
        <small className="dim">Open-Meteo</small>
      </footer>
    </div>
  );
}
