import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openAt } from "./Agents";
import { AppIcon } from "./Apps";
import type { WidgetProps } from "./registry";
import { pollActive } from "../lib/active";
import { usePolled } from "../lib/usePolled";
import {
  DEFAULT_FEED,
  DEFAULT_SHORTCUTS,
  DEFAULT_SYMBOLS,
  convert,
  pct,
  price,
  rangeSpark,
  rotation,
  shortSymbol,
  type Headline,
  type Quote,
  type Rates,
  type Shortcut,
} from "../lib/feeds";

// The everyday widgets people expect from a dock (the kind phones and Macs ship with): Stocks,
// Currency, Headlines, Photo frame and Shortcuts. Data comes from src-tauri/src/feeds.rs; each polls
// only while the dock is showing.

const isVertical = (s: WidgetProps["settings"]) => s.edge === "left" || s.edge === "right";

/** Re-render every `ms` while the dock is showing (for rotating through items). */
function useTick(ms: number, on: boolean): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!on) return;
    const stop = pollActive(ms, () => setNow(Date.now()));
    return () => stop();
  }, [ms, on]);
  return now;
}

/** Stocks: a watchlist of stocks, indexes and crypto; the cell steps through them. */
export function StocksCell({ settings, options }: WidgetProps) {
  const list = String(options.symbols ?? DEFAULT_SYMBOLS);
  const { data, error } = usePolled<Quote[]>("stocks_quotes", { list }, 60_000);
  const rotate = options.rotate !== false;
  const now = useTick(8000, rotate && (data?.length ?? 0) > 1);
  const vertical = isVertical(settings);
  const q = data?.length ? data[rotate ? rotation(data.length, now, 8000) : 0] : null;
  const tip = data?.length ? data.map((x) => `${x.name} (${x.symbol}): ${price(x.price)} ${x.currency} ${pct(x.change)}`).join("\n") : error ?? "Stocks: add symbols in Settings > Widgets (gear)";
  return (
    <button className={`cell stat stock ${q ? (q.change >= 0 ? "up" : "down") : ""}`} title={tip} onClick={(e) => openAt("stocks", e, vertical)}>
      {!q ? (
        <span className="stext">
          <small>Stocks</small>
          <b>{data ? "–" : "…"}</b>
        </span>
      ) : vertical ? (
        <span className="stext">
          <small>{shortSymbol(q.symbol)}</small>
          <b>{pct(q.change)}</b>
        </span>
      ) : (
        <>
          {options.graph !== false && q.points.length > 1 && (
            <svg className="spark" viewBox="0 0 40 22" width="40" height="22" preserveAspectRatio="none" aria-hidden="true">
              <path d={rangeSpark(q.points, 40, 22)} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" />
            </svg>
          )}
          <span className="stext">
            <small>
              {shortSymbol(q.symbol)} <em>{pct(q.change)}</em>
            </small>
            <b>{price(q.price)}</b>
          </span>
        </>
      )}
    </button>
  );
}

/** Currency: today's rate between two currencies; click to convert any amount. */
export function CurrencyCell({ settings, options }: WidgetProps) {
  const from = String(options.from ?? "USD").toUpperCase();
  const to = String(options.to ?? "EUR").toUpperCase();
  const { data, error } = usePolled<Rates>("fx_rates", { base: from }, 30 * 60_000);
  const rate = convert(1, from, to, data);
  const vertical = isVertical(settings);
  const tip = rate !== null ? `1 ${from} = ${price(rate)} ${to} (European Central Bank, ${data?.date})` : error ?? "Currency";
  return (
    <button className="cell stat fx" title={tip} onClick={(e) => openAt("currency", e, vertical)}>
      <span className="stext">
        <small>{vertical ? to : `${from} → ${to}`}</small>
        <b>{rate === null ? (data || error ? "–" : "…") : price(rate)}</b>
      </span>
    </button>
  );
}

/** Headlines: the latest from a news feed, one at a time; click for the list. */
export function HeadlinesCell({ settings, options }: WidgetProps) {
  const url = String(options.feed ?? DEFAULT_FEED);
  const { data, error } = usePolled<Headline[]>("news_headlines", { url }, 15 * 60_000);
  const now = useTick(10_000, (data?.length ?? 0) > 1);
  const vertical = isVertical(settings);
  const h = data?.length ? data[rotation(Math.min(data.length, 8), now, 10_000)] : null;
  const wide = options.width === "wide";
  return (
    <button className={`cell news${wide ? " wide" : ""}`} title={h ? h.title : (error ?? "Headlines")} onClick={(e) => openAt("news", e, vertical)}>
      <svg viewBox="0 0 20 20" width="17" height="17" aria-hidden="true">
        <path d="M4 4h10v12H5a1 1 0 0 1-1-1zM14 7h2v8a1 1 0 0 1-2 0M6.5 7h5M6.5 10h5M6.5 13h3" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
      {!vertical && <span className="ntitle">{h ? h.title : error ? "Feed unavailable" : "Loading headlines…"}</span>}
    </button>
  );
}

/** Photo frame: a picture from your Pictures (or any folder), changing every few minutes. Click
 *  opens it; scroll for the next or previous one. */
export function PhotoCell({ options }: WidgetProps) {
  const every = Math.max(1, Number(options.every ?? 15)) * 60_000;
  const tick = useTick(every, true);
  const [step, setStep] = useState(0);
  const n = Math.floor(tick / every) + step;
  const folder = String(options.folder ?? "");
  const [photo, setPhoto] = useState<{ path: string; url: string } | null | undefined>(undefined);
  useEffect(() => {
    let alive = true;
    invoke<{ path: string; url: string } | null>("photo_frame", { folder, n: Math.abs(n * 7919), size: 160 })
      .then((p) => alive && setPhoto(p))
      .catch(() => alive && setPhoto(null));
    return () => {
      alive = false;
    };
  }, [folder, n]);
  const wide = options.shape === "wide";
  return (
    <button
      className={`cell photo${wide ? " wide" : ""}`}
      title={photo ? `${photo.path}\nScroll for another photo` : "Photo frame: no photos found (pick a folder in Settings > Widgets, gear)"}
      aria-label="Photo frame"
      onClick={() => photo && invoke("start_open", { target: photo.path }).catch(() => {})}
      onWheel={(e) => setStep((s) => s + (e.deltaY > 0 ? 1 : -1))}
    >
      {photo ? <img src={photo.url} alt="" draggable={false} /> : <span className="glyph">{photo === undefined ? "…" : "🖼"}</span>}
    </button>
  );
}

/** Shortcuts: your own buttons for folders, files, apps, web pages and Settings pages. */
export function ShortcutsCell({ options }: WidgetProps) {
  const spec = String(options.items ?? DEFAULT_SHORTCUTS);
  const [items, setItems] = useState<Shortcut[]>([]);
  useEffect(() => {
    invoke<Shortcut[]>("shortcuts_list", { spec }).then(setItems).catch(() => setItems([]));
  }, [spec]);
  if (!items.length) return <div className="cell" title="Shortcuts: add some in Settings > Widgets (gear)">+</div>;
  return (
    <div className="cell group shortcuts">
      {items.map((s) => (
        <button key={s.name + s.target} className="scbtn" title={`${s.name}\n${s.target}`} aria-label={s.name} onClick={() => invoke("shortcut_open", { target: s.target }).catch(() => {})}>
          {s.kind === "web" ? (
            <span className="glyph">{s.name.slice(0, 1).toUpperCase()}</span>
          ) : s.kind === "settings" ? (
            <span className="glyph">⚙</span>
          ) : (
            <AppIcon src={s.target} name={s.name} />
          )}
        </button>
      ))}
    </div>
  );
}
