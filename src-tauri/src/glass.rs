//! Live glass: what's on screen behind our glass windows (the dock, the settings panel and the agents'
//! popup), grabbed a few dozen times a second around each visible one, halved in size (the shader
//! blurs it anyway) and handed to that window's glass shader.
//!
//! The dock window is excluded from screen capture (WDA_EXCLUDEFROMCAPTURE), otherwise every grab
//! would contain the dock itself. That also keeps it out of screenshots and screen shares, which is
//! why live glass is a setting; with it off the glass shows the wallpaper as before.

use std::sync::Mutex;

/// One grab, ready for the UI: RGBA at half size, plus where it sits on the dock's monitor (physical px).
pub struct Frame {
    pub x0: i32,
    pub y0: i32,
    /// Physical px covered (twice the stored size).
    pub cover_w: i32,
    pub cover_h: i32,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// The windows that draw glass, by label.
pub const TARGETS: [&str; 3] = ["dock", "settings", "ask"];

static FRAMES: Mutex<std::collections::BTreeMap<String, Frame>> =
    Mutex::new(std::collections::BTreeMap::new());

/// Header (6 little-endian i32: x0, y0, cover_w, cover_h, w, h) followed by the RGBA bytes.
pub fn encode(f: &Frame) -> Vec<u8> {
    let mut out = Vec::with_capacity(24 + f.rgba.len());
    for v in [f.x0, f.y0, f.cover_w, f.cover_h, f.w as i32, f.h as i32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&f.rgba);
    out
}

pub fn latest(label: &str) -> Option<Vec<u8>> {
    FRAMES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(label)
        .map(encode)
}

/// BGRA (top-down, `w` x `h`) to RGBA at half size, averaging each 2x2 block.
/// A fast 64-bit fingerprint of a grab and where it was taken (FNV-style over 8-byte words).
pub fn fingerprint(bgra: &[u8], pos: (i32, i32)) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325 ^ ((pos.0 as u32 as u64) << 32 | pos.1 as u32 as u64);
    let mut words = bgra.chunks_exact(8);
    for w in &mut words {
        h = (h ^ u64::from_le_bytes(w.try_into().unwrap())).wrapping_mul(0x100000001b3);
    }
    for b in words.remainder() {
        h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
    }
    h
}

pub fn halve(bgra: &[u8], w: usize, h: usize) -> (Vec<u8>, usize, usize) {
    let (w2, h2) = (w / 2, h / 2);
    let mut out = vec![0u8; w2 * h2 * 4];
    for y in 0..h2 {
        for x in 0..w2 {
            let mut acc = [0u32; 3];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let i = ((2 * y + dy) * w + 2 * x + dx) * 4;
                acc[0] += bgra[i + 2] as u32;
                acc[1] += bgra[i + 1] as u32;
                acc[2] += bgra[i] as u32;
            }
            let o = (y * w2 + x) * 4;
            out[o] = (acc[0] / 4) as u8;
            out[o + 1] = (acc[1] / 4) as u8;
            out[o + 2] = (acc[2] / 4) as u8;
            out[o + 3] = 255;
        }
    }
    (out, w2, h2)
}

pub fn live_wanted(s: &crate::settings::Settings) -> bool {
    s.glass_live && matches!(s.style.as_str(), "glass" | "tinted" | "midnight")
}

#[cfg(windows)]
pub mod win {
    use std::ffi::c_void;
    use std::time::{Duration, Instant};

    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW, HBITMAP, HDC, HGDIOBJ,
        MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, ReleaseDC, SRCCOPY, SelectObject,
    };
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, IsWindowVisible, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    };

    use super::*;

    /// A reusable 32-bit DIB the screen is copied into.
    struct Canvas {
        dc: HDC,
        bmp: HBITMAP,
        old: HGDIOBJ,
        bits: *mut c_void,
        w: i32,
        h: i32,
    }

    impl Canvas {
        fn new(screen: HDC, w: i32, h: i32) -> Option<Canvas> {
            unsafe {
                let dc = CreateCompatibleDC(Some(screen));
                let bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: w,
                        biHeight: -h,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut bits = std::ptr::null_mut();
                let Ok(bmp) =
                    CreateDIBSection(Some(screen), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                else {
                    // Out of GDI memory: free the DC too, or each retry would leak one.
                    let _ = DeleteDC(dc);
                    return None;
                };
                let old = SelectObject(dc, bmp.into());
                Some(Canvas {
                    dc,
                    bmp,
                    old,
                    bits,
                    w,
                    h,
                })
            }
        }
    }

    impl Drop for Canvas {
        fn drop(&mut self) {
            unsafe {
                SelectObject(self.dc, self.old);
                let _ = DeleteObject(self.bmp.into());
                let _ = DeleteDC(self.dc);
            }
        }
    }

    fn exclude(h: HWND, on: bool) {
        unsafe {
            let _ = SetWindowDisplayAffinity(h, if on { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE });
        }
    }

    struct Target {
        canvas: Option<Canvas>,
        /// Fingerprint of the last grab (pixels and place), so an unchanged screen costs one pass.
        last: u64,
        changed_at: Instant,
        excluded: Option<(isize, bool)>,
    }

    /// Grab the screen around one window into its canvas. Returns the frame when it changed.
    fn grab(t: &mut Target, h: HWND) -> Option<Frame> {
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
        // A margin around the window, so the glass can bend in what's just outside it.
        let pad = (24 * unsafe { GetDpiForWindow(h) }.max(96) / 96) as i32;
        let x0 = (r.left - pad).max(m.left);
        let y0 = (r.top - pad).max(m.top);
        let x1 = (r.right + pad).min(m.right);
        let y1 = (r.bottom + pad).min(m.bottom);
        // Even sizes, so halving is exact.
        let (w, hh) = ((x1 - x0) & !1, (y1 - y0) & !1);
        if w < 4 || hh < 4 {
            return None;
        }
        let screen = unsafe { GetDC(None) };
        if t.canvas.as_ref().is_none_or(|c| c.w != w || c.h != hh) {
            t.canvas = Canvas::new(screen, w, hh);
        }
        let ok = t.canvas.as_ref().is_some_and(|c| {
            unsafe { BitBlt(c.dc, 0, 0, w, hh, Some(screen), x0, y0, SRCCOPY) }.is_ok()
        });
        unsafe { ReleaseDC(None, screen) };
        let c = t.canvas.as_ref().filter(|_| ok)?;
        let bgra =
            unsafe { std::slice::from_raw_parts(c.bits as *const u8, (w * hh * 4) as usize) };
        let pos = (x0 - m.left, y0 - m.top);
        // Unchanged screen and place: one pass over the grab, no copy, nothing else to do.
        let key = fingerprint(bgra, pos);
        if key == t.last {
            return None;
        }
        t.last = key;
        t.changed_at = Instant::now();
        let (rgba, w2, h2) = halve(bgra, w as usize, hh as usize);
        Some(Frame {
            x0: pos.0,
            y0: pos.1,
            cover_w: w,
            cover_h: hh,
            w: w2 as u32,
            h: h2 as u32,
            rgba,
        })
    }

    /// How long to wait before the next grab, from how recently anything behind a window changed:
    /// 30 a second while it moves, 8 a second just after, and 2 a second once all has been still
    /// for 5 s (a desktop with nothing moving costs almost nothing).
    pub fn pace(since_change: Duration) -> Duration {
        if since_change < Duration::from_secs(1) {
            Duration::from_millis(33)
        } else if since_change < Duration::from_secs(5) {
            Duration::from_millis(125)
        } else {
            Duration::from_millis(500)
        }
    }

    /// Runs for the life of the app; grabs only for shown windows while live glass is on.
    pub fn start(app: tauri::AppHandle) {
        use tauri::{Emitter, Manager};
        std::thread::spawn(move || {
            let mut targets: Vec<Target> = TARGETS
                .iter()
                .map(|_| Target {
                    canvas: None,
                    last: 0,
                    changed_at: Instant::now(),
                    excluded: None,
                })
                .collect();
            let mut wait = Duration::from_millis(33);
            loop {
                std::thread::sleep(wait);
                // Paced by the busiest shown window; with none shown (or live glass off), check rarely.
                wait = Duration::from_millis(500);
                let Some(live) = crate::appbar::with_settings(|_, s| live_wanted(s)) else {
                    continue;
                };
                for (label, t) in TARGETS.iter().zip(targets.iter_mut()) {
                    let Some(h) = app.get_webview_window(label).and_then(|w| w.hwnd().ok()) else {
                        continue;
                    };
                    let raw = h.0 as isize;
                    if t.excluded != Some((raw, live)) {
                        exclude(h, live);
                        t.excluded = Some((raw, live));
                    }
                    let showing = unsafe { IsWindowVisible(h) }.as_bool()
                        && (*label != "dock" || crate::visibility::win::shown());
                    if !live || !showing {
                        t.canvas = None;
                        t.last = 0;
                        if !live
                            && FRAMES
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .remove(*label)
                                .is_some()
                        {
                            let _ = app.emit("glass-frame", *label);
                        }
                        continue;
                    }
                    let got = grab(t, h);
                    wait = wait.min(pace(t.changed_at.elapsed()));
                    if let Some(f) = got {
                        FRAMES
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(label.to_string(), f);
                        let _ = app.emit("glass-frame", *label);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_sees_pixels_and_place() {
        let a = vec![7u8; 4096];
        let mut b = a.clone();
        b[4000] = 8;
        assert_eq!(fingerprint(&a, (0, 0)), fingerprint(&a.clone(), (0, 0)));
        assert_ne!(
            fingerprint(&a, (0, 0)),
            fingerprint(&b, (0, 0)),
            "one byte changed"
        );
        assert_ne!(
            fingerprint(&a, (0, 0)),
            fingerprint(&a, (0, 1)),
            "same pixels, moved"
        );
    }

    #[cfg(windows)]
    #[test]
    fn pacing_slows_down_when_still() {
        use std::time::Duration;
        assert_eq!(
            win::pace(Duration::from_millis(200)),
            Duration::from_millis(33)
        );
        assert_eq!(
            win::pace(Duration::from_secs(3)),
            Duration::from_millis(125)
        );
        assert_eq!(
            win::pace(Duration::from_secs(60)),
            Duration::from_millis(500)
        );
    }

    #[test]
    fn halving_averages_and_swaps_to_rgba() {
        // 2x2 BGRA block: blue-ish values 0, 4, 8, 12 average to 6.
        let px = |b: u8, g: u8, r: u8| [b, g, r, 255];
        let bgra: Vec<u8> = [
            px(0, 10, 100),
            px(4, 10, 100),
            px(8, 30, 100),
            px(12, 30, 100),
        ]
        .concat();
        let (out, w, h) = halve(&bgra, 2, 2);
        assert_eq!((w, h), (1, 1));
        assert_eq!(out, vec![100, 20, 6, 255]);
    }

    #[test]
    fn header_then_pixels() {
        let f = Frame {
            x0: -3,
            y0: 7,
            cover_w: 4,
            cover_h: 2,
            w: 2,
            h: 1,
            rgba: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let b = encode(&f);
        assert_eq!(&b[0..4], &(-3i32).to_le_bytes());
        assert_eq!(&b[20..24], &1i32.to_le_bytes());
        assert_eq!(&b[24..], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
