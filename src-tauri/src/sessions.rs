//! Claude Code sessions on this PC, from their local transcripts (~/.claude/projects/*/*.jsonl):
//! which ones are working, which finished and are waiting on the user, and which seem stuck on a tool or
//! an approval prompt. Read-only; only the end of each recently written transcript is read, and only
//! the project folder name and state leave this module (no message text).

use serde::Serialize;
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::time::{Duration, SystemTime};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Session {
    /// The project folder's name, e.g. "freedock".
    pub project: String,
    pub cwd: String,
    /// "working", "waiting" (finished its turn, the user's move) or "stuck" (a tool call with no result
    /// for a while: usually an approval prompt).
    pub state: String,
    /// Seconds since the transcript was last written.
    pub idle: u64,
}

/// How long a pending tool call can reasonably run before a quiet transcript means it's waiting on
/// an approval instead. A shell command can't outlive its own timeout (2 minutes unless it says
/// otherwise); subagents and workflows run long; most other tools take seconds.
pub fn tool_limit(tool: &Value) -> u64 {
    let input = &tool["input"];
    match tool["name"].as_str().unwrap_or_default() {
        "Bash" | "PowerShell" => {
            if input["run_in_background"].as_bool() == Some(true) {
                return 60;
            }
            input["timeout"].as_u64().unwrap_or(120_000) / 1000 + 30
        }
        "Agent" | "Task" | "Workflow" | "Monitor" => 3600,
        "WebFetch" | "WebSearch" => 180,
        n if n.starts_with("mcp__") => 180,
        _ => 60,
    }
}

/// Unix seconds for a transcript timestamp like "2026-11-16T10:23:04.220Z" (UTC), or None.
pub fn unix_secs(ts: &str) -> Option<u64> {
    let b = ts.as_bytes();
    if b.len() < 19
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let num = |r: std::ops::Range<usize>| ts.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hh, mm, ss) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hh * 3600 + mm * 60 + ss).ok()
}

/// The state from the last user/assistant entries of a transcript (oldest first) and how long ago
/// it was written. None when there's nothing to go on.
pub fn state_of(last: &[Value], idle: u64) -> Option<&'static str> {
    let e = last.iter().rev().find(|j| {
        matches!(j["type"].as_str(), Some("user" | "assistant"))
            && j["isSidechain"].as_bool() != Some(true)
    })?;
    let m = &e["message"];
    let kinds: Vec<&str> = m["content"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x["type"].as_str()).collect())
        .unwrap_or_default();
    match e["type"].as_str()? {
        "assistant" if m["stop_reason"].as_str() == Some("end_turn") => Some("waiting"),
        "assistant" if kinds.contains(&"tool_use") => {
            let limit = m["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|x| x["type"] == "tool_use")
                .map(tool_limit)
                .max()
                .unwrap_or(60);
            Some(if idle > limit { "stuck" } else { "working" })
        }
        // The user's message, or a tool result going back to the model: the model is thinking.
        "user" | "assistant" => Some(if idle > 300 { "stuck" } else { "working" }),
        _ => None,
    }
}

fn tail_lines(path: &std::path::Path, bytes: u64) -> Vec<Value> {
    let Ok(mut f) = std::fs::File::open(path) else {
        return vec![];
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(bytes);
    if f.seek(SeekFrom::Start(start)).is_err() {
        return vec![];
    }
    let mut buf = Vec::new();
    if f.take(bytes).read_to_end(&mut buf).is_err() {
        return vec![];
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<&str> = text.lines().collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0); // probably cut mid-line
    }
    lines
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

/// The repo a session works in: the folder right under "Programming" when the path has one
/// (so `Programming\photo-app\apps\mobile` is "photo-app"), else the last folder.
pub fn project_name(cwd: &str) -> String {
    let parts: Vec<&str> = cwd.split(['\\', '/']).filter(|s| !s.is_empty()).collect();
    if let Some(i) = parts
        .iter()
        .position(|p| p.eq_ignore_ascii_case("Programming"))
    {
        if let Some(p) = parts.get(i + 1) {
            return p.to_string();
        }
    }
    parts
        .last()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Claude".into())
}

/// Sessions written in the last `within`, newest first, at most 12.
pub fn list(within: Duration) -> Vec<Session> {
    let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) else {
        return vec![];
    };
    let root = std::path::Path::new(&home).join(".claude").join("projects");
    let now = SystemTime::now();
    let mut files: Vec<(u64, std::path::PathBuf)> = Vec::new();
    for dir in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        for f in std::fs::read_dir(dir.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            let p = f.path();
            if p.extension().and_then(|x| x.to_str()) != Some("jsonl") {
                continue;
            }
            let Ok(m) = f.metadata() else { continue };
            let Ok(age) = m
                .modified()
                .map(|t| now.duration_since(t).unwrap_or_default())
            else {
                continue;
            };
            if age <= within {
                files.push((age.as_secs(), p));
            }
        }
    }
    files.sort();
    files.truncate(16);
    files
        .into_iter()
        .filter_map(|(file_idle, p)| {
            let rows = tail_lines(&p, 256 * 1024);
            // Idle time comes from the last real turn: Claude Code also appends bookkeeping lines to
            // transcripts that nobody is using, which keeps the file's modified time fresh.
            let now_s = now
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let idle = rows
                .iter()
                .rev()
                .filter(|j| matches!(j["type"].as_str(), Some("user" | "assistant")))
                .find_map(|j| unix_secs(j["timestamp"].as_str()?))
                .map(|t| now_s.saturating_sub(t))
                .unwrap_or(file_idle);
            if idle > within.as_secs() {
                return None;
            }
            let state = state_of(&rows, idle)?;
            let cwd = rows
                .iter()
                .rev()
                .find_map(|j| j["cwd"].as_str())
                .unwrap_or_default()
                .to_string();
            let project = project_name(&cwd);
            Some(Session {
                project,
                cwd,
                state: state.into(),
                idle,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn timestamps() {
        assert_eq!(unix_secs("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_secs("2026-11-16T10:23:04.220Z"), Some(1_794_824_584));
        assert_eq!(unix_secs("2024-02-29T12:00:00Z"), Some(1_709_208_000));
        assert_eq!(unix_secs("nope"), None);
        assert_eq!(unix_secs("2026-13-01T00:00:00Z"), None);
    }

    #[test]
    fn project_names() {
        assert_eq!(
            project_name(r"C:\Users\alex\Programming\photo-app\apps\mobile"),
            "photo-app"
        );
        assert_eq!(
            project_name(r"C:\Users\alex\Programming\freedock"),
            "freedock"
        );
        assert_eq!(
            project_name(r"C:\Users\alex\.claude\projects\C--Users-alex\memory"),
            "memory"
        );
        assert_eq!(project_name(""), "Claude");
    }

    #[test]
    fn states_from_the_last_turn() {
        let done = json!({"type":"assistant","message":{"stop_reason":"end_turn","content":[{"type":"text"}]}});
        let tool = json!({"type":"assistant","message":{"stop_reason":"tool_use","content":[{"type":"tool_use","name":"Edit"}]}});
        let build = json!({"type":"assistant","message":{"stop_reason":"tool_use","content":[{"type":"tool_use","name":"Bash","input":{"timeout":600000}}]}});
        let result = json!({"type":"user","message":{"content":[{"type":"tool_result"}]}});
        let side = json!({"type":"assistant","isSidechain":true,"message":{"stop_reason":"end_turn","content":[]}});
        let other = json!({"type":"system"});
        assert_eq!(
            state_of(&[tool.clone(), done.clone(), other.clone()], 600),
            Some("waiting")
        );
        assert_eq!(state_of(&[done.clone(), tool.clone()], 5), Some("working"));
        assert_eq!(
            state_of(&[done.clone(), tool.clone()], 200),
            Some("stuck"),
            "an edit with no result for minutes"
        );
        assert_eq!(
            state_of(&[done.clone(), build.clone()], 300),
            Some("working"),
            "a 10-minute build is still running"
        );
        assert_eq!(
            state_of(&[done.clone(), build], 700),
            Some("stuck"),
            "past its own timeout"
        );
        assert_eq!(
            state_of(&[tool.clone(), result.clone()], 10),
            Some("working")
        );
        assert_eq!(
            state_of(&[result.clone(), side], 10),
            Some("working"),
            "sidechain entries are skipped"
        );
        assert_eq!(state_of(&[other], 10), None);
    }
}
