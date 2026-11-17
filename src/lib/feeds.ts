// Helpers for the everyday widgets (Stocks, Currency, Headlines, Photo frame, Shortcuts; the data comes
// from src-tauri/src/feeds.rs). The pure parts are tested in feeds.test.ts.

export type Quote = { symbol: string; name: string; price: number; change: number; currency: string; points: number[] };
export type Rates = { base: string; date: string; rates: Record<string, number> };
export type Headline = { title: string; link: string };
export type Shortcut = { name: string; target: string; kind: "web" | "settings" | "app" | "path" };

export const DEFAULT_SYMBOLS = "^GSPC, AAPL, BTC-USD";
export const DEFAULT_FEED = "https://feeds.bbci.co.uk/news/rss.xml";
export const DEFAULT_SHORTCUTS = "Documents = %USERPROFILE%\\Documents; Downloads = %USERPROFILE%\\Downloads; Settings = ms-settings:";

/** A sparkline path scaled to the values' own range (prices don't start at zero). */
export function rangeSpark(values: number[], w: number, h: number): string {
  if (values.length < 2) return "";
  const lo = Math.min(...values);
  const hi = Math.max(...values);
  const span = hi - lo || 1;
  const step = w / (values.length - 1);
  return values.map((v, i) => `${i ? "L" : "M"}${(i * step).toFixed(1)} ${(h - 1 - ((v - lo) / span) * (h - 2)).toFixed(1)}`).join(" ");
}

/** "+1.24%" / "-0.50%" / "0.00%". */
export const pct = (n: number) => `${n > 0 ? "+" : ""}${n.toFixed(2)}%`;

/** A price with sensible decimals: 64,895 / 352.42 / 1.0842 / 0.000012. */
export function price(n: number): string {
  const abs = Math.abs(n);
  const digits = abs >= 1000 ? 0 : abs >= 10 ? 2 : abs >= 0.01 ? 4 : 6;
  return n.toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

/** Ticker as people say it: "^GSPC" -> "S&P 500" style names come from the quote; this only trims. */
export const shortSymbol = (s: string) => s.replace(/^\^/, "").replace(/-USD$/, "").replace(/=X$/, "");

/** Convert `amount` of `from` into `to`, given rates based on `from`. Null when a rate is missing. */
export function convert(amount: number, from: string, to: string, r: Rates | null): number | null {
  if (!r || !Number.isFinite(amount)) return null;
  if (from === to) return amount;
  if (r.base !== from) return null;
  const rate = r.rates[to];
  return rate === undefined ? null : amount * rate;
}

/** Index of the item to show now when rotating through `n` items every `everyMs`. */
export const rotation = (n: number, now: number, everyMs: number) => (n > 0 ? Math.floor(now / everyMs) % n : 0);
