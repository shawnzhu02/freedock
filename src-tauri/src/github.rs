//! GitHub through the user's own logged-in `gh` CLI: unread notifications, the user's open pull requests with
//! their checks and review state, and PRs waiting on the user's review. Read-only, fixed queries only;
//! no token is read or stored here (gh keeps it in the Windows credential store).

use serde::Serialize;
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Note {
    pub title: String,
    pub repo: String,
    pub reason: String,
    pub kind: String,
    pub url: String,
    pub updated: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Pr {
    pub title: String,
    pub repo: String,
    pub url: String,
    pub draft: bool,
    /// APPROVED, CHANGES_REQUESTED, REVIEW_REQUIRED or empty.
    pub review: String,
    /// SUCCESS, FAILURE, ERROR, PENDING, EXPECTED or empty (no checks).
    pub checks: String,
    pub updated: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Summary {
    pub login: String,
    pub unread: usize,
    pub notes: Vec<Note>,
    pub prs: Vec<Pr>,
    pub reviews: Vec<Pr>,
    /// Set when gh is missing or not logged in.
    pub error: Option<String>,
}

const QUERY: &str = r#"query {
  viewer {
    login
    pullRequests(states: OPEN, first: 15, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes { title url isDraft reviewDecision updatedAt repository { nameWithOwner }
        commits(last: 1) { nodes { commit { statusCheckRollup { state } } } } }
    }
  }
  reviews: search(query: "is:pr is:open review-requested:@me archived:false", type: ISSUE, first: 15) {
    nodes { ... on PullRequest { title url isDraft updatedAt repository { nameWithOwner } } }
  }
}"#;

/// The web page for a notification's API subject URL.
/// `https://api.github.com/repos/o/r/pulls/5` becomes `https://github.com/o/r/pull/5`.
pub fn html_url(api: &str, repo: &str) -> String {
    let fallback = format!("https://github.com/{repo}");
    let Some(rest) = api.strip_prefix("https://api.github.com/repos/") else {
        return fallback;
    };
    let parts: Vec<&str> = rest.split('/').collect();
    match parts.as_slice() {
        [o, r, "pulls", n] => format!("https://github.com/{o}/{r}/pull/{n}"),
        [o, r, "issues", n] => format!("https://github.com/{o}/{r}/issues/{n}"),
        [o, r, "commits", sha] => format!("https://github.com/{o}/{r}/commit/{sha}"),
        [o, r, "releases", _] => format!("https://github.com/{o}/{r}/releases"),
        _ => fallback,
    }
}

pub fn parse_notes(v: &Value) -> Vec<Note> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|n| {
            let repo = n["repository"]["full_name"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            Note {
                title: n["subject"]["title"].as_str().unwrap_or_default().into(),
                url: html_url(n["subject"]["url"].as_str().unwrap_or_default(), &repo),
                reason: n["reason"].as_str().unwrap_or_default().into(),
                kind: n["subject"]["type"].as_str().unwrap_or_default().into(),
                updated: n["updated_at"].as_str().unwrap_or_default().into(),
                repo,
            }
        })
        .collect()
}

fn pr(n: &Value) -> Option<Pr> {
    Some(Pr {
        title: n["title"].as_str()?.into(),
        url: n["url"].as_str()?.into(),
        repo: n["repository"]["nameWithOwner"]
            .as_str()
            .unwrap_or_default()
            .into(),
        draft: n["isDraft"].as_bool().unwrap_or(false),
        review: n["reviewDecision"].as_str().unwrap_or_default().into(),
        checks: n["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["state"]
            .as_str()
            .unwrap_or_default()
            .into(),
        updated: n["updatedAt"].as_str().unwrap_or_default().into(),
    })
}

/// (login, the user's open PRs, PRs asking for the user's review) from the GraphQL answer.
pub fn parse_graphql(v: &Value) -> (String, Vec<Pr>, Vec<Pr>) {
    let d = &v["data"];
    let list = |x: &Value| {
        x.as_array()
            .into_iter()
            .flatten()
            .filter_map(pr)
            .collect::<Vec<_>>()
    };
    (
        d["viewer"]["login"].as_str().unwrap_or_default().into(),
        list(&d["viewer"]["pullRequests"]["nodes"]),
        list(&d["reviews"]["nodes"]),
    )
}

fn gh(args: &[&str]) -> Result<Value, String> {
    let mut cmd = std::process::Command::new("gh");
    cmd.args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd
        .output()
        .map_err(|_| "GitHub CLI (gh) isn't installed".to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(
            if err.contains("auth login") || err.contains("not logged") {
                "Run `gh auth login` to connect GitHub".into()
            } else {
                err.lines()
                    .next()
                    .unwrap_or("gh failed")
                    .chars()
                    .take(160)
                    .collect()
            },
        );
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

static CACHE: Mutex<Option<(Instant, Summary)>> = Mutex::new(None);

/// Cached for 3 minutes unless `fresh`.
pub fn summary(fresh: bool) -> Summary {
    if !fresh {
        if let Some((at, s)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if at.elapsed() < Duration::from_secs(180) {
                return s.clone();
            }
        }
    }
    let mut s = Summary::default();
    match gh(&["api", "graphql", "-f", &format!("query={QUERY}")]) {
        Ok(v) => (s.login, s.prs, s.reviews) = parse_graphql(&v),
        Err(e) => s.error = Some(e),
    }
    if s.error.is_none() {
        match gh(&["api", "notifications?per_page=50"]) {
            Ok(v) => {
                s.notes = parse_notes(&v);
                s.unread = s.notes.len();
            }
            Err(e) => s.error = Some(e),
        }
    }
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), s.clone()));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn notification_links_go_to_the_web_page() {
        assert_eq!(
            html_url("https://api.github.com/repos/a/b/pulls/12", "a/b"),
            "https://github.com/a/b/pull/12"
        );
        assert_eq!(
            html_url("https://api.github.com/repos/a/b/issues/3", "a/b"),
            "https://github.com/a/b/issues/3"
        );
        assert_eq!(
            html_url("https://api.github.com/repos/a/b/releases/99", "a/b"),
            "https://github.com/a/b/releases"
        );
        assert_eq!(html_url("", "a/b"), "https://github.com/a/b");
        assert_eq!(
            html_url("https://evil.example/repos/a/b/pulls/1", "a/b"),
            "https://github.com/a/b"
        );
    }

    #[test]
    fn parses_prs_and_reviews() {
        let v = json!({"data": {
            "viewer": {"login": "me", "pullRequests": {"nodes": [
                {"title": "Fix", "url": "https://github.com/a/b/pull/1", "isDraft": false, "reviewDecision": "APPROVED",
                 "updatedAt": "2026-11-16T01:00:00Z", "repository": {"nameWithOwner": "a/b"},
                 "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "FAILURE"}}}]}},
                {"title": "No checks", "url": "https://github.com/a/b/pull/2", "isDraft": true, "reviewDecision": null,
                 "updatedAt": "", "repository": {"nameWithOwner": "a/b"}, "commits": {"nodes": [{"commit": {"statusCheckRollup": null}}]}}
            ]}},
            "reviews": {"nodes": [{"title": "Look", "url": "https://github.com/c/d/pull/9", "isDraft": false, "updatedAt": "", "repository": {"nameWithOwner": "c/d"}}, {}]}
        }});
        let (login, prs, reviews) = parse_graphql(&v);
        assert_eq!(login, "me");
        assert_eq!(prs.len(), 2);
        assert_eq!(prs[0].checks, "FAILURE");
        assert_eq!(prs[0].review, "APPROVED");
        assert_eq!(prs[1].checks, "");
        assert!(prs[1].draft);
        assert_eq!(reviews.len(), 1, "non-PR search hits are skipped");
    }
}
