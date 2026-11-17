//! Downloads, Screenshots and Recent files: the newest items in those folders, with thumbnails. Opening only ever
//! acts on a file from a fresh listing of that folder (open it, or show it in Explorer).

use serde::Serialize;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub name: String,
    pub path: String,
    pub size: u64,
    /// Unix seconds.
    pub modified: i64,
    pub dir: bool,
}

/// The folder for a kind: "downloads", "screenshots" or "recent" (Windows' recent-items shortcuts).
pub fn folder(kind: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::{
            FOLDERID_Downloads, FOLDERID_Recent, FOLDERID_Screenshots, KF_FLAG_DEFAULT,
            SHGetKnownFolderPath,
        };
        let id = match kind {
            "downloads" => FOLDERID_Downloads,
            "screenshots" => FOLDERID_Screenshots,
            "recent" => FOLDERID_Recent,
            _ => return None,
        };
        unsafe {
            let p = SHGetKnownFolderPath(&id, KF_FLAG_DEFAULT, None).ok()?;
            let s = p.to_string().ok();
            windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
            return s.map(PathBuf::from).filter(|p| p.is_dir());
        }
    }
    #[allow(unreachable_code)]
    None
}

/// Hidden, system and half-downloaded files don't belong in the list.
pub fn shown(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !(name.starts_with('.')
        || lower == "desktop.ini"
        || lower == "thumbs.db"
        || lower.ends_with(".crdownload")
        || lower.ends_with(".part")
        || lower.ends_with(".partial")
        || lower.ends_with(".tmp"))
}

/// Newest first, at most `limit`.
pub fn list(kind: &str, limit: usize) -> Vec<Item> {
    let Some(dir) = folder(kind) else {
        return vec![];
    };
    let recent = kind == "recent";
    let mut out: Vec<Item> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if !shown(&name) {
                return None;
            }
            let m = e.metadata().ok()?;
            // Recent holds a shortcut per item (plus jump-list folders, skipped); show the item's name.
            let name = if recent {
                if m.is_dir() {
                    return None;
                }
                recent_name(&name)?
            } else {
                name
            };
            let modified = m
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs() as i64;
            // A path that isn't valid Unicode couldn't be opened again from its lossy text.
            let path = e.path().to_str()?.to_string();
            Some(Item {
                name,
                path,
                size: m.len(),
                modified,
                dir: m.is_dir(),
            })
        })
        .collect();
    out.sort_by_key(|i| std::cmp::Reverse(i.modified));
    out.truncate(limit);
    out
}

/// Programs and scripts: a Recent shortcut that points at one of these is never launched from the
/// dock (Recent lists documents; a shortcut there to a program didn't come from opening a file).
pub fn runs_code(target: &str) -> bool {
    let ext = target
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    target.contains('.')
        && matches!(
            ext.as_str(),
            "exe"
                | "com"
                | "scr"
                | "bat"
                | "cmd"
                | "ps1"
                | "psm1"
                | "vbs"
                | "vbe"
                | "js"
                | "jse"
                | "wsf"
                | "wsh"
                | "msi"
                | "msp"
                | "lnk"
                | "pif"
                | "cpl"
                | "hta"
                | "jar"
                | "reg"
        )
}

/// "report.pdf.lnk" -> "report.pdf"; anything that isn't a shortcut is skipped.
pub fn recent_name(file: &str) -> Option<String> {
    let stem = file
        .strip_suffix(".lnk")
        .or_else(|| file.strip_suffix(".LNK"))?;
    (!stem.is_empty()).then(|| stem.to_string())
}

fn check(kind: &str, path: &str) -> Result<Item, String> {
    list(kind, 200)
        .into_iter()
        .find(|i| i.path == path)
        .ok_or_else(|| "That file isn't in the list any more".into())
}

/// The last listing used for thumbnails, so a flyout's dozen thumbnail requests read the folder
/// once rather than once each. Opening a file always checks a fresh listing.
static THUMB_LIST: std::sync::Mutex<Option<(std::time::Instant, String, Vec<Item>)>> =
    std::sync::Mutex::new(None);

fn check_for_thumb(kind: &str, path: &str) -> Option<Item> {
    let mut g = THUMB_LIST.lock().unwrap_or_else(|e| e.into_inner());
    let stale = match g.as_ref() {
        Some((at, k, _)) => k != kind || at.elapsed() > std::time::Duration::from_secs(10),
        None => true,
    };
    if stale {
        *g = Some((std::time::Instant::now(), kind.to_string(), list(kind, 200)));
    }
    g.as_ref()?.2.iter().find(|i| i.path == path).cloned()
}

pub fn thumb(kind: &str, path: &str) -> Option<String> {
    let it = check_for_thumb(kind, path)?;
    #[cfg(windows)]
    return crate::apps::win::thumbnail(&it.path, 96);
    #[allow(unreachable_code)]
    None
}

/// "open" (the default app) or "reveal" (Explorer with it selected).
pub fn open(kind: &str, path: &str, how: &str) -> Result<(), String> {
    if how == "folder" {
        let dir = folder(kind).ok_or("That folder doesn't exist")?;
        return tauri_plugin_opener::open_path(dir.to_string_lossy().to_string(), None::<&str>)
            .map_err(|e| e.to_string());
    }
    let it = check(kind, path)?;
    #[cfg(windows)]
    if kind == "recent" && how == "open" {
        let target =
            crate::apps::win::lnk_target(&it.path).ok_or("That shortcut doesn't point anywhere")?;
        if runs_code(&target) {
            return Err("That shortcut starts a program, so it isn't opened from here".into());
        }
    }
    match how {
        "open" => tauri_plugin_opener::open_path(&it.path, None::<&str>).map_err(|e| e.to_string()),
        "reveal" => tauri_plugin_opener::reveal_item_in_dir(&it.path).map_err(|e| e.to_string()),
        _ => Err("unknown action".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_targets() {
        assert!(runs_code(r"C:\Windows\System32\cmd.exe"));
        assert!(runs_code(r"C:\x\setup.MSI"));
        assert!(runs_code(r"C:\x\a.ps1"));
        assert!(!runs_code(r"C:\Users\alex\Downloads\paper.docx"));
        assert!(!runs_code(r"C:\Users\alex\Programming"));
    }

    #[test]
    fn hides_partial_and_system_files() {
        assert!(shown("report.pdf"));
        assert!(shown("Screenshot 2026-11-16 024512.png"));
        assert!(!shown("setup.exe.crdownload"));
        assert!(!shown("movie.mkv.part"));
        assert!(!shown("desktop.ini"));
        assert!(!shown("Thumbs.db"));
        assert!(!shown(".hidden"));
    }

    #[test]
    fn recent_names_drop_the_shortcut_suffix() {
        assert_eq!(recent_name("report.pdf.lnk").as_deref(), Some("report.pdf"));
        assert_eq!(
            recent_name("Programming.LNK").as_deref(),
            Some("Programming")
        );
        assert_eq!(recent_name("notes.txt"), None);
        assert_eq!(recent_name(".lnk"), None);
    }
}
