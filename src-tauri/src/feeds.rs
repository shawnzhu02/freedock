//! Data for the everyday widgets: Stocks, Currency, Headlines, Photo frame and Shortcuts.
//!
//! Everything here is key-free and goes straight from this PC to the source, with no freedock server
//! in between:
//! - Stocks: Yahoo Finance's public chart endpoint (no key or account; stocks, indexes like ^GSPC,
//!   crypto like BTC-USD, currencies like EURUSD=X). Cached a minute per symbol.
//! - Currency: frankfurter.dev, the European Central Bank's daily reference rates. Cached an hour.
//! - Headlines: any RSS or Atom feed the user picks. Cached 15 minutes.
//! - Photo frame: pictures from a local folder (Pictures by default), never uploaded.
//! - Shortcuts: a list of things to open that the user writes in the widget's options.
//!
//! The parsing is pure and tested below; network calls are small `ureq` GETs with a timeout.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .redirects(5)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) freedock/0.1")
        .build()
}

/// GET a URL as text, at most 2 MB (a feed or a quote is far smaller; this caps a bad server).
fn get_text(url: &str) -> Result<String, String> {
    use std::io::Read;
    let resp = agent().get(url).call().map_err(|e| match e {
        ureq::Error::Status(c, _) => format!("the server answered {c}"),
        e => format!("couldn't reach it ({})", e.kind()),
    })?;
    let mut s = String::new();
    resp.into_reader()
        .take(2_000_000)
        .read_to_string(&mut s)
        .map_err(|_| "the answer wasn't text".to_string())?;
    Ok(s)
}

/// A small time-limited cache keyed by string.
struct Cache<T> {
    map: Mutex<Option<HashMap<String, (Instant, T)>>>,
}
impl<T: Clone> Cache<T> {
    const fn new() -> Self {
        Cache {
            map: Mutex::new(None),
        }
    }
    fn get(&self, k: &str, ttl: Duration) -> Option<T> {
        self.map
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()?
            .get(k)
            .filter(|(at, _)| at.elapsed() < ttl)
            .map(|(_, v)| v.clone())
    }
    fn put(&self, k: &str, v: T) {
        let mut g = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let m = g.get_or_insert_with(HashMap::new);
        if m.len() > 200 {
            m.clear();
        }
        m.insert(k.to_string(), (Instant::now(), v));
    }
}

// ------------------------------------------------------------------ stocks

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Quote {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    /// Percent change since the previous close.
    pub change: f64,
    pub currency: String,
    /// Today's prices, oldest first, for the sparkline.
    pub points: Vec<f64>,
}

/// The symbols in "AAPL, msft ^GSPC; BTC-USD": upper-cased, only characters tickers use, at most 12.
pub fn symbols(list: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in list.split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
        let s = s.trim().to_uppercase();
        if !s.is_empty()
            && s.len() <= 15
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "^.=-".contains(c))
            && !out.contains(&s)
        {
            out.push(s);
        }
    }
    out.truncate(12);
    out
}

/// Read one symbol's quote from Yahoo's chart JSON.
pub fn parse_chart(symbol: &str, json: &str) -> Option<Quote> {
    let v: Value = serde_json::from_str(json).ok()?;
    let r = v.pointer("/chart/result/0")?;
    let meta = r.get("meta")?;
    let price = meta.get("regularMarketPrice")?.as_f64()?;
    let prev = meta
        .get("chartPreviousClose")
        .or_else(|| meta.get("previousClose"))
        .and_then(Value::as_f64)
        .filter(|p| *p > 0.0);
    let change = prev.map(|p| (price - p) / p * 100.0).unwrap_or(0.0);
    let name = ["shortName", "longName"]
        .iter()
        .find_map(|k| meta.get(*k).and_then(Value::as_str))
        .unwrap_or(symbol)
        .to_string();
    let points = r
        .pointer("/indicators/quote/0/close")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    Some(Quote {
        symbol: symbol.to_string(),
        name,
        price,
        change,
        currency: meta
            .get("currency")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        points,
    })
}

static QUOTES: Cache<Quote> = Cache::new();

/// Quotes for a list of symbols (unknown ones are left out). Blocking; call off the UI thread.
pub fn quotes(list: &str) -> Vec<Quote> {
    symbols(list)
        .into_iter()
        .filter_map(|s| {
            if let Some(q) = QUOTES.get(&s, Duration::from_secs(60)) {
                return Some(q);
            }
            let url = format!(
                "https://query1.finance.yahoo.com/v8/finance/chart/{}?range=1d&interval=15m",
                crate::startmenu::url_encode(&s)
            );
            let q = parse_chart(&s, &get_text(&url).ok()?)?;
            QUOTES.put(&s, q.clone());
            Some(q)
        })
        .collect()
}

// ------------------------------------------------------------------ currency

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Rates {
    pub base: String,
    /// The day the central bank published them (YYYY-MM-DD).
    pub date: String,
    pub rates: HashMap<String, f64>,
}

/// A three-letter currency code, upper-cased, or None.
pub fn currency_code(c: &str) -> Option<String> {
    let c = c.trim().to_uppercase();
    (c.len() == 3 && c.chars().all(|x| x.is_ascii_alphabetic())).then_some(c)
}

pub fn parse_rates(json: &str) -> Option<Rates> {
    let v: Value = serde_json::from_str(json).ok()?;
    let base = v.get("base")?.as_str()?.to_string();
    let rates: HashMap<String, f64> = v
        .get("rates")?
        .as_object()?
        .iter()
        .filter_map(|(k, x)| Some((k.clone(), x.as_f64()?)))
        .collect();
    Some(Rates {
        date: v
            .get("date")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        rates,
        base,
    })
}

static RATES: Cache<Rates> = Cache::new();

pub fn rates(base: &str) -> Result<Rates, String> {
    let base = currency_code(base).ok_or("Pick a three-letter currency code, like USD.")?;
    if let Some(r) = RATES.get(&base, Duration::from_secs(3600)) {
        return Ok(r);
    }
    let r = parse_rates(&get_text(&format!(
        "https://api.frankfurter.dev/v1/latest?base={base}"
    ))?)
    .ok_or("The exchange rates were unreadable.")?;
    RATES.put(&base, r.clone());
    Ok(r)
}

// ------------------------------------------------------------------ headlines

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Headline {
    pub title: String,
    pub link: String,
}

fn decode(s: &str) -> String {
    let s = s.trim();
    let s = s
        .strip_prefix("<![CDATA[")
        .and_then(|x| x.strip_suffix("]]>"))
        .unwrap_or(s);
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(j) = tail.find(';').filter(|j| *j <= 10) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let ent = &tail[1..j];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            e if e.starts_with("#x") || e.starts_with("#X") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &tail[j + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    // Titles sometimes carry markup; keep only the text.
    let mut text = String::with_capacity(out.len());
    let mut in_tag = false;
    for c in out.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn inner<'a>(block: &'a str, tag: &str) -> Option<&'a str> {
    let open = block.find(&format!("<{tag}"))?;
    let after = &block[open + tag.len() + 1..];
    // "<title>" or "<title type=...>", not "<titles>".
    if !after.starts_with('>') && !after.starts_with(' ') && !after.starts_with('/') {
        return None;
    }
    let start = after.find('>')? + 1;
    if after[..start].ends_with("/>") {
        return Some("");
    }
    let end = after[start..].find(&format!("</{tag}>"))? + start;
    Some(&after[start..end])
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let i = tag.find(&format!("{name}=\""))? + name.len() + 2;
    Some(&tag[i..i + tag[i..].find('"')?])
}

/// Headlines from an RSS (`<item>`) or Atom (`<entry>`) feed, newest first as the feed lists them.
/// Only web links are kept.
pub fn parse_feed(xml: &str, n: usize) -> Vec<Headline> {
    let (open, close) = if xml.contains("<item") {
        ("<item", "</item>")
    } else {
        ("<entry", "</entry>")
    };
    let mut out = Vec::new();
    for block in xml.split(open).skip(1) {
        let block = block.split(close).next().unwrap_or("");
        let Some(title) = inner(block, "title").map(decode).filter(|t| !t.is_empty()) else {
            continue;
        };
        let link = match inner(block, "link").map(decode).filter(|l| !l.is_empty()) {
            Some(l) => l,
            // Atom: <link rel="alternate" href="..."/>
            None => block
                .match_indices("<link")
                .filter_map(|(i, _)| block[i..].find('>').map(|j| &block[i..i + j]))
                .find(|t| !t.contains("rel=\"") || t.contains("rel=\"alternate\""))
                .and_then(|t| attr(t, "href"))
                .map(decode)
                .unwrap_or_default(),
        };
        if link.starts_with("https://") || link.starts_with("http://") {
            out.push(Headline { title, link });
        }
        if out.len() >= n {
            break;
        }
    }
    out
}

static FEEDS: Cache<Vec<Headline>> = Cache::new();

pub fn headlines(url: &str) -> Result<Vec<Headline>, String> {
    let url = url.trim();
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("The feed link must start with https://".into());
    }
    if let Some(h) = FEEDS.get(url, Duration::from_secs(900)) {
        return Ok(h);
    }
    let h = parse_feed(&get_text(url)?, 20);
    if h.is_empty() {
        return Err("That link isn't an RSS or Atom feed.".into());
    }
    FEEDS.put(url, h.clone());
    Ok(h)
}

// ------------------------------------------------------------------ photo frame

pub fn is_photo(path: &str) -> bool {
    let p = path.to_lowercase();
    [
        ".jpg", ".jpeg", ".png", ".webp", ".gif", ".bmp", ".heic", ".avif",
    ]
    .iter()
    .any(|e| p.ends_with(e))
}

/// Photos under `dir`, three folders deep at most, up to 3000, skipping hidden and system folders.
pub fn photos_in(dir: &std::path::Path) -> Vec<String> {
    fn walk(d: &std::path::Path, depth: u32, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            if out.len() >= 3000 {
                return;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name.starts_with('$') {
                continue;
            }
            let Ok(t) = e.file_type() else { continue };
            let p = e.path();
            if t.is_dir() && depth < 3 {
                walk(&p, depth + 1, out);
            } else if t.is_file() && is_photo(&name) {
                out.push(p.to_string_lossy().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, 0, &mut out);
    out.sort();
    out
}

static PHOTOS: Cache<std::sync::Arc<Vec<String>>> = Cache::new();

/// The `n`th photo (wrapping) of a folder; "" means the user's Pictures folder.
pub fn photo(folder: &str, n: u64) -> Option<String> {
    let dir = if folder.trim().is_empty() {
        pictures()?
    } else {
        std::path::PathBuf::from(folder.trim())
    };
    if !dir.is_absolute() || !dir.is_dir() {
        return None;
    }
    let key = dir.to_string_lossy().to_string();
    let list = PHOTOS
        .get(&key, Duration::from_secs(600))
        .unwrap_or_else(|| {
            let l = std::sync::Arc::new(photos_in(&dir));
            PHOTOS.put(&key, l.clone());
            l
        });
    (!list.is_empty()).then(|| list[(n % list.len() as u64) as usize].clone())
}

fn pictures() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::Shell::{
            FOLDERID_Pictures, KNOWN_FOLDER_FLAG, SHGetKnownFolderPath,
        };
        if let Ok(p) = SHGetKnownFolderPath(&FOLDERID_Pictures, KNOWN_FOLDER_FLAG(0), None) {
            let s = p.to_string().ok();
            windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
            if let Some(s) = s {
                return Some(s.into());
            }
        }
    }
    std::env::var_os("USERPROFILE").map(|h| std::path::Path::new(&h).join("Pictures"))
}

// ------------------------------------------------------------------ shortcuts

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Shortcut {
    pub name: String,
    /// With %VARIABLES% filled in.
    pub target: String,
    /// "web", "settings", "app" (shell:AppsFolder) or "path".
    pub kind: String,
}

/// Fill in %VARIABLES% (like %USERPROFILE%); unknown ones are left as written.
pub fn expand(s: &str, var: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        match tail.find('%') {
            Some(j)
                if j > 0
                    && tail[..j]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_') =>
            {
                match var(&tail[..j]) {
                    Some(v) => out.push_str(&v),
                    None => out.push_str(&rest[i..i + j + 2]),
                }
                rest = &tail[j + 1..];
            }
            _ => {
                out.push('%');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// "Name = target" items, separated by ";" or new lines. A bare target is named after itself.
pub fn parse_shortcuts(spec: &str, var: impl Fn(&str) -> Option<String>) -> Vec<Shortcut> {
    spec.split(|c| c == ';' || c == '\n')
        .filter_map(|item| {
            let item = item.trim();
            if item.is_empty() {
                return None;
            }
            let (name, target) = match item.split_once('=') {
                // "https://x.com/?a=b" has an "=" too: only split when the left side isn't a link.
                Some((n, t)) if !n.contains("://") && !n.contains('\\') && !n.contains(':') => {
                    (n.trim().to_string(), t.trim())
                }
                _ => (String::new(), item),
            };
            let target = expand(target, &var);
            let kind = if target.starts_with("https://") || target.starts_with("http://") {
                "web"
            } else if target.starts_with("ms-settings:") {
                "settings"
            } else if target.starts_with(r"shell:AppsFolder\") {
                "app"
            } else {
                "path"
            };
            let name = if name.is_empty() {
                let t = target.trim_end_matches(['\\', '/']);
                t.rsplit(['\\', '/']).next().unwrap_or(t).to_string()
            } else {
                name
            };
            Some(Shortcut {
                name,
                target,
                kind: kind.into(),
            })
        })
        .take(12)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_symbol_lists() {
        assert_eq!(
            symbols("aapl, MSFT ^gspc;btc-usd  eurusd=x"),
            vec!["AAPL", "MSFT", "^GSPC", "BTC-USD", "EURUSD=X"]
        );
        assert_eq!(symbols("AAPL aapl"), vec!["AAPL"]);
        assert!(
            symbols("../etc/passwd ?x=1 <script>")
                .iter()
                .all(|s| !s.contains('/') && !s.contains('?') && !s.contains('<'))
        );
        assert_eq!(symbols(&"A ".repeat(30)).len(), 1);
    }

    #[test]
    fn reads_a_quote() {
        let j = r#"{"chart":{"result":[{"meta":{"currency":"USD","symbol":"AAPL","regularMarketPrice":110.0,"chartPreviousClose":100.0,"shortName":"Apple Inc."},
            "indicators":{"quote":[{"close":[100.5,null,104.0,110.0]}]}}]}}"#;
        let q = parse_chart("AAPL", j).unwrap();
        assert_eq!(q.name, "Apple Inc.");
        assert!((q.change - 10.0).abs() < 1e-9);
        assert_eq!(q.points, vec![100.5, 104.0, 110.0]);
        assert_eq!(
            parse_chart(
                "X",
                r#"{"chart":{"result":null,"error":{"code":"Not Found"}}}"#
            ),
            None
        );
        assert_eq!(parse_chart("X", "not json"), None);
    }

    #[test]
    fn reads_rates() {
        let r = parse_rates(
            r#"{"amount":1.0,"base":"USD","date":"2026-11-16","rates":{"EUR":0.9,"GBP":0.75}}"#,
        )
        .unwrap();
        assert_eq!(r.base, "USD");
        assert_eq!(r.rates["EUR"], 0.9);
        assert_eq!(currency_code(" usd "), Some("USD".into()));
        assert_eq!(currency_code("US"), None);
        assert_eq!(currency_code("U$D"), None);
    }

    #[test]
    fn reads_rss_and_atom() {
        let rss = r#"<rss><channel><title>Feed</title><item><title><![CDATA[Rates cut &amp; markets rise]]></title><link>https://ex.com/a</link></item>
            <item><title>Q&amp;A: what&#39;s next &#x2014; <b>bold</b></title><link>https://ex.com/b</link></item>
            <item><title>Bad link</title><link>javascript:alert(1)</link></item></channel></rss>"#;
        let h = parse_feed(rss, 10);
        assert_eq!(h.len(), 2, "only web links are kept");
        assert_eq!(h[0].title, "Rates cut & markets rise");
        assert_eq!(h[1].title, "Q&A: what's next \u{2014} bold");
        let atom = r#"<feed><title>F</title><entry><title type="html">One</title><link rel="self" href="https://ex.com/self"/><link rel="alternate" href="https://ex.com/1"/></entry>
            <entry><title>Two</title><link href="https://ex.com/2"/></entry></feed>"#;
        let h = parse_feed(atom, 10);
        assert_eq!(
            h,
            vec![
                Headline {
                    title: "One".into(),
                    link: "https://ex.com/1".into()
                },
                Headline {
                    title: "Two".into(),
                    link: "https://ex.com/2".into()
                }
            ]
        );
        assert_eq!(parse_feed(rss, 1).len(), 1);
        assert!(parse_feed("<html>not a feed</html>", 5).is_empty());
    }

    /// Live check of the public sources: `cargo test live_sources -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_sources() {
        for q in quotes("AAPL ^GSPC BTC-USD NOTASYMBOL123") {
            println!(
                "{} {} {:.2} {:+.2}% {} pts={}",
                q.symbol,
                q.name,
                q.price,
                q.change,
                q.currency,
                q.points.len()
            );
        }
        let r = rates("usd").unwrap();
        println!(
            "USD on {}: EUR {} GBP {} ({} currencies)",
            r.date,
            r.rates["EUR"],
            r.rates["GBP"],
            r.rates.len()
        );
        for u in [
            "https://feeds.bbci.co.uk/news/rss.xml",
            "https://hnrss.org/frontpage",
            "https://www.theverge.com/rss/index.xml",
        ] {
            match headlines(u) {
                Ok(h) => println!("{u}: {} -> {:?}", h.len(), h.first()),
                Err(e) => println!("{u}: ERR {e}"),
            }
        }
        println!("photo 0: {:?}", photo("", 0));
    }

    #[test]
    fn spots_photos() {
        assert!(is_photo(r"C:\a\IMG_1.JPG"));
        assert!(is_photo("x.webp"));
        assert!(!is_photo("x.mp4"));
        assert!(!is_photo("jpg"));
    }

    #[test]
    fn reads_shortcuts() {
        let var = |k: &str| (k == "USERPROFILE").then(|| r"C:\Users\alex".to_string());
        let s = parse_shortcuts(
            "Docs = %USERPROFILE%\\Documents; GitHub=https://github.com/?tab=repos\nhttps://news.ycombinator.com/item?id=1; ms-settings:display; %NOPE%\\x",
            var,
        );
        assert_eq!(
            s[0],
            Shortcut {
                name: "Docs".into(),
                target: r"C:\Users\alex\Documents".into(),
                kind: "path".into()
            }
        );
        assert_eq!(s[1].target, "https://github.com/?tab=repos");
        assert_eq!(s[1].kind, "web");
        assert_eq!(
            s[2].target, "https://news.ycombinator.com/item?id=1",
            "an = inside a link isn't a name"
        );
        assert_eq!(s[3].kind, "settings");
        assert_eq!(s[4].target, r"%NOPE%\x");
        assert_eq!(expand("100% sure", |_| None), "100% sure");
        assert_eq!(parse_shortcuts(&"a=b;".repeat(40), |_| None).len(), 12);
    }
}
