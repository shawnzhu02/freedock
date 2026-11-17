//! Docker containers through the `docker` CLI: name, image, state and uptime, with start, stop and
//! restart. Only IDs from a fresh listing are acted on, and every argument is a separate argv
//! entry, so nothing from the webview reaches a shell.

use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    /// running, exited, paused, restarting, created or dead.
    pub state: String,
    /// Docker's own wording, e.g. "Up 3 hours" or "Exited (0) 2 days ago".
    pub status: String,
    pub ports: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct List {
    pub containers: Vec<Container>,
    /// Set when the CLI is missing or Docker Desktop isn't running.
    pub error: Option<String>,
}

/// One container per line of `docker ps -a --format "{{json .}}"`; bad lines are skipped.
pub fn parse(out: &str) -> Vec<Container> {
    let mut v: Vec<Container> = out
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
        .filter_map(|j| {
            let s = |k: &str| j[k].as_str().unwrap_or_default().to_string();
            let id = s("ID");
            if !valid_id(&id) {
                return None;
            }
            Some(Container {
                id,
                name: s("Names"),
                image: s("Image"),
                state: s("State").to_lowercase(),
                status: s("Status"),
                ports: s("Ports"),
            })
        })
        .collect();
    // Running first, then by name.
    v.sort_by(|a, b| {
        (a.state != "running")
            .cmp(&(b.state != "running"))
            .then(a.name.cmp(&b.name))
    });
    v
}

/// A container ID as docker prints it: 12 to 64 lowercase hex characters.
pub fn valid_id(id: &str) -> bool {
    (12..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn docker(args: &[&str]) -> Result<String, String> {
    let mut cmd = std::process::Command::new("docker");
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd
        .output()
        .map_err(|_| "Docker isn't installed".to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(
            if err.contains("pipe") || err.contains("daemon") || err.contains("cannot connect") {
                "Docker Desktop isn't running".into()
            } else {
                err.lines()
                    .next()
                    .unwrap_or("docker failed")
                    .chars()
                    .take(160)
                    .collect()
            },
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn list() -> List {
    match docker(&["ps", "-a", "--no-trunc", "--format", "{{json .}}"]) {
        Ok(out) => List {
            containers: parse(&out),
            error: None,
        },
        Err(e) => List {
            containers: vec![],
            error: Some(e),
        },
    }
}

/// start, stop or restart a container that is in the current listing.
pub fn act(id: &str, action: &str) -> Result<(), String> {
    if !matches!(action, "start" | "stop" | "restart") {
        return Err("unknown action".into());
    }
    if !valid_id(id) || !list().containers.iter().any(|c| c.id == id) {
        return Err("that container is gone".into());
    }
    docker(&[action, id]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_docker_ps_json_lines() {
        let out = concat!(
            r#"{"ID":"aaaaaaaaaaaa","Names":"web","Image":"nginx","State":"exited","Status":"Exited (0) 2 days ago","Ports":""}"#,
            "\n",
            r#"{"ID":"bbbbbbbbbbbb","Names":"api","Image":"node:20","State":"running","Status":"Up 3 hours","Ports":"0.0.0.0:3000->3000/tcp"}"#,
            "\nnot json\n",
            r#"{"ID":"$(rm -rf)","Names":"x","Image":"x","State":"running","Status":"","Ports":""}"#,
        );
        let v = parse(out);
        assert_eq!(v.len(), 2, "bad line and bad ID skipped");
        assert_eq!(v[0].name, "api", "running first");
        assert_eq!(v[1].state, "exited");
    }

    #[test]
    fn ids_are_hex_only() {
        assert!(valid_id("0123456789ab"));
        assert!(valid_id(&"f".repeat(64)));
        assert!(!valid_id("0123456789a"));
        assert!(!valid_id("0123456789aB"));
        assert!(!valid_id("--help-xxxxx"));
    }
}
