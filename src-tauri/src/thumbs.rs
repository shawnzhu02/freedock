//! Hover previews: live thumbnails of an app's windows, drawn by the desktop compositor (DWM) into
//! the flyout window over boxes the page lays out. DWM keeps them live at no cost to us; they are
//! unregistered as soon as the flyout hides, so nothing is held while no preview is showing.

/// Fit a `src` sized picture inside the box (x, y, w, h), keeping its shape, centred.
pub fn fit(src: (i32, i32), b: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    let (sw, sh) = (src.0.max(1) as f64, src.1.max(1) as f64);
    let k = (b.2 as f64 / sw).min(b.3 as f64 / sh);
    let (w, h) = ((sw * k).round() as i32, (sh * k).round() as i32);
    (b.0 + (b.2 - w) / 2, b.1 + (b.3 - h) / 2, w, h)
}

#[cfg(windows)]
pub mod win {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Dwm::{
        DWM_THUMBNAIL_PROPERTIES, DWM_TNP_OPACITY, DWM_TNP_RECTDESTINATION,
        DWM_TNP_SOURCECLIENTAREAONLY, DWM_TNP_VISIBLE, DwmQueryThumbnailSourceSize,
        DwmRegisterThumbnail, DwmUnregisterThumbnail, DwmUpdateThumbnailProperties,
    };

    static LIVE: Mutex<Vec<isize>> = Mutex::new(Vec::new());
    /// The windows shown last, so laying the same preview out again doesn't fade it in a second time.
    static SHOWN: Mutex<Vec<i64>> = Mutex::new(Vec::new());
    /// Bumped by every place and clear, so a fade still running for an older preview stops.
    static GEN: AtomicU64 = AtomicU64::new(0);

    /// Show each source window's thumbnail in its box (physical px in `dest`'s client area). With
    /// `fade`, a newly shown preview's thumbnails fade in while the card opens (they are drawn by DWM
    /// over the page, so the page's own animation can't touch them).
    pub fn place(dest: isize, items: &[(i64, (i32, i32, i32, i32))], fade: bool) {
        let srcs: Vec<i64> = items.iter().map(|i| i.0).collect();
        let fresh = {
            let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
            let fresh = *shown != srcs;
            *shown = srcs;
            fresh
        };
        let fade = fade && fresh;
        release();
        let g = GEN.load(Ordering::SeqCst);
        let mut live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
        for &(src, b) in items {
            unsafe {
                let Ok(t) = DwmRegisterThumbnail(HWND(dest as *mut _), crate::apps::win::hwnd(src))
                else {
                    continue;
                };
                live.push(t);
                let size = DwmQueryThumbnailSourceSize(t).unwrap_or_default();
                let (x, y, w, h) = super::fit((size.cx, size.cy), b);
                let props = DWM_THUMBNAIL_PROPERTIES {
                    dwFlags: DWM_TNP_RECTDESTINATION
                        | DWM_TNP_VISIBLE
                        | DWM_TNP_SOURCECLIENTAREAONLY
                        | DWM_TNP_OPACITY,
                    opacity: if fade { 0 } else { 255 },
                    rcDestination: RECT {
                        left: x,
                        top: y,
                        right: x + w,
                        bottom: y + h,
                    },
                    fVisible: true.into(),
                    fSourceClientAreaOnly: false.into(),
                    ..Default::default()
                };
                let _ = DwmUpdateThumbnailProperties(t, &props);
            }
        }
        drop(live);
        if fade {
            std::thread::spawn(move || {
                // Start as the card is most of the way open, then 8 steps over about 140 ms.
                std::thread::sleep(Duration::from_millis(60));
                for step in 1..=8u32 {
                    let live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
                    if GEN.load(Ordering::SeqCst) != g {
                        return;
                    }
                    let props = DWM_THUMBNAIL_PROPERTIES {
                        dwFlags: DWM_TNP_OPACITY,
                        opacity: (step * 255 / 8) as u8,
                        ..Default::default()
                    };
                    for &t in live.iter() {
                        unsafe {
                            let _ = DwmUpdateThumbnailProperties(t, &props);
                        }
                    }
                    drop(live);
                    std::thread::sleep(Duration::from_millis(18));
                }
            });
        }
    }

    /// Unregister the thumbnails (and stop any fade).
    fn release() {
        GEN.fetch_add(1, Ordering::SeqCst);
        for t in LIVE.lock().unwrap_or_else(|e| e.into_inner()).drain(..) {
            unsafe {
                let _ = DwmUnregisterThumbnail(t);
            }
        }
    }

    /// The preview closed: drop its thumbnails, and fade the next one in again.
    pub fn clear() {
        SHOWN.lock().unwrap_or_else(|e| e.into_inner()).clear();
        release();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fits_keeping_shape() {
        assert_eq!(
            super::fit((1920, 1080), (0, 0, 200, 200)),
            (0, 43, 200, 113)
        );
        assert_eq!(
            super::fit((500, 1000), (10, 10, 200, 100)),
            (85, 10, 50, 100)
        );
    }
}
