//! An optional "agenda" MCP server: any MCP server over HTTP that offers some of the tools below
//! (see docs/MCP_AGENDA.md for the shapes). When one is connected, its events and to-dos join the
//! calendar ones, Quick to-do adds there, and the Agent flyout can ask it what's next or search it.
//! The server's link usually carries an access token, so it lives in secrets.rs, never in settings.

use serde_json::{Value, json};

/// The tools the dock may call, and nothing else.
pub const TOOLS: [&str; 4] = ["now", "today_view", "search_memory", "add_todo"];

/// The text of an MCP tools/call reply, which may come as plain JSON or as server-sent events.
pub fn reply_text(content_type: &str, body: &str) -> Result<String, String> {
    let raw = if content_type.contains("event-stream") {
        body.lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .last()
            .unwrap_or("")
            .to_string()
    } else {
        body.to_string()
    };
    let v: Value = serde_json::from_str(&raw)
        .map_err(|_| "the agenda server sent something unreadable".to_string())?;
    if let Some(e) = v.get("error") {
        return Err(format!(
            "agenda server: {}",
            e.get("message").and_then(Value::as_str).unwrap_or("error")
        ));
    }
    let r = &v["result"];
    let text: Vec<&str> = r["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["text"].as_str())
        .collect();
    if r["isError"].as_bool() == Some(true) {
        return Err(format!("agenda server: {}", text.join(" ")));
    }
    Ok(text.join("\n"))
}

/// The https MCP servers configured in Claude Code's ~/.claude.json, by name (never their URLs).
pub fn names_in_claude_config(cfg: &Value) -> Vec<String> {
    let mut out: Vec<String> = cfg["mcpServers"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, v)| v["url"].as_str().is_some_and(|u| u.starts_with("https://")))
        .map(|(k, _)| k.clone())
        .collect();
    out.sort();
    out
}

/// One server's URL from Claude Code's config, https only.
pub fn url_from_claude_config(cfg: &Value, name: &str) -> Option<String> {
    let url = cfg["mcpServers"][name]["url"].as_str()?;
    url.starts_with("https://").then(|| url.to_string())
}

fn claude_config() -> Result<Value, String> {
    let home = std::env::var_os("USERPROFILE").ok_or("no home folder")?;
    let text = std::fs::read_to_string(std::path::Path::new(&home).join(".claude.json"))
        .map_err(|_| "couldn't read ~/.claude.json".to_string())?;
    serde_json::from_str(&text).map_err(|_| "~/.claude.json isn't valid JSON".to_string())
}

/// Server names Claude Code knows (to offer in Settings).
pub fn claude_servers() -> Vec<String> {
    claude_config()
        .map(|c| names_in_claude_config(&c))
        .unwrap_or_default()
}

/// Copy one of Claude Code's servers into the encrypted store.
pub fn import(name: &str) -> Result<(), String> {
    let url = url_from_claude_config(&claude_config()?, name)
        .ok_or(format!("no https `{name}` server in ~/.claude.json"))?;
    crate::secrets::set("agenda_mcp", Some(url))?;
    crate::secrets::set("agenda_mcp_name", Some(name.to_string()))
}

/// Connect a server by its link (https only).
pub fn set_url(url: &str, name: &str) -> Result<(), String> {
    let url = url.trim();
    if !url.starts_with("https://") {
        return Err("Use the server's https:// link".into());
    }
    crate::secrets::set("agenda_mcp", Some(url.to_string()))?;
    crate::secrets::set(
        "agenda_mcp_name",
        Some(if name.trim().is_empty() {
            "MCP server".into()
        } else {
            name.trim().to_string()
        }),
    )
}

pub fn disconnect() -> Result<(), String> {
    crate::secrets::set("agenda_mcp", None)?;
    crate::secrets::set("agenda_mcp_name", None)
}

/// The connected server's display name, if any.
pub fn connected() -> Option<String> {
    crate::secrets::get("agenda_mcp")?;
    Some(crate::secrets::get("agenda_mcp_name").unwrap_or_else(|| "MCP server".into()))
}

/// Call one of the allowed tools. Blocking (run it off the UI thread).
pub fn call(tool: &str, args: Value) -> Result<String, String> {
    if !TOOLS.contains(&tool) {
        return Err(format!("{tool} isn't an agenda tool the dock uses"));
    }
    let url = crate::secrets::get("agenda_mcp")
        .ok_or("No agenda server is connected (Settings > Agenda)")?;
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": tool, "arguments": args}});
    let resp = ureq::post(&url)
        .timeout(std::time::Duration::from_secs(25))
        .set("Content-Type", "application/json")
        .set("Accept", "application/json, text/event-stream")
        .send_string(&body.to_string());
    // Errors from the HTTP client include the URL (and so its token): report only what happened.
    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => {
            return Err(format!("the agenda server answered HTTP {code}"));
        }
        Err(_) => return Err("couldn't reach the agenda server".into()),
    };
    let ct = resp.content_type().to_string();
    let text = resp
        .into_string()
        .map_err(|_| "the agenda server's reply was cut off".to_string())?;
    reply_text(&ct, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_json_and_sse_replies() {
        let j = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"hello"}]}}"#;
        assert_eq!(reply_text("application/json", j), Ok("hello".into()));
        let sse = format!("event: message\ndata: {j}\n\n");
        assert_eq!(reply_text("text/event-stream", &sse), Ok("hello".into()));
        let bad = r#"{"jsonrpc":"2.0","id":1,"result":{"isError":true,"content":[{"type":"text","text":"nope"}]}}"#;
        assert_eq!(
            reply_text("application/json", bad),
            Err("agenda server: nope".into())
        );
    }

    #[test]
    fn only_https_servers_from_claude_config() {
        let cfg = json!({"mcpServers": {
            "cal": {"type": "http", "url": "https://x/j/abc/"},
            "local": {"url": "http://x"},
            "stdio": {"command": "npx"}
        }});
        assert_eq!(names_in_claude_config(&cfg), ["cal"]);
        assert_eq!(
            url_from_claude_config(&cfg, "cal").as_deref(),
            Some("https://x/j/abc/")
        );
        assert_eq!(url_from_claude_config(&cfg, "local"), None);
    }
}
