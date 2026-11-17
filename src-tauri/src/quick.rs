//! The quick-settings flyout's backends: speaker volume and mute, Wi-Fi networks, the Wi-Fi and
//! Bluetooth radios, and screen brightness. Everything here is local; nothing goes over the network
//! except Windows joining a Wi-Fi network the user already has a saved profile for.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, Default)]
pub struct Audio {
    /// 0-100.
    pub volume: u8,
    pub muted: bool,
    /// The output device's name, e.g. "Speakers (Realtek(R) Audio)".
    pub device: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Network {
    pub ssid: String,
    /// 0-100.
    pub signal: u32,
    pub secure: bool,
    pub connected: bool,
    /// A saved profile exists, so it can be joined without asking for a password.
    pub saved: bool,
    /// The saved profile's name, used to connect.
    #[serde(skip)]
    pub profile: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Radios {
    /// None when there's no such radio.
    pub wifi: Option<bool>,
    pub bluetooth: Option<bool>,
}

/// One entry per SSID, the strongest kept; the connected one first, then saved ones, then by signal.
pub fn tidy(mut nets: Vec<Network>) -> Vec<Network> {
    nets.retain(|n| !n.ssid.is_empty());
    nets.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then(b.saved.cmp(&a.saved))
            .then(b.signal.cmp(&a.signal))
    });
    let mut out: Vec<Network> = vec![];
    for n in nets {
        if let Some(o) = out.iter_mut().find(|o| o.ssid == n.ssid) {
            o.connected |= n.connected;
            if o.profile.is_empty() {
                o.profile = n.profile;
                o.saved |= n.saved;
            }
        } else {
            out.push(n);
        }
    }
    out
}

#[cfg(windows)]
pub mod win {
    use super::*;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        EDataFlow, ERole, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eCommunications,
        eConsole, eRender,
    };
    use windows::Win32::NetworkManagement::WiFi::*;
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, STGM_READ,
    };
    use windows::core::{GUID, Interface, PCWSTR};

    fn com() {
        // Already initialised on this thread (either model) is fine.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    }

    fn endpoint() -> windows::core::Result<(IAudioEndpointVolume, String)> {
        endpoint_of(eRender, eConsole)
    }

    fn endpoint_of(
        flow: EDataFlow,
        role: ERole,
    ) -> windows::core::Result<(IAudioEndpointVolume, String)> {
        com();
        unsafe {
            let e: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let dev = e.GetDefaultAudioEndpoint(flow, role)?;
            let vol: IAudioEndpointVolume = dev.Activate(CLSCTX_ALL, None)?;
            // PKEY_Device_FriendlyName
            let key = PROPERTYKEY {
                fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
                pid: 14,
            };
            let name = dev
                .OpenPropertyStore(STGM_READ)
                .and_then(|s| s.GetValue(&key))
                .map(|v| v.to_string())
                .unwrap_or_default();
            Ok((vol, name))
        }
    }

    pub fn audio() -> Option<Audio> {
        let (v, device) = endpoint().ok()?;
        unsafe {
            Some(Audio {
                volume: (v.GetMasterVolumeLevelScalar().ok()? * 100.0).round() as u8,
                muted: v.GetMute().ok()?.as_bool(),
                device,
            })
        }
    }

    /// The default microphone (the communications one, which calls use); None without one.
    pub fn mic() -> Option<Audio> {
        let (v, device) = endpoint_of(eCapture, eCommunications)
            .or_else(|_| endpoint_of(eCapture, eConsole))
            .ok()?;
        unsafe {
            Some(Audio {
                volume: (v.GetMasterVolumeLevelScalar().ok()? * 100.0).round() as u8,
                muted: v.GetMute().ok()?.as_bool(),
                device,
            })
        }
    }

    /// Mute or unmute both default microphones (calls and everything else), so nothing still hears the user.
    pub fn set_mic_muted(muted: bool) -> Result<(), String> {
        let mut done = false;
        for role in [eCommunications, eConsole] {
            if let Ok((v, _)) = endpoint_of(eCapture, role) {
                unsafe {
                    v.SetMute(muted, std::ptr::null())
                        .map_err(|e| e.to_string())?
                };
                done = true;
            }
        }
        if done {
            Ok(())
        } else {
            Err("No microphone".into())
        }
    }

    const PERSONALIZE: windows::core::PCWSTR =
        windows::core::w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");

    fn read_dword(name: windows::core::PCWSTR) -> Option<u32> {
        use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
        let mut v = 0u32;
        let mut n = 4u32;
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PERSONALIZE,
                name,
                RRF_RT_REG_DWORD,
                None,
                Some(&mut v as *mut u32 as *mut _),
                Some(&mut n),
            )
        };
        r.is_ok().then_some(v)
    }

    /// Dark mode = apps use the dark theme (AppsUseLightTheme = 0).
    pub fn dark_mode() -> Option<bool> {
        read_dword(windows::core::w!("AppsUseLightTheme")).map(|v| v == 0)
    }

    /// Switches apps and Windows itself together, like the Settings page's Light/Dark choice, then
    /// tells running programs the theme changed.
    pub fn set_dark_mode(dark: bool) -> Result<(), String> {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::System::Registry::{HKEY_CURRENT_USER, REG_DWORD, RegSetKeyValueW};
        use windows::Win32::UI::WindowsAndMessaging::{
            HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
        };
        let v: u32 = if dark { 0 } else { 1 };
        let apps = windows::core::w!("AppsUseLightTheme");
        let system = windows::core::w!("SystemUsesLightTheme");
        let before = read_dword(apps);
        let put = |name: windows::core::PCWSTR, v: u32| unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                PERSONALIZE,
                name,
                REG_DWORD.0,
                Some(&v as *const u32 as *const _),
                4,
            )
        };
        let r = put(apps, v);
        if r.is_err() {
            return Err(format!("couldn't change the theme ({})", r.0));
        }
        let r = put(system, v);
        if r.is_err() {
            // Don't leave apps and Windows half switched.
            if let Some(b) = before {
                let _ = put(apps, b);
            }
            return Err(format!("couldn't change the theme ({})", r.0));
        }
        let topic = windows::core::w!("ImmersiveColorSet");
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(topic.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                1000,
                None,
            );
        }
        Ok(())
    }

    pub fn set_volume(volume: Option<u8>, muted: Option<bool>) -> Result<(), String> {
        let (v, _) = endpoint().map_err(|e| e.to_string())?;
        unsafe {
            if let Some(p) = volume {
                v.SetMasterVolumeLevelScalar(p.min(100) as f32 / 100.0, std::ptr::null())
                    .map_err(|e| e.to_string())?;
            }
            if let Some(m) = muted {
                v.SetMute(m, std::ptr::null()).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    struct Wlan(HANDLE);
    impl Drop for Wlan {
        fn drop(&mut self) {
            unsafe { WlanCloseHandle(self.0, None) };
        }
    }

    fn wlan() -> Option<(Wlan, Vec<GUID>)> {
        let mut ver = 0u32;
        let mut h = HANDLE::default();
        if unsafe { WlanOpenHandle(2, None, &mut ver, &mut h) } != 0 {
            return None;
        }
        let w = Wlan(h);
        let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
        if unsafe { WlanEnumInterfaces(w.0, None, &mut list) } != 0 || list.is_null() {
            return None;
        }
        let ids = unsafe {
            let l = &*list;
            let items =
                std::slice::from_raw_parts(l.InterfaceInfo.as_ptr(), l.dwNumberOfItems as usize);
            let ids = items.iter().map(|i| i.InterfaceGuid).collect();
            WlanFreeMemory(list as _);
            ids
        };
        Some((w, ids))
    }

    fn wide(s: &[u16]) -> String {
        String::from_utf16_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())])
    }

    /// Nearby networks. `scan` asks the adapter for a fresh scan first (results arrive within a few
    /// seconds; the next call sees them).
    pub fn networks(scan: bool) -> Vec<Network> {
        let Some((w, ids)) = wlan() else {
            return vec![];
        };
        let mut out = vec![];
        for id in ids {
            if scan {
                unsafe { WlanScan(w.0, &id, None, None, None) };
            }
            let mut list: *mut WLAN_AVAILABLE_NETWORK_LIST = std::ptr::null_mut();
            if unsafe { WlanGetAvailableNetworkList(w.0, &id, 0, None, &mut list) } != 0
                || list.is_null()
            {
                continue;
            }
            unsafe {
                let l = &*list;
                for n in std::slice::from_raw_parts(l.Network.as_ptr(), l.dwNumberOfItems as usize)
                {
                    let len = (n.dot11Ssid.uSSIDLength as usize).min(32);
                    out.push(Network {
                        ssid: String::from_utf8_lossy(&n.dot11Ssid.ucSSID[..len]).into_owned(),
                        signal: n.wlanSignalQuality,
                        secure: n.bSecurityEnabled.as_bool(),
                        connected: n.dwFlags & WLAN_AVAILABLE_NETWORK_CONNECTED != 0,
                        saved: n.dwFlags & WLAN_AVAILABLE_NETWORK_HAS_PROFILE != 0,
                        profile: wide(&n.strProfileName),
                    });
                }
                WlanFreeMemory(list as _);
            }
        }
        tidy(out)
    }

    /// Join a network with a saved profile, or disconnect (`ssid` None).
    pub fn connect(ssid: Option<&str>) -> Result<(), String> {
        let (w, ids) = wlan().ok_or("no Wi-Fi adapter")?;
        let id = *ids.first().ok_or("no Wi-Fi adapter")?;
        let Some(ssid) = ssid else {
            let r = unsafe { WlanDisconnect(w.0, &id, None) };
            return if r == 0 {
                Ok(())
            } else {
                Err(format!("disconnect failed ({r})"))
            };
        };
        let net = networks(false)
            .into_iter()
            .find(|n| n.ssid == ssid)
            .ok_or("that network isn't in range")?;
        if !net.saved || net.profile.is_empty() {
            return Err("not a saved network: join it once from Windows' Wi-Fi settings".into());
        }
        let profile: Vec<u16> = net.profile.encode_utf16().chain([0]).collect();
        let params = WLAN_CONNECTION_PARAMETERS {
            wlanConnectionMode: wlan_connection_mode_profile,
            strProfile: PCWSTR(profile.as_ptr()),
            pDot11Ssid: std::ptr::null_mut(),
            pDesiredBssidList: std::ptr::null_mut(),
            dot11BssType: dot11_BSS_type_any,
            dwFlags: 0,
        };
        let r = unsafe { WlanConnect(w.0, &id, &params, None) };
        if r == 0 {
            Ok(())
        } else {
            Err(format!("connect failed ({r})"))
        }
    }

    pub fn radios() -> Radios {
        use windows::Devices::Radios::{Radio, RadioKind, RadioState};
        let mut out = Radios::default();
        let Ok(list) = Radio::GetRadiosAsync().and_then(|a| a.join()) else {
            return out;
        };
        for r in list {
            let on = r.State().map(|s| s == RadioState::On).unwrap_or(false);
            match r.Kind() {
                Ok(RadioKind::WiFi) => out.wifi = Some(out.wifi.unwrap_or(false) || on),
                Ok(RadioKind::Bluetooth) => {
                    out.bluetooth = Some(out.bluetooth.unwrap_or(false) || on)
                }
                _ => {}
            }
        }
        out
    }

    pub fn set_radio(kind: &str, on: bool) -> Result<(), String> {
        use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};
        let want = match kind {
            "wifi" => RadioKind::WiFi,
            "bluetooth" => RadioKind::Bluetooth,
            _ => return Err("unknown radio".into()),
        };
        let list = Radio::GetRadiosAsync()
            .and_then(|a| a.join())
            .map_err(|e| e.to_string())?;
        let mut found = false;
        for r in list {
            if r.Kind().ok() == Some(want) {
                found = true;
                let s = r
                    .SetStateAsync(if on { RadioState::On } else { RadioState::Off })
                    .and_then(|a| a.join())
                    .map_err(|e| e.to_string())?;
                if s != RadioAccessStatus::Allowed {
                    return Err("Windows didn't allow switching that radio".into());
                }
            }
        }
        if found {
            Ok(())
        } else {
            Err("no such radio".into())
        }
    }

    fn powershell(script: &str) -> Option<String> {
        use std::os::windows::process::CommandExt;
        let out = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// The built-in panel's brightness, 0-100, or None (external monitors, desktops).
    pub fn brightness() -> Option<u8> {
        powershell("(Get-CimInstance -Namespace root/wmi -ClassName WmiMonitorBrightness -ErrorAction Stop | Select-Object -First 1).CurrentBrightness")?
            .parse()
            .ok()
    }

    pub fn set_brightness(level: u8) -> Result<(), String> {
        let level = level.min(100);
        powershell(&format!(
            "Get-CimInstance -Namespace root/wmi -ClassName WmiMonitorBrightnessMethods -ErrorAction Stop | Invoke-CimMethod -MethodName WmiSetBrightness -Arguments @{{Timeout=1; Brightness=[byte]{level}}} | Out-Null"
        ))
        .map(|_| ())
        .ok_or_else(|| "this screen's brightness can't be set from here".into())
    }

    #[allow(dead_code)]
    fn _assert_interface<T: Interface>() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(ssid: &str, signal: u32, connected: bool, saved: bool) -> Network {
        Network {
            ssid: ssid.into(),
            signal,
            secure: true,
            connected,
            saved,
            profile: if saved { ssid.into() } else { String::new() },
        }
    }

    #[test]
    fn networks_dedupe_and_order() {
        let out = tidy(vec![
            n("Cafe", 90, false, false),
            n("Home", 60, true, true),
            n("Home", 80, false, true),
            n("", 99, false, false),
            n("Work", 40, false, true),
        ]);
        let names: Vec<_> = out.iter().map(|n| n.ssid.as_str()).collect();
        assert_eq!(names, ["Home", "Work", "Cafe"]);
        assert!(out[0].connected);
    }
}
