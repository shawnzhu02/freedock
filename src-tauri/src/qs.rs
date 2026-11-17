//! The rest of Windows' Quick Settings, for our own flyout: Airplane mode, Mobile hotspot, Night
//! light, Project (display mode), Energy saver, Live captions and the accessibility tools
//! (Magnifier, Narrator, on-screen keyboard). All local.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, Default)]
pub struct Extra {
    /// None when the setting doesn't exist on this PC.
    pub airplane: Option<bool>,
    pub hotspot: Option<Hotspot>,
    pub night_light: Option<bool>,
    /// "pc", "duplicate", "extend" or "second"; None when Windows won't say.
    pub project: Option<String>,
    /// Energy saver (battery saver) is on right now.
    pub energy_saver: Option<bool>,
    pub captions: bool,
    pub magnifier: bool,
    pub narrator: bool,
    pub keyboard: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Hotspot {
    pub on: bool,
    pub clients: u32,
    /// The network name others see.
    pub name: String,
}

// ---- Night light's settings blob (pure, tested)
//
// Windows keeps Night light's on/off state in a CloudStore registry blob:
//   43 42 01 00 0a 02 01 00 2a 06 <timestamp varint> 2a 2b 0e <len> 43 42 01 00 [10 00] ...
// The "10 00" pair is present while it's on, and <len> counts it. Windows only applies a change
// when the timestamp moves forward.

/// Protobuf-style varint at `at`: (value, bytes used).
pub fn varint(b: &[u8], at: usize) -> Option<(u64, usize)> {
    let mut v = 0u64;
    for i in 0..10 {
        let byte = *b.get(at + i)?;
        v |= ((byte & 0x7f) as u64) << (7 * i);
        if byte & 0x80 == 0 {
            return Some((v, i + 1));
        }
    }
    None
}

pub fn put_varint(mut v: u64) -> Vec<u8> {
    let mut out = vec![];
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return out;
        }
        out.push(b | 0x80);
    }
}

fn find(b: &[u8], pat: &[u8]) -> Option<usize> {
    b.windows(pat.len()).position(|w| w == pat)
}

/// Where the state record starts (index of its length byte).
fn state_at(b: &[u8]) -> Option<usize> {
    let i = find(b, &[0x2a, 0x2b, 0x0e])? + 3;
    // The record must be "<len> 43 42 01 00".
    (b.get(i + 1..i + 5)? == [0x43, 0x42, 0x01, 0x00]).then_some(i)
}

pub fn night_light_on(b: &[u8]) -> Option<bool> {
    let i = state_at(b)?;
    Some(b.get(i + 5..i + 7) == Some(&[0x10, 0x00][..]))
}

/// The blob with Night light switched `on`, stamped `now` (unix seconds) or just after the old
/// stamp, whichever is later. None if the blob doesn't look like the format above.
pub fn night_light_set(b: &[u8], on: bool, now: u64) -> Option<Vec<u8>> {
    let was = night_light_on(b)?;
    let mut out = b.to_vec();
    let i = state_at(&out)?;
    if on != was {
        if on {
            out.splice(i + 5..i + 5, [0x10, 0x00]);
            out[i] = out[i].checked_add(2)?;
        } else {
            out.drain(i + 5..i + 7);
            out[i] = out[i].checked_sub(2)?;
        }
    }
    // The timestamp follows the first "2a 06".
    let t = find(&out, &[0x2a, 0x06])? + 2;
    let (old, used) = varint(&out, t)?;
    let stamp = put_varint(now.max(old + 1));
    out.splice(t..t + used, stamp);
    Some(out)
}

/// The display topology names used by the flyout, from Windows' DISPLAYCONFIG_TOPOLOGY_ID.
pub fn topology_name(id: i32) -> Option<&'static str> {
    match id {
        1 => Some("pc"),
        2 => Some("duplicate"),
        4 => Some("extend"),
        8 => Some("second"),
        _ => None,
    }
}

#[cfg(windows)]
pub use win::{get, set_airplane, set_hotspot, set_night_light, set_project, toggle_tool};

#[cfg(not(windows))]
pub fn get() -> Extra {
    Extra::default()
}

#[cfg(windows)]
#[allow(non_snake_case)] // COM interfaces declared below keep Windows' method names
mod win {
    use super::*;
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WPARAM};
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
    };
    use windows::core::{GUID, HRESULT, PCWSTR};

    fn com() {
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    }

    // ---- Airplane mode (the Radio Management API Windows' own toggle uses; not in the SDK)

    #[windows::core::interface("db3afbfb-08e6-46c6-aa70-bf9a34c30ab7")]
    unsafe trait IRadioManager: windows::core::IUnknown {
        fn IsRMSupported(&self, supported: *mut u32) -> HRESULT;
        fn GetUIRadioInstances(&self, list: *mut *mut core::ffi::c_void) -> HRESULT;
        fn GetSystemRadioState(
            &self,
            enabled: *mut i32,
            policy: *mut i32,
            reason: *mut i32,
        ) -> HRESULT;
        fn SetSystemRadioState(&self, enabled: i32) -> HRESULT;
        fn Refresh(&self) -> HRESULT;
        fn OnHardwareSliderChange(&self, a: i32, b: i32) -> HRESULT;
    }
    const CLSID_RADIO_MANAGER: GUID = GUID::from_u128(0x581333f6_28db_41be_bc7a_ff201f12f3f6);

    fn radio_manager() -> Option<IRadioManager> {
        com();
        unsafe { CoCreateInstance(&CLSID_RADIO_MANAGER, None, CLSCTX_ALL) }.ok()
    }

    fn airplane() -> Option<bool> {
        let m = radio_manager()?;
        let (mut on, mut policy, mut reason) = (0i32, 0i32, 0i32);
        unsafe { m.GetSystemRadioState(&mut on, &mut policy, &mut reason) }
            .ok()
            .ok()?;
        // "Radios enabled" is the opposite of airplane mode.
        Some(on == 0)
    }

    pub fn set_airplane(on: bool) -> Result<(), String> {
        let m = radio_manager().ok_or("Airplane mode isn't available")?;
        unsafe { m.SetSystemRadioState(if on { 0 } else { 1 }) }
            .ok()
            .map_err(|e| e.to_string())
    }

    // ---- Mobile hotspot (WinRT tethering, shares the current internet connection)

    fn tethering() -> Option<windows::Networking::NetworkOperators::NetworkOperatorTetheringManager>
    {
        use windows::Networking::Connectivity::NetworkInformation;
        use windows::Networking::NetworkOperators::{
            NetworkOperatorTetheringManager, TetheringCapability,
        };
        let p = NetworkInformation::GetInternetConnectionProfile().ok()?;
        if NetworkOperatorTetheringManager::GetTetheringCapabilityFromConnectionProfile(&p).ok()?
            != TetheringCapability::Enabled
        {
            return None;
        }
        NetworkOperatorTetheringManager::CreateFromConnectionProfile(&p).ok()
    }

    fn hotspot() -> Option<Hotspot> {
        use windows::Networking::NetworkOperators::TetheringOperationalState;
        let t = tethering()?;
        let name = t
            .GetCurrentAccessPointConfiguration()
            .and_then(|c| c.Ssid())
            .map(|s| s.to_string())
            .unwrap_or_default();
        Some(Hotspot {
            on: t.TetheringOperationalState().ok()? == TetheringOperationalState::On,
            clients: t.ClientCount().unwrap_or(0),
            name,
        })
    }

    pub fn set_hotspot(on: bool) -> Result<(), String> {
        use windows::Networking::NetworkOperators::TetheringOperationStatus;
        let t = tethering().ok_or("Mobile hotspot needs an internet connection it can share")?;
        let op = if on {
            t.StartTetheringAsync()
        } else {
            t.StopTetheringAsync()
        }
        .map_err(|e| e.to_string())?;
        let r = op.join().map_err(|e| e.to_string())?;
        let s = r.Status().map_err(|e| e.to_string())?;
        if s == TetheringOperationStatus::Success {
            Ok(())
        } else {
            let msg = r
                .AdditionalErrorMessage()
                .map(|m| m.to_string())
                .unwrap_or_default();
            Err(if msg.is_empty() {
                format!("Windows couldn't switch the hotspot ({})", s.0)
            } else {
                msg
            })
        }
    }

    // ---- Night light

    const NL_KEY: PCWSTR = windows::core::w!(
        r"Software\Microsoft\Windows\CurrentVersion\CloudStore\Store\DefaultAccount\Current\default$windows.data.bluelightreduction.bluelightreductionstate\windows.data.bluelightreduction.bluelightreductionstate"
    );

    fn nl_read() -> Option<Vec<u8>> {
        use windows::Win32::System::Registry::{
            HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RegGetValueW,
        };
        let mut n = 0u32;
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                NL_KEY,
                windows::core::w!("Data"),
                RRF_RT_REG_BINARY,
                None,
                None,
                Some(&mut n),
            )
            .ok()
            .ok()?;
            let mut b = vec![0u8; n as usize];
            RegGetValueW(
                HKEY_CURRENT_USER,
                NL_KEY,
                windows::core::w!("Data"),
                RRF_RT_REG_BINARY,
                None,
                Some(b.as_mut_ptr() as _),
                Some(&mut n),
            )
            .ok()
            .ok()?;
            b.truncate(n as usize);
            Some(b)
        }
    }

    pub fn set_night_light(on: bool) -> Result<(), String> {
        use windows::Win32::System::Registry::{HKEY_CURRENT_USER, REG_BINARY, RegSetKeyValueW};
        let old = nl_read()
            .ok_or("Night light hasn't been set up on this PC yet (turn it on once in Settings)")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let new = night_light_set(&old, on, now)
            .ok_or("Night light's settings are in a format the dock doesn't know")?;
        let r = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                NL_KEY,
                windows::core::w!("Data"),
                REG_BINARY.0,
                Some(new.as_ptr() as _),
                new.len() as u32,
            )
        };
        if r.is_err() {
            return Err(format!("couldn't change Night light ({})", r.0));
        }
        Ok(())
    }

    // ---- Project (display topology)

    fn project() -> Option<String> {
        use windows::Win32::Devices::Display::{
            DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_TOPOLOGY_ID,
            GetDisplayConfigBufferSizes, QDC_DATABASE_CURRENT, QueryDisplayConfig,
        };
        unsafe {
            let (mut np, mut nm) = (0u32, 0u32);
            if GetDisplayConfigBufferSizes(QDC_DATABASE_CURRENT, &mut np, &mut nm).is_err() {
                return None;
            }
            let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
            let mut topo = DISPLAYCONFIG_TOPOLOGY_ID::default();
            if QueryDisplayConfig(
                QDC_DATABASE_CURRENT,
                &mut np,
                paths.as_mut_ptr(),
                &mut nm,
                modes.as_mut_ptr(),
                Some(&mut topo),
            )
            .is_err()
            {
                return None;
            }
            topology_name(topo.0).map(str::to_string)
        }
    }

    pub fn set_project(mode: &str) -> Result<(), String> {
        use windows::Win32::Devices::Display::{
            SDC_APPLY, SDC_TOPOLOGY_CLONE, SDC_TOPOLOGY_EXTEND, SDC_TOPOLOGY_EXTERNAL,
            SDC_TOPOLOGY_INTERNAL, SetDisplayConfig,
        };
        let t = match mode {
            "pc" => SDC_TOPOLOGY_INTERNAL,
            "duplicate" => SDC_TOPOLOGY_CLONE,
            "extend" => SDC_TOPOLOGY_EXTEND,
            "second" => SDC_TOPOLOGY_EXTERNAL,
            _ => return Err("unknown mode".into()),
        };
        let r = unsafe { SetDisplayConfig(None, None, SDC_APPLY | t) };
        if r == 0 {
            Ok(())
        } else {
            Err(format!(
                "Windows couldn't switch displays ({r}); is a second screen connected?"
            ))
        }
    }

    // ---- Energy saver

    fn energy_saver() -> Option<bool> {
        use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        let mut s = SYSTEM_POWER_STATUS::default();
        unsafe { GetSystemPowerStatus(&mut s) }.ok()?;
        Some(s.SystemStatusFlag & 1 == 1)
    }

    // ---- Live captions and the accessibility tools (each is its own program in System32)

    /// (key, exe) for the tools the flyout can start and stop.
    const TOOLS: [(&str, &str); 4] = [
        ("captions", "LiveCaptions.exe"),
        ("magnifier", "Magnify.exe"),
        ("narrator", "Narrator.exe"),
        ("keyboard", "osk.exe"),
    ];

    fn running() -> Vec<(String, u32)> {
        let mut out = vec![];
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
                let n = String::from_utf16_lossy(
                    &e.szExeFile[..e.szExeFile.iter().position(|&c| c == 0).unwrap_or(0)],
                );
                if TOOLS.iter().any(|(_, x)| x.eq_ignore_ascii_case(&n)) {
                    out.push((n.to_lowercase(), e.th32ProcessID));
                }
                ok = Process32NextW(snap, &mut e).is_ok();
            }
            let _ = CloseHandle(snap);
        }
        out
    }

    fn close_windows_of(pids: &[u32]) {
        struct Ctx<'a>(&'a [u32]);
        unsafe extern "system" fn each(h: HWND, l: LPARAM) -> windows::core::BOOL {
            unsafe {
                let ctx = &*(l.0 as *const Ctx);
                let mut pid = 0u32;
                GetWindowThreadProcessId(h, Some(&mut pid));
                if ctx.0.contains(&pid) && IsWindowVisible(h).as_bool() {
                    let _ = PostMessageW(Some(h), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
            true.into()
        }
        let ctx = Ctx(pids);
        unsafe {
            let _ = EnumWindows(Some(each), LPARAM(&ctx as *const Ctx as isize));
        }
    }

    /// Start the tool, or close it if it's running. Programs are started from System32 by full path.
    pub fn toggle_tool(key: &str) -> Result<(), String> {
        let (_, exe) = TOOLS
            .iter()
            .find(|(k, _)| *k == key)
            .ok_or("unknown tool")?;
        let pids: Vec<u32> = running()
            .into_iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(exe))
            .map(|(_, p)| p)
            .collect();
        if !pids.is_empty() {
            close_windows_of(&pids);
            return Ok(());
        }
        let dir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        let path = format!(r"{dir}\System32\{exe}");
        if !std::path::Path::new(&path).exists() {
            return Err(format!("{exe} isn't on this PC"));
        }
        // ShellExecute, not CreateProcess: Magnifier and the keyboard need UI access, which only
        // the shell's launch grants.
        let w: Vec<u16> = path.encode_utf16().chain([0]).collect();
        let r = unsafe {
            windows::Win32::UI::Shell::ShellExecuteW(
                None,
                windows::core::w!("open"),
                PCWSTR(w.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
            )
        };
        if r.0 as isize > 32 {
            Ok(())
        } else {
            Err(format!("couldn't start {exe}"))
        }
    }

    pub fn get() -> Extra {
        let run = running();
        let is = |exe: &str| run.iter().any(|(n, _)| n.eq_ignore_ascii_case(exe));
        Extra {
            airplane: airplane(),
            hotspot: hotspot(),
            night_light: nl_read().and_then(|b| night_light_on(&b)),
            project: project(),
            energy_saver: energy_saver(),
            captions: is("LiveCaptions.exe"),
            magnifier: is("Magnify.exe"),
            narrator: is("Narrator.exe"),
            keyboard: is("osk.exe"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real PC's blob with Night light off.
    const OFF: [u8; 27] = [
        0x43, 0x42, 0x01, 0x00, 0x0a, 0x02, 0x01, 0x00, 0x2a, 0x06, 0xe3, 0xe6, 0xe4, 0xd6, 0x06,
        0x2a, 0x2b, 0x0e, 0x05, 0x43, 0x42, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn varints() {
        assert_eq!(varint(&OFF, 10), Some((1792619363, 5)));
        assert_eq!(put_varint(1792619363), vec![0xe3, 0xe6, 0xe4, 0xd6, 0x06]);
        assert_eq!(put_varint(1), vec![1]);
        assert_eq!(varint(&put_varint(300), 0), Some((300, 2)));
    }

    #[test]
    fn night_light_round_trip() {
        assert_eq!(night_light_on(&OFF), Some(false));
        let on = night_light_set(&OFF, true, 1800000000).unwrap();
        assert_eq!(night_light_on(&on), Some(true));
        assert_eq!(on.len(), OFF.len() + 2);
        assert_eq!(on[18], 0x07);
        assert_eq!(&on[19..25], &[0x43, 0x42, 0x01, 0x00, 0x10, 0x00]);
        assert_eq!(varint(&on, 10), Some((1800000000, 5)));
        let off = night_light_set(&on, false, 1800000005).unwrap();
        assert_eq!(night_light_on(&off), Some(false));
        // Back to the original bytes apart from the newer timestamp.
        assert_eq!(&off[..10], &OFF[..10]);
        assert_eq!(&off[15..], &OFF[15..]);
        // The stamp always moves forward, even with a clock behind the old one.
        let again = night_light_set(&off, false, 5).unwrap();
        assert_eq!(varint(&again, 10).unwrap().0, 1800000006);
        // The longer form Windows uses once a schedule exists ("0e 13" off, "0e 15" on).
        let sched_off = [
            0x2a, 0x06, 0x01, 0x2a, 0x2b, 0x0e, 0x13, 0x43, 0x42, 0x01, 0x00, 0xd0, 0x0a,
        ];
        assert_eq!(night_light_on(&sched_off), Some(false));
        let s = night_light_set(&sched_off, true, 2).unwrap();
        assert_eq!(&s[6..13], &[0x15, 0x43, 0x42, 0x01, 0x00, 0x10, 0x00]);
        assert_eq!(night_light_on(b"garbage"), None);
    }

    #[test]
    fn topologies() {
        assert_eq!(topology_name(4), Some("extend"));
        assert_eq!(topology_name(3), None);
    }
}

#[cfg(all(test, windows))]
mod probe {
    /// Reads (never changes) the live state: `cargo test probe_quick -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn probe_quick() {
        println!("{:?}", super::get());
        println!(
            "{:?}",
            crate::mixer::devices()
                .iter()
                .map(|d| (&d.name, d.default))
                .collect::<Vec<_>>()
        );
        println!("{:?}", crate::mixer::apps());
    }
}
