//! System stats for the CPU / memory / network / storage / top-apps cells. Sampled on demand:
//! each call diffs against the previous sample, so nothing runs while no cell is asking. Calls
//! within a second share one sample (several cells poll at once).

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Debug, Default)]
pub struct Disk {
    pub root: String,
    pub total: u64,
    pub free: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct App {
    pub name: String,
    /// Share of the whole machine, 0-100.
    pub cpu: f64,
    /// Working set, bytes.
    pub mem: u64,
    pub count: u32,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Stats {
    /// 0-100.
    pub cpu: f64,
    pub cores: u32,
    pub mem_used: u64,
    pub mem_total: u64,
    /// Bytes per second across physical adapters.
    pub down: f64,
    pub up: f64,
    pub disks: Vec<Disk>,
    /// Busiest apps first; only filled when asked for.
    pub apps: Vec<App>,
    /// Seconds since Windows started (Fast Startup carries it across shutdowns).
    pub uptime: u64,
}

/// Raw counters at one moment.
#[derive(Clone, Default)]
struct Sample {
    at: Option<Instant>,
    idle: u64,
    total: u64,
    rx: u64,
    tx: u64,
    /// pid -> (name, kernel+user 100 ns ticks)
    procs: HashMap<u32, (String, u64)>,
}

struct State {
    prev: Sample,
    last: Option<(Instant, bool, Stats)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Percent busy between two readings of (idle, kernel+user) ticks.
/// How the PC is online from the interface types of its connected physical adapters:
/// "ethernet" (wired wins when both are up, as Windows prefers it), "wifi", or "none".
pub fn link_kind(up_types: &[u32]) -> &'static str {
    const ETHERNET: u32 = 6;
    const WIFI: u32 = 71;
    if up_types.contains(&ETHERNET) {
        "ethernet"
    } else if up_types.contains(&WIFI) {
        "wifi"
    } else {
        "none"
    }
}

pub fn link() -> &'static str {
    link_kind(&win::up_types())
}

pub fn busy(idle0: u64, total0: u64, idle1: u64, total1: u64) -> f64 {
    let dt = total1.saturating_sub(total0);
    if dt == 0 {
        return 0.0;
    }
    let di = idle1.saturating_sub(idle0).min(dt);
    ((dt - di) as f64 / dt as f64 * 100.0).clamp(0.0, 100.0)
}

/// Bytes per second from two counter readings `secs` apart (0 when a counter went backwards, e.g. an
/// adapter reset).
pub fn rate(a: u64, b: u64, secs: f64) -> f64 {
    if secs <= 0.0 || b < a {
        0.0
    } else {
        (b - a) as f64 / secs
    }
}

/// Group per-process CPU deltas by app name: (name, ticks, mem) -> busiest-first apps.
pub fn group_apps(
    rows: Vec<(String, u64, u64)>,
    wall_ticks: u64,
    cores: u32,
    top: usize,
) -> Vec<App> {
    let mut by: HashMap<String, App> = HashMap::new();
    let denom = (wall_ticks.max(1) * cores.max(1) as u64) as f64;
    for (name, ticks, mem) in rows {
        let key = name.strip_suffix(".exe").unwrap_or(&name).to_string();
        let a = by.entry(key.clone()).or_insert_with(|| App {
            name: key,
            ..Default::default()
        });
        a.cpu += ticks as f64 / denom * 100.0;
        a.mem += mem;
        a.count += 1;
    }
    let mut v: Vec<App> = by
        .into_values()
        .filter(|a| a.name != "System Idle Process" && a.name != "Idle")
        .collect();
    v.sort_by(|a, b| {
        b.cpu
            .partial_cmp(&a.cpu)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.mem.cmp(&a.mem))
    });
    v.truncate(top);
    for a in &mut v {
        a.cpu = (a.cpu * 10.0).round() / 10.0;
    }
    v
}

fn uptime_secs() -> u64 {
    #[cfg(windows)]
    {
        unsafe { windows::Win32::System::SystemInformation::GetTickCount64() / 1000 }
    }
    #[cfg(not(windows))]
    {
        0
    }
}

pub fn get(apps: bool) -> Stats {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let st = g.get_or_insert_with(|| State {
        prev: Sample::default(),
        last: None,
    });
    if let Some((at, had_apps, s)) = &st.last {
        if at.elapsed() < Duration::from_millis(900) && (*had_apps || !apps) {
            return s.clone();
        }
    }
    // A first or stale reading has nothing fresh to diff against: take a short baseline.
    let stale = st
        .prev
        .at
        .map_or(true, |t| t.elapsed() > Duration::from_secs(10));
    if stale || (apps && st.prev.procs.is_empty()) {
        st.prev = win::sample(apps);
        drop(g);
        std::thread::sleep(Duration::from_millis(400));
        g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    }
    let st = g.as_mut().expect("state");
    let now = win::sample(apps);
    let prev = std::mem::replace(&mut st.prev, now.clone());
    let secs = match (prev.at, now.at) {
        (Some(a), Some(b)) => (b - a).as_secs_f64(),
        _ => 0.0,
    };
    let (mem_used, mem_total) = win::memory();
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1);
    let mut s = Stats {
        cpu: (busy(prev.idle, prev.total, now.idle, now.total) * 10.0).round() / 10.0,
        cores,
        mem_used,
        mem_total,
        down: rate(prev.rx, now.rx, secs),
        up: rate(prev.tx, now.tx, secs),
        disks: win::disks(),
        apps: vec![],
        uptime: uptime_secs(),
    };
    if apps {
        let wall = (secs * 10_000_000.0) as u64;
        let mems = win::process_memory(now.procs.keys().copied());
        let rows = now
            .procs
            .iter()
            .map(|(pid, (name, t))| {
                // A pid reused by another program since the last sample starts from zero.
                let before = prev
                    .procs
                    .get(pid)
                    .filter(|(n, _)| n == name)
                    .map(|p| p.1)
                    .unwrap_or(*t);
                (
                    name.clone(),
                    t.saturating_sub(before),
                    mems.get(pid).copied().unwrap_or(0),
                )
            })
            .collect();
        s.apps = group_apps(rows, wall, cores, 8);
    }
    st.last = Some((Instant::now(), apps, s.clone()));
    s
}

#[cfg(windows)]
mod win {
    use super::{Disk, Sample};
    use std::collections::HashMap;
    use std::time::Instant;
    use windows::Win32::Foundation::{CloseHandle, FILETIME};
    use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
    use windows::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows::Win32::System::Threading::{
        GetProcessTimes, GetSystemTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::core::PCWSTR;

    fn ft(f: FILETIME) -> u64 {
        ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
    }

    pub fn sample(apps: bool) -> Sample {
        let mut s = Sample {
            at: Some(Instant::now()),
            ..Default::default()
        };
        unsafe {
            let (mut i, mut k, mut u) = (
                FILETIME::default(),
                FILETIME::default(),
                FILETIME::default(),
            );
            if GetSystemTimes(Some(&mut i), Some(&mut k), Some(&mut u)).is_ok() {
                // Kernel time includes idle time.
                s.idle = ft(i);
                s.total = ft(k) + ft(u);
            }
            let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
            if GetIfTable2(&mut table).is_ok() && !table.is_null() {
                let n = (*table).NumEntries as usize;
                let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), n);
                for r in rows {
                    let bits = r.InterfaceAndOperStatusFlags._bitfield;
                    // HardwareInterface, and not a filter layered on top of one (counted twice otherwise).
                    if bits & 1 == 1 && bits & 2 == 0 && r.OperStatus.0 == 1 {
                        s.rx += r.InOctets;
                        s.tx += r.OutOctets;
                    }
                }
                FreeMibTable(table as *const _);
            }
            if apps {
                s.procs = processes();
            }
        }
        s
    }

    fn processes() -> HashMap<u32, (String, u64)> {
        let mut out = HashMap::new();
        unsafe {
            let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
                return out;
            };
            let mut e = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut ok = Process32FirstW(snap, &mut e).is_ok();
            while ok {
                let pid = e.th32ProcessID;
                let len = e
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(e.szExeFile.len());
                let name = String::from_utf16_lossy(&e.szExeFile[..len]);
                if pid != 0 {
                    if let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                        let (mut c, mut x, mut k, mut u) = (
                            FILETIME::default(),
                            FILETIME::default(),
                            FILETIME::default(),
                            FILETIME::default(),
                        );
                        if GetProcessTimes(h, &mut c, &mut x, &mut k, &mut u).is_ok() {
                            out.insert(pid, (name, ft(k) + ft(u)));
                        }
                        let _ = CloseHandle(h);
                    }
                }
                ok = Process32NextW(snap, &mut e).is_ok();
            }
            let _ = CloseHandle(snap);
        }
        out
    }

    /// Interface types of physical adapters that are up (not virtual, not filter layers).
    pub fn up_types() -> Vec<u32> {
        let mut out = vec![];
        unsafe {
            let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
            if GetIfTable2(&mut table).is_ok() && !table.is_null() {
                let n = (*table).NumEntries as usize;
                for r in std::slice::from_raw_parts((*table).Table.as_ptr(), n) {
                    let bits = r.InterfaceAndOperStatusFlags._bitfield;
                    // HardwareInterface, not a filter, and connected (MediaConnectState 1).
                    if bits & 1 == 1
                        && bits & 2 == 0
                        && r.OperStatus.0 == 1
                        && r.MediaConnectState.0 == 1
                    {
                        out.push(r.Type);
                    }
                }
                FreeMibTable(table as *const _);
            }
        }
        out
    }

    pub fn process_memory(pids: impl Iterator<Item = u32>) -> HashMap<u32, u64> {
        let mut out = HashMap::new();
        for pid in pids {
            unsafe {
                if let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                    let mut m = PROCESS_MEMORY_COUNTERS {
                        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                        ..Default::default()
                    };
                    if K32GetProcessMemoryInfo(h, &mut m, m.cb).as_bool() {
                        out.insert(pid, m.WorkingSetSize as u64);
                    }
                    let _ = CloseHandle(h);
                }
            }
        }
        out
    }

    pub fn memory() -> (u64, u64) {
        let mut m = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        unsafe {
            if GlobalMemoryStatusEx(&mut m).is_ok() {
                return (m.ullTotalPhys - m.ullAvailPhys, m.ullTotalPhys);
            }
        }
        (0, 0)
    }

    pub fn disks() -> Vec<Disk> {
        let mut out = vec![];
        let mask = unsafe { GetLogicalDrives() };
        for i in 0..26u32 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let root = format!("{}:\\", (b'A' + i as u8) as char);
            let w: Vec<u16> = root.encode_utf16().chain(Some(0)).collect();
            unsafe {
                // DRIVE_FIXED only: skip optical, network and removable drives (they can be slow).
                if GetDriveTypeW(PCWSTR(w.as_ptr())) != 3 {
                    continue;
                }
                let (mut free, mut total) = (0u64, 0u64);
                if GetDiskFreeSpaceExW(PCWSTR(w.as_ptr()), Some(&mut free), Some(&mut total), None)
                    .is_ok()
                    && total > 0
                {
                    out.push(Disk {
                        root: root[..2].to_string(),
                        total,
                        free,
                    });
                }
            }
        }
        out
    }
}

#[cfg(not(windows))]
mod win {
    use super::{Disk, Sample};
    use std::collections::HashMap;
    pub fn sample(_: bool) -> Sample {
        Sample::default()
    }
    pub fn up_types() -> Vec<u32> {
        vec![]
    }
    pub fn process_memory(_: impl Iterator<Item = u32>) -> HashMap<u32, u64> {
        HashMap::new()
    }
    pub fn memory() -> (u64, u64) {
        (0, 0)
    }
    pub fn disks() -> Vec<Disk> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wired_wins_over_wifi() {
        assert_eq!(link_kind(&[71, 6]), "ethernet");
        assert_eq!(link_kind(&[71]), "wifi");
        assert_eq!(link_kind(&[]), "none");
        assert_eq!(link_kind(&[24]), "none", "loopback and others don't count");
    }

    #[test]
    fn busy_percent() {
        assert_eq!(busy(0, 0, 25, 100), 75.0);
        assert_eq!(busy(10, 10, 10, 10), 0.0);
        assert_eq!(busy(0, 0, 200, 100), 0.0); // idle can't exceed the total
    }

    #[test]
    fn rates() {
        assert_eq!(rate(100, 1100, 2.0), 500.0);
        assert_eq!(rate(500, 100, 1.0), 0.0);
        assert_eq!(rate(0, 10, 0.0), 0.0);
    }

    #[test]
    fn groups_apps() {
        let rows = vec![
            ("firefox.exe".into(), 4_000_000, 300),
            ("firefox.exe".into(), 2_000_000, 200),
            ("Code.exe".into(), 1_000_000, 900),
            ("System Idle Process".into(), 90_000_000, 0),
        ];
        // 1 s of wall time on 2 cores = 20,000,000 ticks.
        let a = group_apps(rows, 10_000_000, 2, 5);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].name, "firefox");
        assert_eq!(a[0].cpu, 30.0);
        assert_eq!(a[0].mem, 500);
        assert_eq!(a[0].count, 2);
        assert_eq!(a[1].name, "Code");
    }
}
