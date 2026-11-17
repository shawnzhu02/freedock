import { useEffect, useMemo, useState } from "react";
import { useKept } from "../lib/kept";
import { press } from "../lib/press";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../settings";
import { usePolled } from "../lib/usePolled";
import { DEFAULT_FEED, DEFAULT_SYMBOLS, convert, pct, price, rangeSpark, type Headline, type Quote, type Rates } from "../lib/feeds";

// Flyouts for the everyday widgets (src/widgets/Everyday.tsx): the Stocks watchlist, a currency
// converter, and a feed's headlines. Each reads its widget's options from the saved settings.

/** The options of the first widget with this id on the dock, once loaded. */
function useOptions(id: string, opened: number): Record<string, unknown> | null {
  const [o, setO] = useKept<Record<string, unknown> | null>(`options.${id}`, null);
  useEffect(() => {
    invoke<Settings>("get_settings")
      .then((s) => setO((s.widgets.find((w) => w.id === id)?.options as Record<string, unknown>) ?? {}))
      .catch(() => setO({}));
  }, [id, opened]);
  return o;
}

const openLink = (url: string) => invoke("open_link", { url }).then(() => invoke("popup_close")).catch(() => {});

export function StocksView({ opened }: { opened: number }) {
  const o = useOptions("stocks", opened);
  return o === null ? <div className="view stsv" /> : <StocksList list={String(o.symbols ?? DEFAULT_SYMBOLS)} />;
}

function StocksList({ list }: { list: string }) {
  const { data, error } = usePolled<Quote[]>("stocks_quotes", { list }, 60_000);
  return (
    <div className="view stsv">
      <header>
        <b>Stocks</b>
        <span className="sp" />
        <small className="ghlogin">Today · Yahoo Finance</small>
      </header>
      {!data && !error && <p className="empty center">Loading…</p>}
      {error && !data && <p className="empty center">Couldn't get quotes: {error}</p>}
      {data && !data.length && <p className="empty center">No quotes. Check the symbols in the widget's settings (like AAPL, ^GSPC, BTC-USD).</p>}
      {data && data.length > 0 && (
        <div className="ghbody">
          {data.map((q) => (
            <div className={`strow quote ${q.change >= 0 ? "up" : "down"}`} key={q.symbol} title={`Open ${q.symbol} on Yahoo Finance`} {...press(() => openLink(`https://finance.yahoo.com/quote/${encodeURIComponent(q.symbol)}`))}>
              <div className="fmeta">
                <b>{q.symbol}</b>
                <small>{q.name}</small>
              </div>
              {q.points.length > 1 && (
                <svg className="qspark" viewBox="0 0 70 26" width="70" height="26" preserveAspectRatio="none" aria-hidden="true">
                  <path d={rangeSpark(q.points, 70, 26)} fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
                </svg>
              )}
              <div className="qnum">
                <b>{price(q.price)}</b>
                <small>{pct(q.change)}</small>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function CurrencyView({ opened }: { opened: number }) {
  const o = useOptions("currency", opened);
  return o === null ? <div className="view stsv" /> : <Converter from0={String(o.from ?? "USD").toUpperCase()} to0={String(o.to ?? "EUR").toUpperCase()} opened={opened} />;
}

function Converter({ from0, to0, opened }: { from0: string; to0: string; opened: number }) {
  const [from, setFrom] = useState(from0);
  const [to, setTo] = useState(to0);
  const [amount, setAmount] = useState("1");
  useEffect(() => {
    setFrom(from0);
    setTo(to0);
    setAmount("1");
  }, [from0, to0, opened]);
  const { data, error } = usePolled<Rates>("fx_rates", { base: from }, 30 * 60_000);
  const codes = useMemo(() => (data ? [data.base, ...Object.keys(data.rates)].sort() : [from, to]), [data, from, to]);
  const n = Number(amount.replace(/,/g, ""));
  const out = convert(n, from, to, data);
  return (
    <div className="view stsv fxv">
      <header>
        <b>Currency</b>
        <span className="sp" />
        <small className="ghlogin">{data ? `ECB rates, ${data.date}` : ""}</small>
      </header>
      <div className="fxrow">
        <input className="psearch" inputMode="decimal" aria-label="Amount" value={amount} onChange={(e) => setAmount(e.target.value)} autoFocus />
        <select aria-label="From currency" value={from} onChange={(e) => setFrom(e.target.value)}>
          {codes.map((c) => (
            <option key={c}>{c}</option>
          ))}
        </select>
      </div>
      <button className="ghost fxswap" title="Swap" aria-label="Swap currencies" onClick={() => (setFrom(to), setTo(from))}>
        ⇅
      </button>
      <div className="fxrow">
        <output className="fxout">{out === null ? (error ? "–" : "…") : price(out)}</output>
        <select aria-label="To currency" value={to} onChange={(e) => setTo(e.target.value)}>
          {codes.map((c) => (
            <option key={c}>{c}</option>
          ))}
        </select>
      </div>
      {error && <p className="empty">Couldn't get rates: {error}</p>}
    </div>
  );
}

export function NewsView({ opened }: { opened: number }) {
  const o = useOptions("news", opened);
  return o === null ? <div className="view stsv" /> : <NewsList url={String(o.feed ?? DEFAULT_FEED)} />;
}

function NewsList({ url }: { url: string }) {
  const { data, error } = usePolled<Headline[]>("news_headlines", { url }, 15 * 60_000);
  let host = "";
  try {
    host = new URL(url).hostname.replace(/^www\./, "");
  } catch {
    host = "";
  }
  return (
    <div className="view stsv">
      <header>
        <b>Headlines</b>
        <span className="sp" />
        <small className="ghlogin">{host}</small>
      </header>
      {!data && !error && <p className="empty center">Loading…</p>}
      {error && <p className="empty center">{error}</p>}
      {data && (
        <div className="ghbody">
          {data.map((h) => (
            <div className="strow news" key={h.link} title={h.link} {...press(() => openLink(h.link))}>
              <span className="ntext">{h.title}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
