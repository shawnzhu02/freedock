//! What the glass refracts: the desktop wallpaper and where the dock sits on the screen.
//! (Step one of the glass. Step two adds the windows behind the dock through screen capture.)

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Geometry {
    /// The monitor and the dock window, in physical pixels.
    pub mon_w: i32,
    pub mon_h: i32,
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Wallpaper {
    /// The image as a data URL (read once per change; the UI keeps it).
    pub data: Option<String>,
    /// How Windows fits it: "fill", "fit", "stretch", "center", "tile" or "span".
    pub fit: String,
}

/// HKCU\Control Panel\Desktop WallpaperStyle + TileWallpaper to a fit name.
pub fn fit_name(style: &str, tile: &str) -> &'static str {
    match (style.trim(), tile.trim()) {
        (_, "1") => "tile",
        ("10", _) => "fill",
        ("6", _) => "fit",
        ("2", _) => "stretch",
        ("22", _) => "span",
        ("0", _) => "center",
        _ => "fill",
    }
}

fn mime(bytes: &[u8]) -> &'static str {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [b'B', b'M', ..] => "image/bmp",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "image/webp",
        _ => "image/jpeg",
    }
}

#[cfg(windows)]
pub mod win {
    use base64::Engine;
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
        SystemParametersInfoW,
    };
    use windows::core::{HSTRING, w};

    use super::*;

    fn reg(name: &str) -> String {
        let mut buf = [0u16; 64];
        let mut len = (buf.len() * 2) as u32;
        let ok = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!(r"Control Panel\Desktop"),
                &HSTRING::from(name),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr() as *mut _),
                Some(&mut len),
            )
        };
        if ok.is_err() {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)])
    }

    pub fn current() -> Wallpaper {
        let mut buf = [0u16; 1024];
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETDESKWALLPAPER,
                buf.len() as u32,
                Some(buf.as_mut_ptr() as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
        let path = if ok.is_ok() {
            String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(0)])
        } else {
            String::new()
        };
        let data = (!path.is_empty())
            .then(|| std::fs::read(&path).ok())
            .flatten()
            .map(|b| {
                format!(
                    "data:{};base64,{}",
                    mime(&b),
                    base64::engine::general_purpose::STANDARD.encode(&b)
                )
            });
        Wallpaper {
            data,
            fit: fit_name(&reg("WallpaperStyle"), &reg("TileWallpaper")).into(),
        }
    }

    pub fn geometry(raw: isize) -> Geometry {
        let h = HWND(raw as *mut _);
        let mut r = RECT::default();
        let mut mi = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            let _ = GetWindowRect(h, &mut r);
            let _ = GetMonitorInfoW(MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST), &mut mi);
        }
        let m = mi.rcMonitor;
        Geometry {
            mon_w: m.right - m.left,
            mon_h: m.bottom - m.top,
            left: r.left - m.left,
            top: r.top - m.top,
            width: r.right - r.left,
            height: r.bottom - r.top,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_names() {
        assert_eq!(fit_name("10", "0"), "fill");
        assert_eq!(fit_name("6", "0"), "fit");
        assert_eq!(fit_name("0", "1"), "tile");
        assert_eq!(
            fit_name("", ""),
            "fill",
            "unknown falls back to Windows' default"
        );
    }

    #[test]
    fn sniffs_image_types() {
        assert_eq!(mime(&[0x89, b'P', b'N', b'G', 0]), "image/png");
        assert_eq!(
            mime(&[0xFF, 0xD8, 0xFF]),
            "image/jpeg",
            "TranscodedWallpaper has no extension"
        );
    }
}
