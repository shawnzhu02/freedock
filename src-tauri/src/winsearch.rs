//! What Windows' own Start search uses, so freedock's Start finds the same things:
//!
//! - **The Windows Search index** (the "SystemIndex" catalog the Windows Search service keeps, over
//!   whatever locations the user indexes in Settings > Privacy & security > Searching Windows). It is
//!   queried with Windows Search SQL through its OLE DB provider, the documented way. This is what
//!   finds portable programs like `C:\Tools\ThrottleStop\ThrottleStop.exe` that have no Start Menu
//!   shortcut, and documents, folders and photos by name.
//! - **Launch counts** from the shell's UserAssist records (HKCU\...\Explorer\UserAssist), the
//!   history Windows uses to put the apps you use most at the top.
//! - **The Settings search index**: `C:\Windows\ImmersiveControlPanel\Settings\AllSystemSettings_*.xml`,
//!   the list (with search keywords) that Windows' Settings and Start search over. Its text lives in
//!   Windows' resource files and is resolved once, then cached in `settings-index.json`.
//!
//! The pure parts (query building, sorting results into kinds, the UserAssist record format, the
//! Settings XML) are tested below; the Windows calls are in `win`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The words of a search, made safe for a Windows Search SQL string: letters and digits only (so no
/// quote can end the string), at most 6 words of at most 40 characters each.
pub fn query_words(q: &str) -> Vec<String> {
    q.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(6)
        .map(|w| w.chars().take(40).collect())
        .collect()
}

/// Build output and dependency folders: indexed, but thousands of files nobody searches Start for.
/// Filtered out of the results here, not in the query: a `NOT LIKE` on the path makes the index scan
/// every match and turned 100 ms searches into seconds.
const NOISE: [&str; 7] = [
    "\\node_modules\\",
    "\\.git\\",
    "\\__pycache__\\",
    "\\target\\debug\\",
    "\\target\\release\\",
    "\\.venv\\",
    "\\site-packages\\",
];

/// File types Start lists as apps.
const PROGRAMS: [&str; 4] = [".exe", ".lnk", ".appref-ms", ".msc"];

/// The SQL for an index search: file names with a word starting with each search word (the way
/// Start matches "thrott" to ThrottleStop.exe), files only (not mail). `programs`: only programs (a
/// separate query, so a program is never crowded out by a pile of documents with the same word).
/// No ORDER BY: sorting every match costs far more than the search; results are ranked by name here.
pub fn index_sql(words: &[String], top: usize, programs: bool) -> Option<String> {
    if words.is_empty() {
        return None;
    }
    let terms: Vec<String> = words.iter().map(|w| format!("\"{w}*\"")).collect();
    let only = if programs {
        format!(
            " AND ({})",
            PROGRAMS
                .iter()
                .map(|t| format!("System.ItemType = '{t}'"))
                .collect::<Vec<_>>()
                .join(" OR ")
        )
    } else {
        String::new()
    };
    Some(format!(
        "SELECT TOP {top} System.ItemPathDisplay, System.ItemType FROM SystemIndex \
         WHERE SCOPE='file:' AND CONTAINS(System.FileName, '{}'){only}",
        terms.join(" AND ")
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Program,
    Folder,
    Photo,
    Video,
    Music,
    Document,
}

/// What an index hit is, from its type (".exe", "Directory"...), the way Start groups results.
pub fn kind_of(item_type: &str) -> Kind {
    let t = item_type.to_lowercase();
    match t.as_str() {
        "directory" | "folder" => Kind::Folder,
        t if PROGRAMS.contains(&t) => Kind::Program,
        ".png" | ".jpg" | ".jpeg" | ".gif" | ".bmp" | ".webp" | ".heic" | ".tif" | ".tiff"
        | ".svg" | ".ico" | ".raw" | ".avif" => Kind::Photo,
        ".mp4" | ".mov" | ".mkv" | ".avi" | ".wmv" | ".webm" | ".m4v" => Kind::Video,
        ".mp3" | ".wav" | ".flac" | ".m4a" | ".aac" | ".ogg" | ".wma" => Kind::Music,
        _ => Kind::Document,
    }
}

/// The same build folders, checked again on the results (the query's LIKE is the main filter).
pub fn is_noise(path: &str) -> bool {
    let p = path.to_lowercase();
    NOISE.iter().any(|n| p.contains(n)) || p.ends_with("\\node_modules") || p.ends_with("\\.git")
}

/// Places anything can drop a program into without installing it: Downloads, temp folders, browser
/// and mail caches, the Recycle Bin. A program found there is listed as a file, never as an app or
/// the best match, so a downloaded "Chrome.exe" can't stand in for the installed Chrome.
pub fn untrusted_place(path: &str, downloads: Option<&str>, temp: Option<&str>) -> bool {
    let p = path.to_lowercase();
    let under = |dir: Option<&str>| {
        dir.is_some_and(|d| {
            !d.is_empty()
                && p.starts_with(&format!("{}\\", d.trim_end_matches('\\').to_lowercase()))
        })
    };
    under(downloads)
        || under(temp)
        || [
            "\\appdata\\local\\temp\\",
            "\\inetcache\\",
            "\\content.outlook\\",
            "\\$recycle.bin\\",
            "\\windows\\temp\\",
        ]
        .iter()
        .any(|n| p.contains(n))
}

/// Shortcuts in the Start Menu folders are already in the apps list (shell:AppsFolder).
pub fn in_start_menu(path: &str) -> bool {
    path.to_lowercase()
        .contains("\\microsoft\\windows\\start menu\\")
}

/// UserAssist names are ROT13-encoded (letters only).
pub fn rot13(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' => (((c as u8 - b'a') + 13) % 26 + b'a') as char,
            'A'..='Z' => (((c as u8 - b'A') + 13) % 26 + b'A') as char,
            _ => c,
        })
        .collect()
}

/// A UserAssist record (Windows 7 and later, 72 bytes): the run count is the second 32-bit number.
pub fn launch_count(data: &[u8]) -> Option<u32> {
    (data.len() >= 16).then(|| u32::from_le_bytes([data[4], data[5], data[6], data[7]]))
}

/// One entry of the Settings search index, before its text is looked up.
#[derive(Debug, Clone, PartialEq)]
pub struct RawSetting {
    /// Resource references ("@{windows?ms-resource://...}") or plain text.
    pub name: String,
    pub keywords: Vec<String>,
    /// The `ms-settings:` page that opens it.
    pub uri: String,
}

fn tag<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let i = s.find(&open)? + open.len();
    let j = s[i..].find(&format!("</{name}>"))? + i;
    Some(s[i..j].trim())
}

/// Parse `AllSystemSettings_*.xml`. Each `<SearchableContent>` has a description (what Start shows),
/// keywords, and `PolicyIds`: the page names `ms-settings:` takes (the same names the
/// SettingsPageVisibility policy uses), most specific first. Entries without a page are skipped.
pub fn parse_settings_xml(xml: &str) -> Vec<RawSetting> {
    let mut out = Vec::new();
    for block in xml.split("<SearchableContent").skip(1) {
        let block = block.split("</SearchableContent>").next().unwrap_or("");
        let (Some(name), Some(ids)) = (tag(block, "Description"), tag(block, "PolicyIds")) else {
            continue;
        };
        let Some(page) = ids.split(';').map(str::trim).find(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        }) else {
            continue;
        };
        let keywords = ["HighKeywords", "Keywords"]
            .iter()
            .filter_map(|k| tag(block, k))
            .map(String::from)
            .collect();
        out.push(RawSetting {
            name: name.to_string(),
            keywords,
            uri: format!("ms-settings:{page}"),
        });
    }
    out
}

/// A Settings search entry with its text looked up.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Setting {
    pub name: String,
    pub keywords: Vec<String>,
    pub uri: String,
}

/// How well a setting answers `q` (lower is better): by its name, else by a keyword (one step worse).
pub fn rank_setting(s: &Setting, q: &str) -> Option<u32> {
    let by_name = crate::startmenu::rank(&s.name, q);
    let by_word = s
        .keywords
        .iter()
        .filter_map(|k| crate::startmenu::rank(k, q))
        .min()
        .map(|r| r + 1);
    match (by_name, by_word) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// The best `n` settings for `q`, best first (shorter names first among equals), one per name.
pub fn search_settings(all: &[Setting], q: &str, n: usize) -> Vec<Setting> {
    let mut hits: Vec<(u32, &Setting)> = all
        .iter()
        .filter_map(|s| rank_setting(s, q).map(|r| (r, s)))
        .collect();
    hits.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.name.len().cmp(&b.1.name.len())));
    let mut seen = std::collections::HashSet::new();
    hits.into_iter()
        .filter(|(_, s)| seen.insert(s.name.to_lowercase()))
        .take(n)
        .map(|(_, s)| s.clone())
        .collect()
}

/// One search result for the Start menu.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Hit {
    pub name: String,
    /// What opens it: `shell:AppsFolder\...`, a path, or an `ms-settings:` page.
    pub target: String,
    /// "app", "setting", or a file `Kind` ("program", "folder", "document", "photo"...).
    pub kind: String,
    /// The folder a file is in (shown under its name).
    pub place: Option<String>,
}

/// Lowercased, with a known-folder GUID prefix spelled out, so a Start Menu app id, a UserAssist
/// name and an index path for the same program compare equal.
pub fn same_key(id: &str, folders: &HashMap<String, String>) -> String {
    let id = id.strip_prefix(r"shell:AppsFolder\").unwrap_or(id);
    if let Some(rest) = id.strip_prefix('{') {
        if let Some((guid, tail)) = rest.split_once('}') {
            if let Some(base) = folders.get(&guid.to_lowercase()) {
                return format!("{base}{tail}").to_lowercase();
            }
        }
    }
    id.to_lowercase()
}

#[cfg(windows)]
pub mod win {
    use super::*;
    use crate::sta::Lane;
    use std::sync::Mutex;
    use std::time::{Duration, Instant, SystemTime};
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
    use windows::Win32::System::Search::*;
    use windows::core::{GUID, HSTRING, IUnknown, Interface};

    static LANE: Lane = Lane::new("index-search", false);
    /// The Windows Search OLE DB provider ("Search.CollatorDSO").
    const CLSID_COLLATOR: GUID = GUID::from_u128(0x9E175B8B_F52A_11D8_B9A5_505054503030);
    const DBGUID_DEFAULT: GUID = GUID::from_u128(0xc8b521fb_5cf3_11ce_ade5_00aa0044773d);

    thread_local! {
        /// The open session, kept on the search lane so each search skips the connection setup.
        static SESSION: std::cell::RefCell<Option<IDBCreateCommand>> = const { std::cell::RefCell::new(None) };
    }

    fn session() -> Option<IDBCreateCommand> {
        if let Some(s) = SESSION.with(|s| s.borrow().clone()) {
            return Some(s);
        }
        unsafe {
            let init: IDBInitialize =
                CoCreateInstance(&CLSID_COLLATOR, None, CLSCTX_INPROC_SERVER).ok()?;
            init.Initialize().ok()?;
            let create: IDBCreateSession = init.cast().ok()?;
            let s: IDBCreateCommand = create
                .CreateSession(None, &IDBCreateCommand::IID)
                .ok()?
                .cast()
                .ok()?;
            SESSION.with(|c| *c.borrow_mut() = Some(s.clone()));
            Some(s)
        }
    }

    /// One text column as the provider hands it over: status, length in bytes, then the characters.
    #[repr(C)]
    struct Col {
        status: u32,
        len: usize,
        text: [u16; 1024],
    }
    #[repr(C)]
    struct Row {
        path: Col,
        kind: Col,
    }

    fn col_text(c: &Col) -> Option<String> {
        if c.status != DBSTATUS_S_OK.0 as u32 {
            return None; // null, or a path too long for the buffer
        }
        let n = (c.len / 2).min(c.text.len());
        Some(String::from_utf16_lossy(&c.text[..n]))
    }

    fn run_sql(sql: &str) -> Option<Vec<(String, String)>> {
        let make = session()?;
        let got = unsafe { rows(&make, sql) };
        if got.is_none() {
            // The service may have restarted: reconnect on the next search.
            SESSION.with(|s| *s.borrow_mut() = None);
        }
        got
    }

    unsafe fn rows(make: &IDBCreateCommand, sql: &str) -> Option<Vec<(String, String)>> {
        unsafe {
            let cmd: ICommandText = make
                .CreateCommand(None, &ICommandText::IID)
                .ok()?
                .cast()
                .ok()?;
            cmd.SetCommandText(&DBGUID_DEFAULT, &HSTRING::from(sql))
                .ok()?;
            let mut out: Option<IUnknown> = None;
            cmd.Execute(None, &IRowset::IID, None, None, Some(&mut out))
                .ok()?;
            let rowset: IRowset = out?.cast().ok()?;
            let acc: IAccessor = rowset.cast().ok()?;
            let bind = |ord: usize, at: usize| DBBINDING {
                iOrdinal: ord,
                obValue: at + std::mem::offset_of!(Col, text),
                obLength: at + std::mem::offset_of!(Col, len),
                obStatus: at + std::mem::offset_of!(Col, status),
                dwPart: (DBPART_VALUE.0 | DBPART_LENGTH.0 | DBPART_STATUS.0) as u32,
                dwMemOwner: DBMEMOWNER_CLIENTOWNED.0 as u32,
                eParamIO: DBPARAMIO_NOTPARAM.0 as u32,
                cbMaxLen: std::mem::size_of::<[u16; 1024]>(),
                wType: DBTYPE_WSTR.0 as u16,
                ..Default::default()
            };
            let binds = [
                bind(1, std::mem::offset_of!(Row, path)),
                bind(2, std::mem::offset_of!(Row, kind)),
            ];
            let mut h = HACCESSOR::default();
            acc.CreateAccessor(
                DBACCESSOR_ROWDATA.0 as u32,
                binds.len(),
                binds.as_ptr(),
                std::mem::size_of::<Row>(),
                &mut h,
                None,
            )
            .ok()?;
            let mut found = Vec::new();
            let mut row = Box::new(std::mem::zeroed::<Row>());
            loop {
                const N: usize = 20;
                let mut handles = [0usize; N];
                // The binding takes the HROW** as a slice whose length is the row count asked for;
                // only its first element (our handle buffer) is read.
                let mut slot = [std::ptr::null_mut::<usize>(); N];
                slot[0] = handles.as_mut_ptr();
                let mut got = 0usize;
                let more = rowset.GetNextRows(0, 0, &mut got, &mut slot);
                for &hrow in &handles[..got.min(N)] {
                    if rowset
                        .GetData(hrow, h, &mut *row as *mut Row as *mut _)
                        .is_ok()
                    {
                        if let (Some(p), k) = (col_text(&row.path), col_text(&row.kind)) {
                            found.push((p, k.unwrap_or_default()));
                        }
                    }
                }
                if got > 0 {
                    let _ = rowset.ReleaseRows(
                        got,
                        handles.as_ptr(),
                        std::ptr::null(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    );
                }
                if more.is_err() || got < N {
                    break;
                }
            }
            let _ = acc.ReleaseAccessor(h, None);
            Some(found)
        }
    }

    /// The newest search; a queued search that is no longer it is skipped (typing outruns the index).
    static LATEST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    /// Index hits for `q`: (path, kind), programs first, without build and dependency folders.
    /// Empty for a single letter (it matches most of the disk) or when a newer search has started.
    pub fn index(q: &str, top: usize) -> Vec<(String, Kind)> {
        use std::sync::atomic::Ordering::SeqCst;
        let words = query_words(q);
        if words.iter().map(|w| w.chars().count()).sum::<usize>() < 2 {
            return vec![];
        }
        let (Some(progs), Some(rest)) =
            (index_sql(&words, 40, true), index_sql(&words, top, false))
        else {
            return vec![];
        };
        let me = LATEST.fetch_add(1, SeqCst) + 1;
        let rows = LANE
            .run(move || {
                let mut out = Vec::new();
                for sql in [progs, rest] {
                    if LATEST.load(SeqCst) != me {
                        break;
                    }
                    out.extend(run_sql(&sql).unwrap_or_default());
                }
                out
            })
            .unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        rows.into_iter()
            .filter(|(p, _)| !is_noise(p) && seen.insert(p.to_lowercase()))
            .map(|(p, t)| (p, kind_of(&t)))
            .collect()
    }

    // ------------------------------------------------------------ launch counts

    /// Known-folder GUIDs (lowercase, no braces) to their paths, for `same_key`.
    pub fn known_folders() -> &'static HashMap<String, String> {
        static F: std::sync::OnceLock<HashMap<String, String>> = std::sync::OnceLock::new();
        F.get_or_init(|| {
            use windows::Win32::UI::Shell::*;
            let ids = [
                FOLDERID_ProgramFilesX64,
                FOLDERID_ProgramFilesX86,
                FOLDERID_ProgramFiles,
                FOLDERID_System,
                FOLDERID_SystemX86,
                FOLDERID_Windows,
                FOLDERID_LocalAppData,
                FOLDERID_RoamingAppData,
                FOLDERID_Profile,
                FOLDERID_UserProgramFiles,
                FOLDERID_Programs,
                FOLDERID_CommonPrograms,
                FOLDERID_Desktop,
                FOLDERID_Downloads,
                FOLDERID_Documents,
            ];
            let mut m = HashMap::new();
            for id in ids {
                unsafe {
                    if let Ok(p) = SHGetKnownFolderPath(&id, KNOWN_FOLDER_FLAG(0), None) {
                        if let Ok(s) = p.to_string() {
                            m.insert(
                                format!("{id:?}")
                                    .trim_matches(|c| c == '{' || c == '}')
                                    .to_lowercase(),
                                s,
                            );
                        }
                        CoTaskMemFree(Some(p.0 as *const _));
                    }
                }
            }
            m
        })
    }

    static COUNTS: Mutex<Option<(Instant, HashMap<String, u32>)>> = Mutex::new(None);

    /// How often each program was started from the shell, by `same_key`. Re-read every 30 seconds.
    pub fn launch_counts() -> HashMap<String, u32> {
        if let Some((at, m)) = COUNTS.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if at.elapsed() < Duration::from_secs(30) {
                return m.clone();
            }
        }
        let m = read_counts();
        *COUNTS.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), m.clone()));
        m
    }

    fn read_counts() -> HashMap<String, u32> {
        use windows::Win32::System::Registry::*;
        let folders = known_folders();
        let mut m: HashMap<String, u32> = HashMap::new();
        // Programs started directly, and programs started from shortcuts.
        for key in [
            "{CEBFF5CD-ACE2-4F4F-9178-9926F41749EA}",
            "{F4E57C4B-2036-45F0-A9AB-443BCFE33D9F}",
        ] {
            let path = format!(
                r"Software\Microsoft\Windows\CurrentVersion\Explorer\UserAssist\{key}\Count"
            );
            let mut hk = HKEY::default();
            unsafe {
                if RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    &HSTRING::from(path),
                    None,
                    KEY_READ,
                    &mut hk,
                )
                .is_err()
                {
                    continue;
                }
                for i in 0.. {
                    let mut name = [0u16; 1024];
                    let mut nlen = name.len() as u32;
                    let mut data = [0u8; 128];
                    let mut dlen = data.len() as u32;
                    let r = RegEnumValueW(
                        hk,
                        i,
                        Some(windows::core::PWSTR(name.as_mut_ptr())),
                        &mut nlen,
                        None,
                        None,
                        Some(data.as_mut_ptr()),
                        Some(&mut dlen),
                    );
                    if r.is_err() {
                        if r == windows::Win32::Foundation::ERROR_MORE_DATA {
                            continue;
                        }
                        break;
                    }
                    let Some(n) = launch_count(&data[..dlen as usize]) else {
                        continue;
                    };
                    if n == 0 {
                        continue;
                    }
                    let id = rot13(&String::from_utf16_lossy(&name[..nlen as usize]));
                    *m.entry(same_key(&id, folders)).or_default() += n;
                }
                let _ = RegCloseKey(hk);
            }
        }
        m
    }

    // ------------------------------------------------------------ the Settings index

    static SETTINGS: Mutex<Option<std::sync::Arc<Vec<Setting>>>> = Mutex::new(None);
    static BUILDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    fn xml_files() -> Vec<std::path::PathBuf> {
        let dir =
            std::path::Path::new(&std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()))
                .join(r"ImmersiveControlPanel\Settings");
        let mut v: Vec<_> = std::fs::read_dir(dir)
            .map(|d| {
                d.flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                            n.starts_with("AllSystemSettings_") && n.ends_with(".xml")
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }

    /// The files' change times and the display language: when either changes, rebuild.
    fn stamp() -> String {
        let times: Vec<String> = xml_files()
            .iter()
            .map(|p| {
                std::fs::metadata(p)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs().to_string())
                    .unwrap_or_default()
            })
            .collect();
        format!("{}|{}", times.join(","), unsafe {
            windows::Win32::Globalization::GetUserDefaultUILanguage()
        })
    }

    #[derive(Serialize, Deserialize)]
    struct Cached {
        stamp: String,
        items: Vec<Setting>,
    }

    fn cache_file() -> std::path::PathBuf {
        crate::settings::dir().join("settings-index.json")
    }

    fn resolve(s: &str) -> Option<String> {
        if !s.starts_with('@') {
            return Some(s.to_string());
        }
        let mut buf = [0u16; 1024];
        unsafe {
            windows::Win32::UI::Shell::SHLoadIndirectString(&HSTRING::from(s), &mut buf, None)
                .ok()?
        };
        let t = String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(0)]);
        (!t.trim().is_empty()).then(|| t.trim().to_string())
    }

    fn build() -> Vec<Setting> {
        let mut raw = Vec::new();
        for f in xml_files() {
            if let Ok(xml) = std::fs::read_to_string(&f) {
                raw.extend(parse_settings_xml(&xml));
            }
        }
        let mut seen = std::collections::HashSet::new();
        raw.into_iter()
            .filter(|r| seen.insert((r.name.clone(), r.uri.clone())))
            .filter_map(|r| {
                let name = resolve(&r.name)?;
                let keywords = r
                    .keywords
                    .iter()
                    .filter_map(|k| resolve(k))
                    .flat_map(|k| {
                        k.split(';')
                            .map(|w| w.trim().to_string())
                            .collect::<Vec<_>>()
                    })
                    .filter(|w| !w.is_empty())
                    .collect();
                Some(Setting {
                    name,
                    keywords,
                    uri: r.uri,
                })
            })
            .collect()
    }

    /// The Settings index: from memory, else the cache file, else built (about a second, once per
    /// Windows update or language change). Returns empty while a first build runs in the background.
    pub fn settings_index() -> std::sync::Arc<Vec<Setting>> {
        if let Some(s) = SETTINGS.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            return s.clone();
        }
        let stamp = stamp();
        if let Some(c) = std::fs::read_to_string(cache_file())
            .ok()
            .and_then(|t| serde_json::from_str::<Cached>(&t).ok())
        {
            if c.stamp == stamp && !c.items.is_empty() {
                let items = std::sync::Arc::new(c.items);
                *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = Some(items.clone());
                return items;
            }
        }
        if !BUILDING.swap(true, std::sync::atomic::Ordering::SeqCst) {
            std::thread::spawn(move || {
                let items = build();
                if !items.is_empty() {
                    let _ = std::fs::create_dir_all(crate::settings::dir());
                    if let Ok(t) = serde_json::to_string(&Cached {
                        stamp,
                        items: items.clone(),
                    }) {
                        let _ = std::fs::write(cache_file(), t);
                    }
                }
                // Kept even when empty (no settings index on this Windows), so searches don't rebuild
                // it every time; Start then uses its short built-in list of Settings pages.
                *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(std::sync::Arc::new(items));
                BUILDING.store(false, std::sync::atomic::Ordering::SeqCst);
            });
        }
        std::sync::Arc::new(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_safe_for_sql() {
        assert_eq!(query_words("thrott"), vec!["thrott"]);
        assert_eq!(query_words("  math 101 "), vec!["math", "101"]);
        assert_eq!(query_words("x' OR 1=1; --\""), vec!["x", "OR", "1", "1"]);
        assert!(query_words("'\"%;").is_empty());
        assert_eq!(query_words("café"), vec!["café"]);
    }

    #[test]
    fn builds_index_sql() {
        assert_eq!(index_sql(&[], 10, false), None);
        let sql = index_sql(&query_words("math 101"), 30, false).unwrap();
        assert!(
            sql.starts_with(
                "SELECT TOP 30 System.ItemPathDisplay, System.ItemType FROM SystemIndex"
            )
        );
        assert!(sql.contains("CONTAINS(System.FileName, '\"math*\" AND \"101*\"')"));
        assert!(sql.contains("SCOPE='file:'"));
        assert!(
            !sql.contains("LIKE") && !sql.contains("ORDER BY"),
            "both make the index scan every match"
        );
        assert!(!sql.contains("System.ItemType ="));
        let progs = index_sql(&query_words("thrott"), 15, true).unwrap();
        assert!(progs.contains("AND (System.ItemType = '.exe' OR System.ItemType = '.lnk'"));
    }

    #[test]
    fn sorts_hits_into_kinds() {
        assert_eq!(kind_of(".exe"), Kind::Program);
        assert_eq!(kind_of(".EXE"), Kind::Program);
        assert_eq!(kind_of("Directory"), Kind::Folder);
        assert_eq!(kind_of(".png"), Kind::Photo);
        assert_eq!(kind_of(".mp4"), Kind::Video);
        assert_eq!(kind_of(".flac"), Kind::Music);
        assert_eq!(kind_of(".pdf"), Kind::Document);
        assert_eq!(kind_of(""), Kind::Document);
    }

    #[test]
    fn skips_build_folders() {
        assert!(is_noise(
            r"C:\Users\alex\code\app\node_modules\lodash\index.js"
        ));
        assert!(is_noise(r"C:\Users\alex\code\app\.git\HEAD"));
        assert!(is_noise(r"C:\Users\alex\code\app\node_modules"));
        assert!(!is_noise(
            r"C:\Users\alex\Programs\ThrottleStop\ThrottleStop.exe"
        ));
        assert!(in_start_menu(
            r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Steam.lnk"
        ));
        let dl = Some(r"C:\Users\alex\Downloads");
        let tmp = Some(r"C:\Users\alex\AppData\Local\Temp\");
        assert!(untrusted_place(
            r"C:\Users\alex\Downloads\chrome.exe",
            dl,
            tmp
        ));
        assert!(untrusted_place(
            r"c:\users\alex\downloads\setup\Chrome.exe",
            dl,
            tmp
        ));
        assert!(untrusted_place(
            r"C:\Users\alex\AppData\Local\Temp\x\Chrome.exe",
            dl,
            None
        ));
        assert!(untrusted_place(
            r"C:\Users\alex\AppData\Local\Microsoft\Windows\INetCache\IE\a.exe",
            None,
            None
        ));
        assert!(!untrusted_place(
            r"C:\Users\alex\Downloadsfolder\a.exe",
            dl,
            tmp
        ));
        assert!(!untrusted_place(
            r"C:\Users\alex\Programs\ThrottleStop\ThrottleStop.exe",
            dl,
            tmp
        ));
        assert!(!in_start_menu(r"C:\Users\alex\Desktop\Steam.lnk"));
    }

    #[test]
    fn reads_userassist_records() {
        assert_eq!(
            rot13("P:\\Hfref\\nyrk\\GuebggyrFgbc.rkr"),
            "C:\\Users\\alex\\ThrottleStop.exe"
        );
        assert_eq!(
            rot13(&rot13("Microsoft.Windows.Explorer")),
            "Microsoft.Windows.Explorer"
        );
        let mut rec = [0u8; 72];
        rec[4..8].copy_from_slice(&17u32.to_le_bytes());
        assert_eq!(launch_count(&rec), Some(17));
        assert_eq!(launch_count(&[0u8; 8]), None);
    }

    #[test]
    fn matches_the_same_program_by_any_name() {
        let mut f = HashMap::new();
        f.insert(
            "6d809377-6af0-444b-8957-a3773f02200e".to_string(),
            r"C:\Program Files".to_string(),
        );
        assert_eq!(
            same_key(
                r"{6D809377-6AF0-444B-8957-A3773F02200E}\Steam\steam.exe",
                &f
            ),
            r"c:\program files\steam\steam.exe"
        );
        assert_eq!(
            same_key(
                r"shell:AppsFolder\{6D809377-6AF0-444B-8957-A3773F02200E}\Steam\steam.exe",
                &f
            ),
            r"c:\program files\steam\steam.exe"
        );
        assert_eq!(
            same_key(r"C:\Program Files\Steam\steam.exe", &f),
            r"c:\program files\steam\steam.exe"
        );
        assert_eq!(
            same_key("Microsoft.Windows.Explorer", &f),
            "microsoft.windows.explorer"
        );
    }

    const XML: &str = r#"<PCSettings>
  <SearchableContent ExcludeWithFeature="X">
    <Filename>AAA_Battery</Filename>
    <SettingIdentity><SettingPaths><Path>
      <PageID>SettingsPagePowerAndBattery</PageID>
      <PolicyIds>batterysaver;batterysaver-settings;powersleep</PolicyIds>
    </Path></SettingPaths></SettingIdentity>
    <SettingInformation>
      <Description>@{windows?ms-resource://Windows.UI.SettingsAppThreshold/SearchResources/Battery/Description}</Description>
      <HighKeywords>@{windows?ms-resource://Windows.UI.SettingsAppThreshold/SearchResources/Battery/HighKeywords}</HighKeywords>
    </SettingInformation>
  </SearchableContent>
  <SearchableContent>
    <Filename>Defender</Filename>
    <SettingIdentity><HostID>{x}</HostID><DeepLink>AccountProtection</DeepLink></SettingIdentity>
    <SettingInformation><Description>Account protection</Description></SettingInformation>
  </SearchableContent>
</PCSettings>"#;

    #[test]
    fn parses_the_settings_index() {
        let got = parse_settings_xml(XML);
        assert_eq!(got.len(), 1, "entries without a settings page are skipped");
        assert_eq!(got[0].uri, "ms-settings:batterysaver");
        assert!(got[0].name.starts_with("@{windows?ms-resource://"));
        assert_eq!(got[0].keywords.len(), 1);
    }

    #[test]
    fn ranks_settings_by_name_then_keywords() {
        let s = |n: &str, k: &[&str]| Setting {
            name: n.into(),
            keywords: k.iter().map(|w| w.to_string()).collect(),
            uri: format!("ms-settings:{n}"),
        };
        let all = vec![
            s("Change the mouse pointer size", &["cursor"]),
            s("Mouse settings", &["pointer"]),
            s("Night light", &["blue light"]),
            s("Mouse settings", &["dup"]),
        ];
        let got = search_settings(&all, "mouse", 5);
        assert_eq!(
            got.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["Mouse settings", "Change the mouse pointer size"]
        );
        assert_eq!(
            search_settings(&all, "cursor", 5)[0].name,
            "Change the mouse pointer size"
        );
        assert!(search_settings(&all, "zzz", 5).is_empty());
    }
}
