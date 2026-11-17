//! freedock's own Start menu (popup mode "start", src/popup/StartView.tsx): search over every
//! installed app, Settings pages and recent files; a pinned grid; recent apps; all apps A-Z; a power
//! row. Windows' Start can't be restyled, so this one wears the dock's look; the Windows one is one
//! click away in its footer.
//!
//! Apps come from the shell's Applications folder (shell:AppsFolder), which lists desktop programs and
//! Store apps alike; each launches by its `shell:AppsFolder\<id>` name. The list is cached and only
//! re-read when the Start Menu folders change or after 10 minutes.
//!
//! Search (`win::find`) works like Windows' own Start search: those apps plus the Windows Search index
//! (programs with no Start Menu shortcut, folders, documents, photos...), the Settings search index,
//! and apps you start more often ranked higher. See winsearch.rs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StartApp {
    pub name: String,
    /// `shell:AppsFolder\<id>`: what launches it, and the key for its icon.
    pub id: String,
}

/// How well an app name answers a search (lower is better; None = no match): the whole name, its
/// start, the start of a word, its initials ("vsc" for Visual Studio Code), anywhere in it.
pub fn rank(name: &str, q: &str) -> Option<u32> {
    let (n, q) = (name.to_lowercase(), q.trim().to_lowercase());
    if q.is_empty() {
        return None;
    }
    let words: Vec<&str> = n
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let initials: String = words.iter().filter_map(|w| w.chars().next()).collect();
    if n == q {
        Some(0)
    } else if n.starts_with(&q) {
        Some(1)
    } else if words.iter().any(|w| w.starts_with(&q)) {
        Some(2)
    } else if q.len() >= 2 && initials.starts_with(&q) {
        Some(3)
    } else if n.contains(&q) {
        Some(4)
    } else {
        None
    }
}

/// Uninstallers and help files clutter the list; Windows' own Start hides most of them too.
pub fn is_clutter(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "uninstall",
        "readme",
        "release notes",
        "documentation",
        "help and support",
    ]
    .iter()
    .any(|w| n.contains(w))
}

/// Help files, readmes and web links that installers put in the Start Menu next to their program.
pub fn is_document(id: &str) -> bool {
    let id = id.to_lowercase();
    [
        ".chm", ".txt", ".pdf", ".htm", ".html", ".rtf", ".url", ".ini", ".log",
    ]
    .iter()
    .any(|e| id.ends_with(e))
}

/// Start's search results, grouped the way Windows' Start groups them.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Found {
    /// Apps and programs, best first (the first is the best match).
    pub apps: Vec<crate::winsearch::Hit>,
    pub settings: Vec<crate::winsearch::Hit>,
    /// Folders, documents, photos, videos and music from the index.
    pub files: Vec<crate::winsearch::Hit>,
    /// False while the Settings index is still being built the first time.
    pub settings_ready: bool,
}

/// One candidate app for search: how well its name matches, whether it's installed (`false`: a
/// program the index found elsewhere), how often it was started, the hit.
pub type Candidate = (u32, bool, u32, crate::winsearch::Hit);

/// Best first: name match, then installed apps before other programs (so an indexed program can't
/// outrank or stand in for an installed app of the same name), then launch count, then shorter
/// names. One result per name, and the installed one wins a tie.
pub fn order_apps(mut c: Vec<Candidate>, n: usize) -> Vec<crate::winsearch::Hit> {
    c.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(b.1.cmp(&a.1))
            .then(b.2.cmp(&a.2))
            .then(a.3.name.len().cmp(&b.3.name.len()))
            .then(a.3.name.cmp(&b.3.name))
    });
    let mut seen = std::collections::HashSet::new();
    c.into_iter()
        .filter(|x| seen.insert(x.3.name.to_lowercase()))
        .take(n)
        .map(|x| x.3)
        .collect()
}

/// What the Start menu remembers: its pins (in order) and the apps launched from it lately.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct StartState {
    pub pins: Vec<String>,
    pub recent: Vec<String>,
}

impl StartState {
    pub fn launched(&mut self, id: &str) {
        self.recent.retain(|r| r != id);
        self.recent.insert(0, id.to_string());
        self.recent.truncate(8);
    }
    pub fn pin(&mut self, id: &str, on: bool) {
        self.pins.retain(|p| p != id);
        if on {
            self.pins.push(id.to_string());
        }
    }
}

fn state_file() -> std::path::PathBuf {
    crate::settings::dir().join("start.json")
}

pub fn load_state() -> StartState {
    load_for_change().unwrap_or_default()
}

/// The state to change and save back. A file that can't be read right now (another program has it
/// locked) is an error, so the caller doesn't save an empty state over the user's pins; an unreadable
/// file is kept as start.json.bad and starts over.
pub fn load_for_change() -> Result<StartState, String> {
    let f = state_file();
    match std::fs::read_to_string(&f) {
        Ok(s) => Ok(serde_json::from_str(&s).unwrap_or_else(|_| {
            let _ = std::fs::copy(&f, f.with_extension("json.bad"));
            StartState::default()
        })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(StartState::default()),
        Err(e) => Err(format!("Start's saved state can't be read: {e}")),
    }
}

pub fn save_state(s: &StartState) -> Result<(), String> {
    let _ = std::fs::create_dir_all(crate::settings::dir());
    let f = state_file();
    let tmp = f.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_string_pretty(s).map_err(|e| e.to_string())?,
    )
    .and_then(|_| std::fs::rename(&tmp, &f))
    .map_err(|e| e.to_string())
}

/// Only these may be opened from the Start menu: apps by their shell name, Settings pages, and
/// files or folders that exist.
pub fn allowed(target: &str) -> bool {
    // Network paths are refused: checking one can hang for the network timeout and sends your
    // Windows sign-in to that host.
    let network = target.starts_with(r"\\") || target.starts_with("//");
    target.starts_with(r"shell:AppsFolder\")
        || target.starts_with("ms-settings:")
        || !network
            && std::path::Path::new(target).is_absolute()
            && std::path::Path::new(target).exists()
}

/// A query made safe for a URL's query string.
pub fn url_encode(q: &str) -> String {
    q.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(windows)]
pub mod win {
    use super::StartApp;
    use std::sync::Mutex;
    use std::time::{Duration, Instant, SystemTime};
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree};
    use windows::Win32::UI::Shell::{
        BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, KF_FLAG_DEFAULT,
        SHGetKnownFolderItem, SIGDN, SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING,
        ShellExecuteW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, PCWSTR, w};

    type Stamp = Vec<Option<SystemTime>>;
    static CACHE: Mutex<Option<(Instant, Stamp, Vec<StartApp>)>> = Mutex::new(None);

    /// The Start Menu folders' change times: a new or removed shortcut changes one of them.
    fn stamp() -> Stamp {
        ["ProgramData", "APPDATA"]
            .iter()
            .map(|v| {
                let base = std::env::var_os(v)?;
                let sub = if *v == "ProgramData" {
                    r"Microsoft\Windows\Start Menu\Programs"
                } else {
                    r"Microsoft\Windows\Start Menu\Programs"
                };
                std::fs::metadata(std::path::Path::new(&base).join(sub))
                    .and_then(|m| m.modified())
                    .ok()
            })
            .collect()
    }

    pub fn apps() -> Vec<StartApp> {
        let now = stamp();
        if let Some((at, st, list)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if *st == now && at.elapsed() < Duration::from_secs(600) {
                return list.clone();
            }
        }
        let list = read();
        *CACHE.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((Instant::now(), now, list.clone()));
        list
    }

    fn name_of(item: &IShellItem, kind: SIGDN) -> Option<String> {
        unsafe {
            let p = item.GetDisplayName(kind).ok()?;
            let s = p.to_string().ok();
            CoTaskMemFree(Some(p.0 as *const _));
            s
        }
    }

    fn read() -> Vec<StartApp> {
        crate::sta::run(read_here).unwrap_or_default()
    }

    fn read_here() -> Vec<StartApp> {
        let mut out = Vec::new();
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let Ok(folder) =
                SHGetKnownFolderItem::<IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)
            else {
                return out;
            };
            let Ok(items) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems)
            else {
                return out;
            };
            loop {
                let mut one = [None];
                let mut got = 0u32;
                if items.Next(&mut one, Some(&mut got)).is_err() || got == 0 {
                    break;
                }
                let Some(item) = one[0].take() else { break };
                let (Some(name), Some(id)) = (
                    name_of(&item, SIGDN_NORMALDISPLAY),
                    name_of(&item, SIGDN_PARENTRELATIVEPARSING),
                ) else {
                    continue;
                };
                if name.is_empty() || super::is_clutter(&name) || super::is_document(&id) {
                    continue;
                }
                out.push(StartApp {
                    name,
                    id: format!(r"shell:AppsFolder\{id}"),
                });
            }
        }
        out.sort_by_key(|a| a.name.to_lowercase());
        out.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
        out
    }

    /// Open it; `admin` asks Windows to run it as administrator (Windows shows its own prompt).
    pub fn open(target: &str, admin: bool) {
        unsafe {
            let verb = if admin { w!("runas") } else { w!("open") };
            ShellExecuteW(
                None,
                verb,
                &HSTRING::from(target),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }

    /// Show a file or folder selected in File Explorer ("Open file location").
    pub fn reveal(path: &str) {
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                w!("explorer.exe"),
                &HSTRING::from(format!("/select,\"{path}\"")),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }

    /// Search like Windows' Start: Start Menu apps and indexed programs ranked by name match and
    /// how often they're started, the Settings index, and indexed files by kind. Without `index`, only
    /// the instant part (Start Menu apps and Settings), so the menu answers on the first keystroke.
    pub fn find(q: &str, index: bool) -> super::Found {
        use crate::winsearch::{Hit, Kind, in_start_menu, same_key, win as ws};
        let q = q.trim();
        if q.is_empty() {
            return super::Found::default();
        }
        let counts = ws::launch_counts();
        let folders = ws::known_folders();
        let uses = |id: &str| counts.get(&same_key(id, folders)).copied().unwrap_or(0);
        let all = apps();
        let mut known: std::collections::HashSet<String> =
            all.iter().map(|a| same_key(&a.id, folders)).collect();
        let mut cands: Vec<super::Candidate> = all
            .iter()
            .filter_map(|a| {
                super::rank(&a.name, q).map(|r| {
                    (
                        r,
                        true,
                        uses(&a.id),
                        Hit {
                            name: a.name.clone(),
                            target: a.id.clone(),
                            kind: "app".into(),
                            place: None,
                        },
                    )
                })
            })
            .collect();
        let mut files: Vec<(u32, Hit)> = Vec::new();
        let place = |p: &str| {
            std::path::Path::new(p)
                .parent()
                .map(|d| d.to_string_lossy().to_string())
        };
        let downloads = folders
            .get("374de290-123f-4565-9164-39c4925e467b")
            .map(String::as_str); // FOLDERID_Downloads
        let temp = std::env::temp_dir().to_string_lossy().to_string();
        for (path, kind) in if index { ws::index(q, 200) } else { vec![] } {
            let file = std::path::Path::new(&path);
            let stem = file
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            // The index matched a word of the name; the name ranking may still miss ("101" in "MATH 101").
            let r = super::rank(&stem, q).unwrap_or(5);
            if kind == Kind::Program
                && !crate::winsearch::untrusted_place(&path, downloads, Some(&temp))
            {
                if in_start_menu(&path) || !known.insert(same_key(&path, folders)) {
                    continue; // already listed as a Start Menu app
                }
                cands.push((
                    r,
                    false,
                    uses(&path),
                    Hit {
                        name: stem,
                        target: path.clone(),
                        kind: "app".into(),
                        place: place(&path),
                    },
                ));
            } else {
                let name = file
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let k = serde_json::to_value(kind)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default();
                files.push((
                    r,
                    Hit {
                        name,
                        target: path.clone(),
                        kind: k,
                        place: place(&path),
                    },
                ));
            }
        }
        // Best name match first; among equals, shallower paths (Desktop, Documents) before deep ones.
        files.sort_by_key(|f| (f.0, f.1.target.matches('\\').count()));
        let sets = ws::settings_index();
        let settings = crate::winsearch::search_settings(&sets, q, 4)
            .into_iter()
            .map(|s| Hit {
                name: s.name,
                target: s.uri,
                kind: "setting".into(),
                place: None,
            })
            .collect();
        super::Found {
            apps: super::order_apps(cands, 8),
            settings,
            files: files.into_iter().take(10).map(|f| f.1).collect(),
            settings_ready: !sets.is_empty(),
        }
    }

    /// Explorer's own file search for `q` in the user's folder.
    pub fn search_files(q: &str) {
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        let uri = format!(
            "search-ms:query={}&crumb=location:{}",
            super::url_encode(q),
            super::url_encode(&home)
        );
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                w!("explorer.exe"),
                &HSTRING::from(uri),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_names() {
        assert_eq!(rank("Visual Studio Code", "visual studio code"), Some(0));
        assert_eq!(rank("Visual Studio Code", "vis"), Some(1));
        assert_eq!(rank("Visual Studio Code", "code"), Some(2));
        assert_eq!(rank("Visual Studio Code", "vsc"), Some(3));
        assert_eq!(rank("Notepad", "pad"), Some(4));
        assert_eq!(rank("Notepad", "x"), None);
        assert_eq!(rank("Notepad", "  "), None);
    }

    #[test]
    fn orders_apps_by_match_then_use() {
        let h = |n: &str, t: &str| crate::winsearch::Hit {
            name: n.into(),
            target: t.into(),
            kind: "app".into(),
            place: None,
        };
        let got = order_apps(
            vec![
                (2, true, 0, h("Visual Studio", "a")),
                (1, true, 3, h("ThrottleStop", "b")),
                (1, true, 40, h("Thunderbird", "c")),
                (1, true, 3, h("Tor", "d")),
                (2, true, 9, h("thunderbird", "e")),
            ],
            3,
        );
        assert_eq!(
            got.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
            vec!["Thunderbird", "Tor", "ThrottleStop"]
        );
        // A program the index found, however often started, never beats or replaces an installed app
        // of the same name.
        let got = order_apps(
            vec![
                (1, false, 999, h("Chrome", r"C:\Tools\Chrome.exe")),
                (1, true, 0, h("Chrome", r"shell:AppsFolder\Chrome")),
            ],
            5,
        );
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].target, r"shell:AppsFolder\Chrome");
    }

    #[test]
    fn remembers_recent_and_pins() {
        let mut s = StartState::default();
        for i in 0..10 {
            s.launched(&i.to_string());
        }
        s.launched("3");
        assert_eq!(s.recent.len(), 8);
        assert_eq!(s.recent[0], "3");
        s.pin("a", true);
        s.pin("b", true);
        s.pin("a", true);
        assert_eq!(s.pins, ["b", "a"]);
        s.pin("b", false);
        assert_eq!(s.pins, ["a"]);
    }

    #[test]
    fn only_safe_targets_open() {
        assert!(allowed(
            r"shell:AppsFolder\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"
        ));
        assert!(allowed("ms-settings:display"));
        assert!(!allowed("https://example.com"));
        assert!(!allowed("calc.exe"));
        assert!(!allowed(r"\\server\share\x.exe"));
        assert!(!allowed(r"\\?\UNC\server\share\x.exe"));
        assert_eq!(url_encode("a b&c"), "a+b%26c");
        assert!(is_document(
            "{X}/7-Zip/7-zip.chm".replace('/', "\\").as_str()
        ));
        assert!(!is_document(
            "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"
        ));
    }
}

#[cfg(all(test, windows))]
mod live {
    /// Reads this PC's real app list: `cargo test lists_installed_apps -- --ignored --nocapture`.
    /// Live check against this PC's index: `cargo test finds_like_windows_start -- --ignored --nocapture`.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn finds_like_windows_start() {
        for q in ["thrott", "chrome", "mouse", "math 101", "explorer"] {
            let t = std::time::Instant::now();
            let f = super::win::find(q, true);
            println!(
                "{q:?} in {} ms, settings ready: {}",
                t.elapsed().as_millis(),
                f.settings_ready
            );
            for h in f.apps.iter().chain(&f.settings).chain(&f.files).take(9) {
                println!("  {:9} {}  <{}>", h.kind, h.name, h.target);
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(8));
        let f = super::win::find("mouse", true);
        println!(
            "after build: settings ready {} -> {:?}",
            f.settings_ready,
            f.settings
                .iter()
                .map(|h| (&h.name, &h.target))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore]
    fn lists_installed_apps() {
        let apps = super::win::apps();
        println!(
            "{} apps; first: {:?}",
            apps.len(),
            apps.iter().take(3).collect::<Vec<_>>()
        );
        assert!(apps.len() > 5);
        assert!(apps.iter().all(|a| a.id.starts_with(r"shell:AppsFolder\")));
    }
}
