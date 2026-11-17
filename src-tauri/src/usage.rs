//! Claude usage: how much of the 5-hour and weekly limits is used, as Claude Code reports it in its
//! stream-json `rate_limit_event`. Read for free whenever Quick Ask runs; otherwise a tiny probe
//! (Haiku, a one-word system prompt, no tools, MCP servers, hooks or settings: about 700 tokens)
//! asks for a fresh reading, at most every few minutes and only when a cell or flyout wants one.

use serde::Serialize;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Window {
    /// 0-1.
    pub utilization: f64,
    /// Unix seconds.
    pub resets_at: i64,
    /// "allowed", "allowed_warning", "rejected"...
    pub status: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct Usage {
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
    /// Unix seconds when this was read.
    pub at: i64,
}

static LAST: Mutex<Option<Usage>> = Mutex::new(None);
static PROBING: Mutex<()> = Mutex::new(());

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The usage in one stream-json line, if it's a rate-limit event with the unified windows.
pub fn parse_event(line: &str, at: i64) -> Option<Usage> {
    if !line.contains("rate_limit_event") {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v.get("type")?.as_str()? != "rate_limit_event" {
        return None;
    }
    let windows = v.get("rate_limit_info")?.get("unifiedWindows")?;
    let read = |k: &str| {
        let w = windows.get(k)?;
        Some(Window {
            utilization: w.get("utilization")?.as_f64()?,
            resets_at: w.get("resetsAt")?.as_i64()?,
            status: w
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("allowed")
                .to_string(),
        })
    };
    let u = Usage {
        five_hour: read("five_hour"),
        seven_day: read("seven_day"),
        at,
    };
    (u.five_hour.is_some() || u.seven_day.is_some()).then_some(u)
}

/// Remember a reading (from Quick Ask's stream or a probe). Returns true when it's new.
pub fn record(line: &str) -> bool {
    let Some(u) = parse_event(line, now_secs()) else {
        return false;
    };
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(u);
    crate::notify("usage-changed");
    true
}

pub fn last() -> Option<Usage> {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The last reading if it's younger than `max_age` seconds, else a fresh probe (one at a time;
/// a second caller waits for the first and gets its result).
pub fn get(max_age: i64) -> Result<Usage, String> {
    if let Some(u) = last().filter(|u| now_secs() - u.at <= max_age) {
        return Ok(u);
    }
    let _one = PROBING.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(u) = last().filter(|u| now_secs() - u.at <= max_age) {
        return Ok(u);
    }
    // A probe that failed (not installed, logged out, no limits to report) isn't retried until the
    // same age has passed: each probe starts Claude Code, so retrying every minute would be costly.
    if let Some((at, e)) = FAILED.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        if now_secs() - at < max_age.clamp(60, 3600) {
            return Err(e);
        }
    }
    let r = probe().and_then(|_| last().ok_or_else(|| "Claude didn't report usage".to_string()));
    *FAILED.lock().unwrap_or_else(|e| e.into_inner()) =
        r.as_ref().err().map(|e| (now_secs(), e.clone()));
    r
}

static FAILED: Mutex<Option<(i64, String)>> = Mutex::new(None);

fn probe() -> Result<(), String> {
    use std::io::{BufRead, BufReader};
    let exe = crate::ask::claude_exe().ok_or("Claude Code isn't installed")?;
    let mut cmd = std::process::Command::new(exe);
    cmd.args([
        "-p",
        ".",
        "--output-format",
        "stream-json",
        "--verbose",
        "--no-session-persistence",
        "--model",
        "haiku",
        "--system-prompt",
        "Reply with one word.",
        "--tools",
        "",
        "--strict-mcp-config",
        "--mcp-config",
        r#"{"mcpServers":{}}"#,
        "--setting-sources",
        "",
        "--disable-slash-commands",
    ])
    .current_dir(crate::ask::workdir())
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("couldn't start Claude Code: {e}"))?;
    let out = child.stdout.take().ok_or("no output")?;
    // A stuck probe is ended after a minute (only this child, which we started).
    let child = std::sync::Arc::new(Mutex::new(child));
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let (child, done) = (child.clone(), done.clone());
        std::thread::spawn(move || {
            for _ in 0..60 {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if done.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
            }
            let mut c = child.lock().unwrap_or_else(|e| e.into_inner());
            if matches!(c.try_wait(), Ok(None)) {
                let _ = c.kill();
            }
        });
    }
    let mut got = false;
    for line in BufReader::new(out).lines().map_while(Result::ok) {
        got |= record(&line);
    }
    let _ = child.lock().unwrap_or_else(|e| e.into_inner()).wait();
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    if got {
        Ok(())
    } else {
        Err("Claude didn't report usage (signed out?)".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_unified_windows() {
        let line = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","resetsAt":1794819000,"rateLimitType":"five_hour","unifiedWindows":{"five_hour":{"utilization":0.58,"resetsAt":1794819000,"status":"allowed"},"seven_day":{"utilization":0.44,"resetsAt":1795050000,"status":"allowed_warning"}}},"uuid":"x"}"#;
        let u = parse_event(line, 7).unwrap();
        assert_eq!(u.five_hour.as_ref().unwrap().utilization, 0.58);
        assert_eq!(u.seven_day.as_ref().unwrap().resets_at, 1795050000);
        assert_eq!(u.seven_day.unwrap().status, "allowed_warning");
        assert_eq!(u.at, 7);
    }

    #[test]
    fn ignores_other_lines() {
        assert!(parse_event(r#"{"type":"result","usage":{}}"#, 0).is_none());
        assert!(
            parse_event(
                r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#,
                0
            )
            .is_none()
        );
        assert!(parse_event("not json rate_limit_event", 0).is_none());
    }
}
