//! GPU load for the GPU cell and the System flyout, counted the way Task Manager does it: the
//! "GPU Engine" performance counters give every engine's utilisation per process; per adapter, the
//! engine type (3D, Copy, Video Decode, Compute...) with the highest total is the adapter's load.
//! Dedicated memory in use comes from "GPU Adapter Memory"; names and VRAM size from DXGI.
//! Only sampled while something asks (see stats::get), so an unused GPU cell costs nothing.

use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Gpu {
    pub name: String,
    /// 0-100, the busiest engine type.
    pub load: f64,
    /// Which engine type that is ("3D", "Compute", "VideoDecode"...).
    pub engine: String,
    /// Dedicated (VRAM) bytes in use and in total; total is 0 for integrated GPUs.
    pub mem_used: u64,
    pub mem_total: u64,
}

/// The adapter part of a counter instance, lower-cased: "luid_0x00000000_0x0000d1a3".
pub fn luid_of(instance: &str) -> Option<String> {
    let lower = instance.to_ascii_lowercase();
    let start = lower.find("luid_")?;
    let rest = &lower[start..];
    // luid_0xHHHHHHHH_0xLLLLLLLL
    let parts: Vec<&str> = rest.splitn(4, '_').collect();
    if parts.len() < 3 || !parts[1].starts_with("0x") || !parts[2].starts_with("0x") {
        return None;
    }
    Some(format!("luid_{}_{}", parts[1], parts[2]))
}

/// "pid_1234_luid_..._phys_0_eng_0_engtype_3D" -> "3D".
pub fn engine_of(instance: &str) -> Option<String> {
    let i = instance.find("engtype_")?;
    let e = &instance[i + "engtype_".len()..];
    (!e.is_empty()).then(|| e.to_string())
}

/// Per adapter: (load, engine) where load is the largest per-engine-type sum, capped at 100.
pub fn loads(rows: &[(String, f64)]) -> HashMap<String, (f64, String)> {
    let mut sums: HashMap<(String, String), f64> = HashMap::new();
    for (inst, v) in rows {
        if let (Some(l), Some(e)) = (luid_of(inst), engine_of(inst)) {
            *sums.entry((l, e)).or_default() += v.max(0.0);
        }
    }
    let mut out: HashMap<String, (f64, String)> = HashMap::new();
    for ((l, e), v) in sums {
        let cur = out.entry(l).or_insert((0.0, String::new()));
        if v > cur.0 || cur.1.is_empty() {
            *cur = (v.min(100.0), e);
        }
    }
    out
}

#[cfg(windows)]
pub use win::sample;

#[cfg(not(windows))]
pub fn sample() -> Vec<Gpu> {
    vec![]
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1,
    };
    use windows::Win32::System::Performance::{
        PDH_FMT, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_FMT_LARGE, PDH_HCOUNTER,
        PDH_HQUERY, PDH_MORE_DATA, PdhAddEnglishCounterW, PdhCollectQueryData,
        PdhGetFormattedCounterArrayW, PdhOpenQueryW,
    };
    use windows::core::PCWSTR;

    struct Adapter {
        luid: String,
        name: String,
        vram: u64,
    }

    /// Hardware adapters, read once (they don't change while the dock runs).
    fn adapters() -> &'static Vec<Adapter> {
        static A: OnceLock<Vec<Adapter>> = OnceLock::new();
        A.get_or_init(|| unsafe {
            let mut out = vec![];
            let Ok(f) = CreateDXGIFactory1::<IDXGIFactory1>() else {
                return out;
            };
            let mut i = 0;
            while let Ok(a) = f.EnumAdapters1(i) {
                i += 1;
                let Ok(d) = a.GetDesc1() else { continue };
                if d.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
                    continue;
                }
                let name = String::from_utf16_lossy(
                    &d.Description[..d
                        .Description
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(d.Description.len())],
                );
                let luid = format!(
                    "luid_0x{:08x}_0x{:08x}",
                    d.AdapterLuid.HighPart as u32, d.AdapterLuid.LowPart
                );
                if out.iter().any(|x: &Adapter| x.luid == luid) {
                    continue;
                }
                // Integrated GPUs report a small carve-out (128 MB here) and really use shared RAM, so
                // their "VRAM" isn't shown.
                let vram = d.DedicatedVideoMemory as u64;
                let vram = if vram < 512 << 20 { 0 } else { vram };
                out.push(Adapter {
                    luid,
                    name: name.trim().to_string(),
                    vram,
                });
            }
            out
        })
    }

    struct Query {
        q: PDH_HQUERY,
        engine: PDH_HCOUNTER,
        memory: PDH_HCOUNTER,
    }
    // PDH handles are plain handles, used only under the mutex below.
    unsafe impl Send for Query {}

    static QUERY: Mutex<Option<Query>> = Mutex::new(None);

    fn open() -> Option<Query> {
        unsafe {
            let mut q = PDH_HQUERY::default();
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut q) != 0 {
                return None;
            }
            let mut engine = PDH_HCOUNTER::default();
            let mut memory = PDH_HCOUNTER::default();
            let e = windows::core::w!("\\GPU Engine(*)\\Utilization Percentage");
            let m = windows::core::w!("\\GPU Adapter Memory(*)\\Dedicated Usage");
            if PdhAddEnglishCounterW(q, e, 0, &mut engine) != 0
                || PdhAddEnglishCounterW(q, m, 0, &mut memory) != 0
            {
                // No GPU counters here (a VM, remote desktop, some drivers): close the query.
                windows::Win32::System::Performance::PdhCloseQuery(q);
                return None;
            }
            // Utilisation is a rate: the first collection only sets the baseline.
            PdhCollectQueryData(q);
            Some(Query { q, engine, memory })
        }
    }

    /// Every instance of a wildcard counter as (instance name, value).
    fn rows(c: PDH_HCOUNTER, fmt: PDH_FMT, large: bool) -> Vec<(String, f64)> {
        unsafe {
            let mut size = 0u32;
            let mut count = 0u32;
            if PdhGetFormattedCounterArrayW(c, fmt, &mut size, &mut count, None) != PDH_MORE_DATA
                || size == 0
            {
                return vec![];
            }
            // One allocation: the items, then the instance names they point into.
            let mut buf = vec![0u64; (size as usize).div_ceil(8) + 1];
            let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            if PdhGetFormattedCounterArrayW(c, fmt, &mut size, &mut count, Some(items)) != 0 {
                return vec![];
            }
            let mut out = Vec::with_capacity(count as usize);
            for i in 0..count as usize {
                let it = &*items.add(i);
                // 0 = valid data, 1 = new data.
                if it.FmtValue.CStatus > 1 {
                    continue;
                }
                let v = if large {
                    it.FmtValue.Anonymous.largeValue as f64
                } else {
                    it.FmtValue.Anonymous.doubleValue
                };
                out.push((it.szName.to_string().unwrap_or_default(), v));
            }
            out
        }
    }

    /// Calls within 0.9 s share one reading (the cell and the flyout poll together).
    static LAST: Mutex<Option<(std::time::Instant, Vec<Gpu>)>> = Mutex::new(None);

    pub fn sample() -> Vec<Gpu> {
        if let Some((at, v)) = LAST.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if at.elapsed() < std::time::Duration::from_millis(900) {
                return v.clone();
            }
        }
        let v = read();
        *LAST.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((std::time::Instant::now(), v.clone()));
        v
    }

    fn read() -> Vec<Gpu> {
        let mut g = QUERY.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() {
            // Without GPU counters, try again only every five minutes, not on every sample.
            static TRIED: Mutex<Option<std::time::Instant>> = Mutex::new(None);
            let mut t = TRIED.lock().unwrap_or_else(|e| e.into_inner());
            if t.is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(300)) {
                return vec![];
            }
            *t = Some(std::time::Instant::now());
            *g = open();
        }
        let Some(q) = g.as_ref() else { return vec![] };
        unsafe {
            if PdhCollectQueryData(q.q) != 0 {
                return vec![];
            }
        }
        let load = loads(&rows(q.engine, PDH_FMT_DOUBLE, false));
        let mut mem: HashMap<String, u64> = HashMap::new();
        for (inst, v) in rows(q.memory, PDH_FMT_LARGE, true) {
            if let Some(l) = luid_of(&inst) {
                *mem.entry(l).or_default() += v.max(0.0) as u64;
            }
        }
        let mut out: Vec<Gpu> = adapters()
            .iter()
            .map(|a| {
                let (l, e) = load.get(&a.luid).cloned().unwrap_or_default();
                Gpu {
                    name: a.name.clone(),
                    load: l,
                    engine: e,
                    mem_used: mem.get(&a.luid).copied().unwrap_or(0),
                    mem_total: a.vram,
                }
            })
            .collect();
        // The card with the most VRAM (the discrete one on a laptop) first.
        out.sort_by(|a, b| b.mem_total.cmp(&a.mem_total));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_instances() {
        let i = "pid_1234_luid_0x00000000_0x0000D1A3_phys_0_eng_0_engtype_3D";
        assert_eq!(luid_of(i).as_deref(), Some("luid_0x00000000_0x0000d1a3"));
        assert_eq!(engine_of(i).as_deref(), Some("3D"));
        assert_eq!(
            luid_of("luid_0x00000000_0x0000D1A3_phys_0").as_deref(),
            Some("luid_0x00000000_0x0000d1a3")
        );
        assert_eq!(luid_of("_Total"), None);
        assert_eq!(engine_of("pid_1_luid_0x0_0x1_phys_0_eng_0"), None);
    }

    #[test]
    fn busiest_engine_type_wins() {
        let a = "luid_0x00000000_0x0000aaaa";
        let rows = vec![
            (format!("pid_1_{a}_phys_0_eng_0_engtype_3D"), 20.0),
            (format!("pid_2_{a}_phys_0_eng_0_engtype_3D"), 15.0),
            (format!("pid_3_{a}_phys_0_eng_5_engtype_Compute"), 30.0),
            (format!("pid_3_{a}_phys_0_eng_6_engtype_Copy"), 90.0),
            (
                format!("pid_4_luid_0x00000000_0x0000bbbb_phys_0_eng_0_engtype_3D"),
                60.0,
            ),
            (
                format!("pid_4_luid_0x00000000_0x0000bbbb_phys_0_eng_0_engtype_3D"),
                70.0,
            ),
        ];
        let l = loads(&rows);
        assert_eq!(l[a], (90.0, "Copy".to_string()));
        // Sums past 100 (rounding across processes) are capped.
        assert_eq!(l["luid_0x00000000_0x0000bbbb"], (100.0, "3D".to_string()));
    }
}
