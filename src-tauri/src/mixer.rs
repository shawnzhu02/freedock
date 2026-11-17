//! The quick-settings sound page: output devices (pick the default one, the way Windows' own
//! Quick Settings does) and a volume mixer with one slider per app that is playing or has played
//! sound since it started. Everything is local Core Audio.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub default: bool,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct AppVolume {
    /// What the row is called: the program's description ("Firefox"), or "System sounds".
    pub name: String,
    /// The key used to change it (exe name lower-case, or "system").
    pub key: String,
    /// 0-100.
    pub volume: u8,
    pub muted: bool,
    /// Sound is playing right now.
    pub active: bool,
}

/// Sessions of one app (Firefox has several) become one row: the loudest volume, muted only if
/// all are, active if any is. System sounds first, then playing apps, then by name.
pub fn group(rows: Vec<AppVolume>) -> Vec<AppVolume> {
    let mut out: Vec<AppVolume> = vec![];
    for r in rows {
        if let Some(o) = out.iter_mut().find(|o| o.key == r.key) {
            o.volume = o.volume.max(r.volume);
            o.muted &= r.muted;
            o.active |= r.active;
        } else {
            out.push(r);
        }
    }
    out.sort_by(|a, b| {
        (b.key == "system")
            .cmp(&(a.key == "system"))
            .then(b.active.cmp(&a.active))
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

#[cfg(windows)]
pub use win::{apps, devices, set_app, set_default};

#[cfg(not(windows))]
pub fn devices() -> Vec<Device> {
    vec![]
}
#[cfg(not(windows))]
pub fn set_default(_: &str) -> Result<(), String> {
    Err("not supported".into())
}
#[cfg(not(windows))]
pub fn apps() -> Vec<AppVolume> {
    vec![]
}
#[cfg(not(windows))]
pub fn set_app(_: &str, _: Option<u8>, _: Option<bool>) -> Result<(), String> {
    Err("not supported".into())
}

#[cfg(windows)]
#[allow(non_snake_case)] // COM interfaces declared below keep Windows' method names
mod win {
    use super::*;
    use windows::Win32::Foundation::{CloseHandle, PROPERTYKEY, S_OK};
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, STGM_READ,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::core::{GUID, HRESULT, Interface, PCWSTR};

    fn com() {
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    }

    fn enumerator() -> Result<IMMDeviceEnumerator, String> {
        com();
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .map_err(|e| e.to_string())
    }

    fn name_of(dev: &IMMDevice) -> String {
        // PKEY_Device_FriendlyName
        let key = PROPERTYKEY {
            fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
            pid: 14,
        };
        unsafe {
            dev.OpenPropertyStore(STGM_READ)
                .and_then(|s| s.GetValue(&key))
                .map(|v| v.to_string())
                .unwrap_or_default()
        }
    }

    fn id_of(dev: &IMMDevice) -> String {
        unsafe {
            dev.GetId()
                .map(|p| {
                    let s = p.to_string().unwrap_or_default();
                    windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as _));
                    s
                })
                .unwrap_or_default()
        }
    }

    /// Active speakers/headphones, the default one marked.
    pub fn devices() -> Vec<Device> {
        let Ok(e) = enumerator() else { return vec![] };
        unsafe {
            let def = e
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map(|d| id_of(&d))
                .unwrap_or_default();
            let Ok(list) = e.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else {
                return vec![];
            };
            let n = list.GetCount().unwrap_or(0);
            (0..n)
                .filter_map(|i| list.Item(i).ok())
                .map(|d| {
                    let id = id_of(&d);
                    Device {
                        default: id == def,
                        name: name_of(&d),
                        id,
                    }
                })
                .collect()
        }
    }

    /// IPolicyConfig: not in the SDK, but it is how Windows' own sound pickers set the default
    /// device, and its layout has been stable since Windows 7.
    #[windows::core::interface("f8679f50-850a-41cf-9c72-430f290290c8")]
    unsafe trait IPolicyConfig: windows::core::IUnknown {
        fn GetMixFormat(&self, a: PCWSTR, b: *mut *mut core::ffi::c_void) -> HRESULT;
        fn GetDeviceFormat(&self, a: PCWSTR, b: i32, c: *mut *mut core::ffi::c_void) -> HRESULT;
        fn ResetDeviceFormat(&self, a: PCWSTR) -> HRESULT;
        fn SetDeviceFormat(
            &self,
            a: PCWSTR,
            b: *mut core::ffi::c_void,
            c: *mut core::ffi::c_void,
        ) -> HRESULT;
        fn GetProcessingPeriod(&self, a: PCWSTR, b: i32, c: *mut i64, d: *mut i64) -> HRESULT;
        fn SetProcessingPeriod(&self, a: PCWSTR, b: *mut i64) -> HRESULT;
        fn GetShareMode(&self, a: PCWSTR, b: *mut core::ffi::c_void) -> HRESULT;
        fn SetShareMode(&self, a: PCWSTR, b: *mut core::ffi::c_void) -> HRESULT;
        fn GetPropertyValue(
            &self,
            a: PCWSTR,
            b: *const PROPERTYKEY,
            c: *mut core::ffi::c_void,
        ) -> HRESULT;
        fn SetPropertyValue(
            &self,
            a: PCWSTR,
            b: *const PROPERTYKEY,
            c: *mut core::ffi::c_void,
        ) -> HRESULT;
        fn SetDefaultEndpoint(&self, id: PCWSTR, role: ERole) -> HRESULT;
        fn SetEndpointVisibility(&self, a: PCWSTR, b: i32) -> HRESULT;
    }
    const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

    /// Make this output the default for everything (apps, media and calls), as Windows' picker does.
    pub fn set_default(id: &str) -> Result<(), String> {
        if !devices().iter().any(|d| d.id == id) {
            return Err("That device isn't connected".into());
        }
        com();
        let p: IPolicyConfig = unsafe { CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL) }
            .map_err(|e| e.to_string())?;
        let w: Vec<u16> = id.encode_utf16().chain([0]).collect();
        for role in [eConsole, eMultimedia, eCommunications] {
            unsafe { p.SetDefaultEndpoint(PCWSTR(w.as_ptr()), role) }
                .ok()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn exe_of(pid: u32) -> Option<String> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut n = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                h,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut n,
            )
            .is_ok();
            let _ = CloseHandle(h);
            ok.then(|| String::from_utf16_lossy(&buf[..n as usize]))
        }
    }

    /// "C:\...\firefox.exe" -> "Firefox"; "msedgewebview2.exe" -> "Msedgewebview2".
    fn pretty(path: &str) -> String {
        let stem = path.rsplit(['\\', '/']).next().unwrap_or(path);
        let stem = stem
            .strip_suffix(".exe")
            .or_else(|| stem.strip_suffix(".EXE"))
            .unwrap_or(stem);
        let mut c = stem.chars();
        c.next()
            .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
            .unwrap_or_default()
    }

    /// Every session on the default output with its key, for reading or changing.
    fn sessions() -> Vec<(String, String, IAudioSessionControl2)> {
        let Ok(e) = enumerator() else { return vec![] };
        unsafe {
            let Ok(dev) = e.GetDefaultAudioEndpoint(eRender, eConsole) else {
                return vec![];
            };
            let Ok(mgr) = dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
                return vec![];
            };
            let Ok(list) = mgr.GetSessionEnumerator() else {
                return vec![];
            };
            let n = list.GetCount().unwrap_or(0);
            let mut out = vec![];
            for i in 0..n {
                let Ok(c) = list.GetSession(i) else { continue };
                let Ok(c2) = c.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                if c.GetState()
                    .map(|s| s == AudioSessionStateExpired)
                    .unwrap_or(true)
                {
                    continue;
                }
                if c2.IsSystemSoundsSession() == S_OK {
                    out.push(("system".to_string(), "System sounds".to_string(), c2));
                    continue;
                }
                let pid = c2.GetProcessId().unwrap_or(0);
                let Some(path) = exe_of(pid) else { continue };
                let key = path
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(&path)
                    .to_lowercase();
                // The dock itself plays nothing worth a slider.
                if key == "freedock.exe" {
                    continue;
                }
                out.push((key, pretty(&path), c2));
            }
            out
        }
    }

    pub fn apps() -> Vec<AppVolume> {
        let rows = sessions()
            .into_iter()
            .filter_map(|(key, name, c)| unsafe {
                let v = c.cast::<ISimpleAudioVolume>().ok()?;
                Some(AppVolume {
                    name,
                    key,
                    volume: (v.GetMasterVolume().ok()? * 100.0).round() as u8,
                    muted: v.GetMute().ok()?.as_bool(),
                    active: c
                        .GetState()
                        .map(|s| s == AudioSessionStateActive)
                        .unwrap_or(false),
                })
            })
            .collect();
        group(rows)
    }

    /// Set one app's volume and/or mute across all of its sessions.
    pub fn set_app(key: &str, volume: Option<u8>, muted: Option<bool>) -> Result<(), String> {
        let mut found = false;
        for (k, _, c) in sessions() {
            if k != key {
                continue;
            }
            found = true;
            let v = c.cast::<ISimpleAudioVolume>().map_err(|e| e.to_string())?;
            unsafe {
                if let Some(p) = volume {
                    v.SetMasterVolume(p.min(100) as f32 / 100.0, std::ptr::null())
                        .map_err(|e| e.to_string())?;
                }
                if let Some(m) = muted {
                    v.SetMute(m, std::ptr::null()).map_err(|e| e.to_string())?;
                }
            }
        }
        if found {
            Ok(())
        } else {
            Err("That app isn't playing sound any more".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(key: &str, v: u8, m: bool, act: bool) -> AppVolume {
        AppVolume {
            name: key.to_string(),
            key: key.to_string(),
            volume: v,
            muted: m,
            active: act,
        }
    }

    #[test]
    fn groups_sessions_of_one_app() {
        let g = group(vec![
            a("zoom.exe", 50, true, false),
            a("firefox.exe", 40, true, false),
            a("firefox.exe", 80, false, true),
            a("system", 100, false, false),
        ]);
        assert_eq!(g.len(), 3);
        assert_eq!(g[0].key, "system");
        assert_eq!(
            g[1],
            AppVolume {
                name: "firefox.exe".into(),
                key: "firefox.exe".into(),
                volume: 80,
                muted: false,
                active: true
            }
        );
        assert_eq!(g[2].key, "zoom.exe");
    }
}
