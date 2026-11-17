//! Site status: is each of the user's sites answering, how fast, and with what HTTP status. A plain GET of
//! the page (redirects followed), at most 8 sites, checked in parallel, cached for a minute.

use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Site {
    pub url: String,
    /// Shown name: the host without "www.".
    pub name: String,
    pub up: bool,
    /// HTTP status, 0 when nothing answered.
    pub code: u16,
    pub ms: u64,
    pub error: Option<String>,
}

/// "example.com, https://x.dev/health" -> full URLs, http(s) only, duplicates and junk dropped, max 8.
pub fn parse_list(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in s
        .split([',', '\n', ' '])
        .map(str::trim)
        .filter(|x| !x.is_empty())
    {
        let url = if raw.starts_with("https://") || raw.starts_with("http://") {
            raw.to_string()
        } else if raw.contains("://") {
            continue;
        } else {
            format!("https://{raw}")
        };
        let host = host_of(&url);
        if host.is_empty()
            || !host.contains('.')
            || host
                .chars()
                .any(|c| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':'))
        {
            continue;
        }
        if !out.contains(&url) {
            out.push(url);
        }
        if out.len() == 8 {
            break;
        }
    }
    out
}

pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map(|x| x.1).unwrap_or(url);
    rest.split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_lowercase()
}

fn name_of(url: &str) -> String {
    let h = host_of(url);
    h.strip_prefix("www.").unwrap_or(&h).to_string()
}

fn check_one(url: &str) -> Site {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .redirects(5)
        .user_agent("freedock-status/1")
        .build();
    let t = Instant::now();
    let r = agent.get(url).call();
    let ms = t.elapsed().as_millis() as u64;
    let (code, error) = match r {
        Ok(resp) => (resp.status(), None),
        Err(ureq::Error::Status(c, _)) => (c, None),
        Err(e) => (0, Some(e.kind().to_string())),
    };
    Site {
        url: url.into(),
        name: name_of(url),
        up: (200..400).contains(&code),
        code,
        ms,
        error,
    }
}

static CACHE: Mutex<Option<(Instant, Vec<String>, Vec<Site>)>> = Mutex::new(None);

pub fn check(list: &str, fresh: bool) -> Vec<Site> {
    let urls = parse_list(list);
    if !fresh {
        if let Some((at, u, s)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if *u == urls && at.elapsed() < Duration::from_secs(60) {
                return s.clone();
            }
        }
    }
    let handles: Vec<_> = urls
        .iter()
        .cloned()
        .map(|u| std::thread::spawn(move || check_one(&u)))
        .collect();
    let sites: Vec<Site> = handles
        .into_iter()
        .zip(&urls)
        .map(|(h, u)| {
            h.join().unwrap_or_else(|_| Site {
                url: u.clone(),
                name: name_of(u),
                up: false,
                code: 0,
                ms: 0,
                error: Some("check failed".into()),
            })
        })
        .collect();
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), urls, sites.clone()));
    sites
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_site_lists() {
        assert_eq!(
            parse_list(
                "example.com, https://my-app.dev/health\nhttp://a.dev  ftp://x.com, localhost, bad host.com, example.com"
            ),
            vec![
                "https://example.com",
                "https://my-app.dev/health",
                "http://a.dev",
                "https://host.com"
            ]
        );
        assert_eq!(parse_list(""), Vec::<String>::new());
        assert_eq!(
            parse_list(
                &(0..20)
                    .map(|i| format!("s{i}.com"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
            .len(),
            8
        );
    }

    #[test]
    fn names_drop_www() {
        assert_eq!(name_of("https://www.Example.com/x?y"), "example.com");
        assert_eq!(host_of("https://a.dev:8443/p"), "a.dev:8443");
    }
}
