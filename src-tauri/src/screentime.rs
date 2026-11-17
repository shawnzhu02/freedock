//! Screen Time: which program is in front, sampled every 5 seconds while the user is at the PC (any input
//! in the last 5 minutes and the screen not locked). Only program paths and seconds per day are
//! kept, never window titles, in `%APPDATA%\freedock\screentime.json`, for the last 14 days.

use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// date ("2026-11-16") -> program path -> seconds.
pub type Days = BTreeMap<String, BTreeMap<String, u64>>;

pub const STEP_SECS: u64 = 5;
pub const IDLE_SECS: u64 = 300;
pub const KEEP_DAYS: usize = 14;

static DAYS: Mutex<Option<Days>> = Mutex::new(None);
static RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AppTime {
    pub exe: String,
    pub name: String,
    pub secs: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Day {
    pub date: String,
    pub total: u64,
    /// Most used first.
    pub apps: Vec<AppTime>,
}

/// Add time to a program on a day, dropping days beyond the newest `KEEP_DAYS`.
pub fn add(days: &mut Days, date: &str, exe: &str, secs: u64) {
    *days
        .entry(date.to_string())
        .or_default()
        .entry(exe.to_string())
        .or_default() += secs;
    while days.len() > KEEP_DAYS {
        let oldest = days.keys().next().cloned().unwrap_or_default();
        days.remove(&oldest);
    }
}

/// "C:\...\WindowsTerminal.exe" -> "WindowsTerminal".
pub fn name_of(exe: &str) -> String {
    let file = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    file.strip_suffix(".exe")
        .or_else(|| file.strip_suffix(".EXE"))
        .unwrap_or(file)
        .to_string()
}

/// The newest `n` days, newest first, each with programs sorted by time.
pub fn summary(days: &Days, n: usize) -> Vec<Day> {
    days.iter()
        .rev()
        .take(n)
        .map(|(date, apps)| {
            let mut list: Vec<AppTime> = apps
                .iter()
                .map(|(exe, &secs)| AppTime {
                    exe: exe.clone(),
                    name: name_of(exe),
                    secs,
                })
                .collect();
            list.sort_by(|a, b| b.secs.cmp(&a.secs).then(a.name.cmp(&b.name)));
            Day {
                date: date.clone(),
                total: list.iter().map(|a| a.secs).sum(),
                apps: list,
            }
        })
        .collect()
}

/// Programs that aren't really "using the PC": the lock screen and freedock itself.
pub fn ignored(exe: &str) -> bool {
    let n = name_of(exe).to_ascii_lowercase();
    matches!(
        n.as_str(),
        "lockapp" | "freedock" | "searchhost" | "shellexperiencehost" | "startmenuexperiencehost"
    )
}

fn file() -> std::path::PathBuf {
    crate::settings::dir().join("screentime.json")
}

fn load() -> Days {
    std::fs::read_to_string(file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(days: &Days) {
    let p = file();
    let tmp = p.with_extension("json.tmp");
    if let Ok(s) = serde_json::to_string(days) {
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }
}

pub fn get(n: usize) -> Vec<Day> {
    let mut g = DAYS.lock().unwrap_or_else(|e| e.into_inner());
    summary(g.get_or_insert_with(load), n)
}

/// Start sampling (once; later calls do nothing).
pub fn watch() {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    #[cfg(windows)]
    std::thread::spawn(|| {
        let mut unsaved = 0u64;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(STEP_SECS));
            let Some(exe) = win::active() else { continue };
            if ignored(&exe) {
                continue;
            }
            let mut g = DAYS.lock().unwrap_or_else(|e| e.into_inner());
            let days = g.get_or_insert_with(load);
            add(days, &win::today(), &exe, STEP_SECS);
            unsaved += STEP_SECS;
            if unsaved >= 60 {
                save(days);
                unsaved = 0;
            }
        }
    });
}

#[cfg(windows)]
mod win {
    use windows::Win32::System::SystemInformation::{GetLocalTime, GetTickCount64};
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    pub fn today() -> String {
        let t = unsafe { GetLocalTime() };
        format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay)
    }

    /// The program in front, if the user has been active in the last few minutes.
    pub fn active() -> Option<String> {
        let mut li = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if !unsafe { GetLastInputInfo(&mut li) }.as_bool() {
            return None;
        }
        // dwTime is the low 32 bits of the tick count.
        let idle_ms = (unsafe { GetTickCount64() } as u32).wrapping_sub(li.dwTime) as u64;
        if idle_ms > super::IDLE_SECS * 1000 {
            return None;
        }
        let h = unsafe { GetForegroundWindow() };
        if h.is_invalid() {
            return None;
        }
        crate::apps::win::real_exe(h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_and_keeps_two_weeks() {
        let mut d = Days::new();
        for i in 1..=20 {
            add(&mut d, &format!("2026-11-{i:02}"), "C:\\a\\firefox.exe", 5);
        }
        add(&mut d, "2026-11-20", "C:\\a\\firefox.exe", 5);
        assert_eq!(d.len(), KEEP_DAYS);
        assert!(d.contains_key("2026-11-07") && !d.contains_key("2026-11-06"));
        assert_eq!(d["2026-11-20"]["C:\\a\\firefox.exe"], 10);
    }

    #[test]
    fn summary_is_newest_first_and_sorted() {
        let mut d = Days::new();
        add(&mut d, "2026-11-15", "C:\\x\\Code.exe", 60);
        add(&mut d, "2026-11-16", "C:\\x\\Code.exe", 30);
        add(&mut d, "2026-11-16", "C:\\y\\firefox.exe", 90);
        let s = summary(&d, 7);
        assert_eq!(s[0].date, "2026-11-16");
        assert_eq!(s[0].total, 120);
        assert_eq!(s[0].apps[0].name, "firefox");
        assert_eq!(s[1].apps[0].name, "Code");
        assert_eq!(summary(&d, 1).len(), 1);
    }

    #[test]
    fn names_and_ignores() {
        assert_eq!(
            name_of("C:\\Program Files\\WindowsApps\\WindowsTerminal.exe"),
            "WindowsTerminal"
        );
        assert!(ignored("C:\\Windows\\SystemApps\\LockApp.exe"));
        assert!(!ignored("C:\\x\\firefox.exe"));
    }
}
