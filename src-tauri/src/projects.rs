//! Projects: the git repos in ~/Programming (and one level of subfolders), most recently worked on
//! first, with when Claude Code last worked in each. Opening one only ever launches a fixed set of
//! programs on a folder from a fresh scan, never a path or command taken from the page.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Project {
    pub name: String,
    pub path: String,
    pub branch: String,
    /// Unix seconds of the latest git activity (commit, checkout, staging).
    pub touched: i64,
    /// Unix seconds Claude Code last wrote a transcript for this folder, if ever.
    pub claude: Option<i64>,
    /// https://github.com/owner/repo when origin is on GitHub.
    pub web: Option<String>,
}

fn secs(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn mtime(p: &Path) -> Option<i64> {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .map(secs)
}

/// The branch name from a .git/HEAD file's text ("" when detached).
pub fn branch_of(head: &str) -> String {
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .unwrap_or("")
        .to_string()
}

/// origin's URL from a .git/config's text, as a GitHub web address.
pub fn github_of(config: &str) -> Option<String> {
    let mut in_origin = false;
    for line in config.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_origin = l == "[remote \"origin\"]";
            continue;
        }
        if in_origin {
            if let Some(url) = l
                .strip_prefix("url")
                .map(|r| r.trim_start())
                .and_then(|r| r.strip_prefix('='))
            {
                let url = url.trim().trim_end_matches('/').trim_end_matches(".git");
                let rest = url
                    .strip_prefix("https://github.com/")
                    .or_else(|| url.strip_prefix("git@github.com:"))
                    .or_else(|| url.strip_prefix("ssh://git@github.com/"))?;
                // Drop any credentials-free "owner/repo" check: exactly two plain segments.
                let parts: Vec<&str> = rest.split('/').collect();
                let ok = |s: &str| {
                    !s.is_empty()
                        && s.chars()
                            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
                };
                return (parts.len() == 2 && ok(parts[0]) && ok(parts[1]))
                    .then(|| format!("https://github.com/{}/{}", parts[0], parts[1]));
            }
        }
    }
    None
}

/// Claude Code's transcript folder name for a working directory.
pub fn claude_dir_name(path: &str) -> String {
    path.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// The .git directory of a repo folder (a plain dir, or a worktree's `gitdir:` file).
fn git_dir(dir: &Path) -> Option<PathBuf> {
    let g = dir.join(".git");
    if g.is_dir() {
        return Some(g);
    }
    let text = std::fs::read_to_string(&g).ok()?;
    let p = PathBuf::from(text.trim().strip_prefix("gitdir:")?.trim());
    p.is_dir().then_some(p)
}

fn read(dir: &Path, claude_root: &Path) -> Option<Project> {
    let g = git_dir(dir)?;
    let branch = branch_of(&std::fs::read_to_string(g.join("HEAD")).unwrap_or_default());
    // Worktrees keep config in the main repo: follow commondir.
    let common = std::fs::read_to_string(g.join("commondir"))
        .ok()
        .map(|c| g.join(c.trim()))
        .unwrap_or_else(|| g.clone());
    let web = github_of(&std::fs::read_to_string(common.join("config")).unwrap_or_default());
    let touched = ["logs/HEAD", "index", "HEAD"]
        .iter()
        .filter_map(|f| mtime(&g.join(f)))
        .max()
        .unwrap_or(0);
    let path = dir.to_string_lossy().to_string();
    let claude = std::fs::read_dir(claude_root.join(claude_dir_name(&path)))
        .ok()
        .and_then(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
                .filter_map(|e| e.metadata().ok()?.modified().ok().map(secs))
                .max()
        });
    Some(Project {
        name: dir.file_name()?.to_string_lossy().to_string(),
        path,
        branch,
        touched,
        claude,
        web,
    })
}

/// Folders in your user folder where people usually keep code, in the order they're searched.
pub const ROOTS: [&str; 9] = [
    "Programming",
    "Projects",
    "projects",
    "code",
    "dev",
    "src",
    "repos",
    r"source\repos",
    r"Documents\GitHub",
];

/// The code folders that exist on this PC (Windows paths are case-insensitive, so duplicates are dropped).
pub fn roots() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return vec![];
    };
    let mut out: Vec<PathBuf> = Vec::new();
    for r in ROOTS {
        let p = home.join(r);
        if p.is_dir()
            && !out.iter().any(|o| {
                o.to_string_lossy()
                    .eq_ignore_ascii_case(&p.to_string_lossy())
            })
        {
            out.push(p);
        }
    }
    out
}

/// Every repo directly in a code folder (see ROOTS) or one folder down, latest activity first.
pub fn list() -> Vec<Project> {
    let claude_root = std::env::var_os("USERPROFILE")
        .map(|h| PathBuf::from(h).join(".claude").join("projects"))
        .unwrap_or_default();
    let mut out = vec![];
    let skip = |n: &str| n.starts_with('.') || n.starts_with('_') || n == "node_modules";
    for root in roots() {
        for e in std::fs::read_dir(&root)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
        {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if !p.is_dir() || skip(&name) {
                continue;
            }
            if let Some(pr) = read(&p, &claude_root) {
                out.push(pr);
                continue;
            }
            for sub in std::fs::read_dir(&p)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
            {
                let sp = sub.path();
                let sn = sub.file_name().to_string_lossy().to_string();
                if sp.is_dir() && !skip(&sn) {
                    if let Some(mut pr) = read(&sp, &claude_root) {
                        pr.name = format!("{name}/{sn}");
                        out.push(pr);
                    }
                }
            }
        }
    }
    out.sort_by_key(|p| std::cmp::Reverse(p.touched.max(p.claude.unwrap_or(0))));
    out.dedup_by(|a, b| a.path.eq_ignore_ascii_case(&b.path));
    out
}

/// Open a listed project with one of a fixed set of programs.
pub fn open(path: &str, how: &str) -> Result<(), String> {
    let p = list()
        .into_iter()
        .find(|p| p.path == path)
        .ok_or("That folder isn't one of your projects")?;
    use std::process::Command;
    let spawn = |c: &mut Command| c.spawn().map(|_| ()).map_err(|e| e.to_string());
    match how {
        "code" => {
            let local = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_default();
            let exe = local
                .join("Programs")
                .join("Microsoft VS Code")
                .join("Code.exe");
            if !exe.is_file() {
                return Err("VS Code isn't installed".into());
            }
            spawn(Command::new(exe).arg(&p.path))
        }
        // wt.exe splits its command line into subcommands at ';', so the folder never goes on it: the
        // terminal starts in our working directory ("-d ."), and a path with ';' is refused outright.
        "terminal" | "claude" if p.path.contains(';') => {
            Err("That folder's name has a ';', which Windows Terminal can't open safely".into())
        }
        "terminal" => spawn(
            Command::new("wt.exe")
                .current_dir(&p.path)
                .args(["-d", "."]),
        ),
        "claude" => {
            let exe = crate::ask::claude_exe().ok_or("Claude Code isn't installed")?;
            if exe.to_string_lossy().contains(';') {
                return Err("Claude Code's path has a ';'".into());
            }
            spawn(
                Command::new("wt.exe")
                    .current_dir(&p.path)
                    .args(["-d", "."])
                    .arg(exe),
            )
        }
        "explorer" => spawn(Command::new("explorer.exe").arg(&p.path)),
        "github" => {
            let url = p.web.ok_or("No GitHub remote")?;
            tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
        }
        _ => Err("unknown action".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches() {
        assert_eq!(branch_of("ref: refs/heads/main\n"), "main");
        assert_eq!(branch_of("ref: refs/heads/feat/x"), "feat/x");
        assert_eq!(branch_of("4f1c2a9e0d"), "");
    }

    #[test]
    fn github_urls() {
        let cfg = "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = https://github.com/alex/glass-dock.git\n\tfetch = +refs/heads/*\n";
        assert_eq!(
            github_of(cfg).as_deref(),
            Some("https://github.com/alex/glass-dock")
        );
        assert_eq!(
            github_of("[remote \"origin\"]\n url = git@github.com:a/b.git").as_deref(),
            Some("https://github.com/a/b")
        );
        assert_eq!(
            github_of("[remote \"upstream\"]\n url = https://github.com/a/b"),
            None
        );
        assert_eq!(
            github_of("[remote \"origin\"]\n url = https://gitlab.com/a/b"),
            None
        );
        // Odd characters never make it into a URL we open.
        assert_eq!(
            github_of("[remote \"origin\"]\n url = https://github.com/a/b?x=1"),
            None
        );
        assert_eq!(
            github_of("[remote \"origin\"]\n url = https://github.com/a/b/c"),
            None
        );
    }

    #[test]
    fn claude_folder_names() {
        assert_eq!(
            claude_dir_name(r"C:\Users\alex\Programming\home\server"),
            "C--Users-alex-Programming-home-server"
        );
        assert_eq!(
            claude_dir_name(r"C:\Users\alex\Programming\chess_bot"),
            "C--Users-alex-Programming-chess-bot"
        );
    }
}
