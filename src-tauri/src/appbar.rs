//! The dock as a Windows AppBar: it reserves its strip so maximized windows stop above (or below) it,
//! and Windows tells it when a fullscreen app (game, video) starts, so it can step aside.

use std::sync::Mutex;

use crate::settings::{Edge, Settings, Shape};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// The strip along the dock's edge of the monitor, in physical pixels: dock thickness plus a margin each side.
pub fn strip(mon: Rect, edge: Edge, thickness: i32, margin: i32) -> Rect {
    let h = thickness + 2 * margin;
    match edge {
        Edge::Bottom => Rect {
            top: mon.bottom - h,
            ..mon
        },
        Edge::Top => Rect {
            bottom: mon.top + h,
            ..mon
        },
        Edge::Left => Rect {
            right: mon.left + h,
            ..mon
        },
        Edge::Right => Rect {
            left: mon.right - h,
            ..mon
        },
    }
}

/// Where the dock window itself goes inside its strip: the whole strip, or a pill with margins.
/// A pill hugs its content (`content`, its length along the edge in physical px) up to `max_share` of the
/// edge, centred at `offset` along it (0..1) but never past either end.
pub fn window_rect(
    strip: Rect,
    edge: Edge,
    shape: Shape,
    max_share: f64,
    margin: i32,
    content: Option<i32>,
    offset: f64,
) -> Rect {
    match shape {
        Shape::Full => strip,
        Shape::Pill => {
            let vertical = edge.vertical();
            let len = if vertical {
                strip.height()
            } else {
                strip.width()
            };
            let max = ((len as f64) * max_share.clamp(0.2, 1.0)).round() as i32;
            let w = content.map_or(max, |c| c.clamp(120.min(max), max));
            let begin = if vertical { strip.top } else { strip.left };
            let centre = begin + ((len as f64) * offset.clamp(0.0, 1.0)).round() as i32;
            let start = (centre - w / 2).clamp(begin, begin + len - w);
            if vertical {
                Rect {
                    left: strip.left + margin,
                    right: strip.right - margin,
                    top: start,
                    bottom: start + w,
                }
            } else {
                Rect {
                    left: start,
                    right: start + w,
                    top: strip.top + margin,
                    bottom: strip.bottom - margin,
                }
            }
        }
    }
}

/// Where a dragged dock lands: the monitor edge nearest the pointer, and the dock's centre along that
/// edge as a 0..1 offset (within 4% of the middle snaps to exactly centred).
pub fn snap(mon: Rect, pointer: (i32, i32), centre: (i32, i32)) -> (Edge, f64) {
    let (x, y) = pointer;
    let edges = [
        (Edge::Bottom, mon.bottom - y),
        (Edge::Top, y - mon.top),
        (Edge::Left, x - mon.left),
        (Edge::Right, mon.right - x),
    ];
    let edge = edges
        .iter()
        .min_by_key(|(_, d)| *d)
        .map(|(e, _)| *e)
        .unwrap_or_default();
    let o = if edge.vertical() {
        (centre.1 - mon.top) as f64 / mon.height().max(1) as f64
    } else {
        (centre.0 - mon.left) as f64 / mon.width().max(1) as f64
    };
    let o = o.clamp(0.0, 1.0);
    (
        edge,
        if (o - 0.5).abs() < 0.04 {
            0.5
        } else {
            (o * 1000.0).round() / 1000.0
        },
    )
}

/// Only the "always" mode keeps a strip of screen for itself.
pub fn reserves(s: &Settings) -> bool {
    s.visibility == "always"
}

/// Read what a hot loop needs from the current settings without cloning them (the widget list and
/// its options make a full copy 50 times a second add up).
/// One monitor: its Windows device name, its rectangle, and whether it is the main one.
pub type Monitor = (String, Rect, bool);

/// The monitor named `want` (a device name such as `\\.\DISPLAY2`), else the main one, else any.
pub fn pick_monitor(list: &[Monitor], want: &str) -> Option<Rect> {
    list.iter()
        .find(|m| !want.is_empty() && m.0 == want)
        .or_else(|| list.iter().find(|m| m.2))
        .or_else(|| list.first())
        .map(|m| m.1)
}

pub fn with_settings<R>(f: impl FnOnce(isize, &Settings) -> R) -> Option<R> {
    DOCK.lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|d| f(d.hwnd, &d.settings))
}

/// What the window procedure needs to lay the dock out again after a display or DPI change.
struct Dock {
    hwnd: isize,
    settings: Settings,
    /// The width the UI's tiles need, in logical px (pill only).
    content: Option<u32>,
}

static DOCK: Mutex<Option<Dock>> = Mutex::new(None);
/// The dock is being dragged: layout() leaves it where the pointer has it.
static DRAGGING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(windows)]
pub mod win {
    use std::ffi::c_void;

    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST,
        MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MONITORINFOEXW, MonitorFromPoint,
    };
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Shell::{
        ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_TOP, ABM_ACTIVATE, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE,
        ABM_SETPOS, ABM_WINDOWPOSCHANGED, ABN_FULLSCREENAPP, ABN_POSCHANGED, APPBARDATA,
        DefSubclassProc, SHAppBarMessage, SetWindowSubclass,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, RegisterShellHookWindow,
        RegisterWindowMessageW, SPI_SETDESKWALLPAPER, STYLESTRUCT, SW_HIDE, SW_SHOWNOACTIVATE,
        SWP_NOACTIVATE, SetWindowLongPtrW, SetWindowPos, ShowWindow, WM_ACTIVATE, WM_DISPLAYCHANGE,
        WM_DPICHANGED, WM_SETTINGCHANGE, WM_STYLECHANGING, WM_WINDOWPOSCHANGED, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW,
    };
    use windows::core::w;

    use super::*;

    fn hwnd(raw: isize) -> HWND {
        HWND(raw as *mut c_void)
    }

    fn callback_msg() -> u32 {
        unsafe { RegisterWindowMessageW(w!("freedock.appbar")) }
    }

    /// The shell hook message: window created/destroyed/activated, and HSHELL_FLASH for attention.
    fn shellhook_msg() -> u32 {
        unsafe { RegisterWindowMessageW(w!("SHELLHOOK")) }
    }

    fn data(h: HWND) -> APPBARDATA {
        APPBARDATA {
            cbSize: size_of::<APPBARDATA>() as u32,
            hWnd: h,
            ..Default::default()
        }
    }

    fn primary_monitor() -> Rect {
        let mut mi = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            let m = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
            let _ = GetMonitorInfoW(m, &mut mi);
        }
        let r = mi.rcMonitor;
        Rect {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }

    fn monitor_info(m: HMONITOR) -> Option<Monitor> {
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        unsafe { GetMonitorInfoW(m, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO) }
            .as_bool()
            .then_some(())?;
        let n = mi
            .szDevice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(mi.szDevice.len());
        let r = mi.monitorInfo.rcMonitor;
        Some((
            String::from_utf16_lossy(&mi.szDevice[..n]),
            Rect {
                left: r.left,
                top: r.top,
                right: r.right,
                bottom: r.bottom,
            },
            mi.monitorInfo.dwFlags & 1 != 0,
        ))
    }

    /// Every monitor, in Windows' order.
    pub fn monitors() -> Vec<Monitor> {
        let mut out: Vec<Monitor> = Vec::new();
        unsafe extern "system" fn each(
            m: HMONITOR,
            _: HDC,
            _: *mut RECT,
            lp: LPARAM,
        ) -> windows::core::BOOL {
            if let Some(x) = monitor_info(m) {
                unsafe { (*(lp.0 as *mut Vec<Monitor>)).push(x) };
            }
            windows::core::BOOL(1)
        }
        unsafe {
            let _ =
                EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut out as *mut _ as isize));
        }
        out
    }

    /// The monitor the settings put the dock on (the main one if that monitor is gone).
    fn dock_monitor(s: &Settings) -> Rect {
        pick_monitor(&monitors(), &s.monitor).unwrap_or_else(primary_monitor)
    }

    /// Reserve the strip and move the window into it. Called at start and on every display change.
    pub fn layout() {
        // Copied out of the lock first: SetWindowPos sends messages that can land back in here.
        static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        use std::sync::atomic::Ordering::SeqCst;
        let Some((raw, s, content)) = DOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|d| (d.hwnd, d.settings.clone(), d.content))
        else {
            return;
        };
        if DRAGGING.load(SeqCst) || BUSY.swap(true, SeqCst) {
            return;
        }
        let h = hwnd(raw);
        let scale = unsafe { GetDpiForWindow(h) }.max(96) as f64 / 96.0;
        let px = |v: u32| (v as f64 * scale).round() as i32;
        let want = strip(dock_monitor(&s), s.edge, px(s.height), px(s.margin));
        let mut d = data(h);
        d.uEdge = match s.edge {
            Edge::Top => ABE_TOP,
            Edge::Bottom => ABE_BOTTOM,
            Edge::Left => ABE_LEFT,
            Edge::Right => ABE_RIGHT,
        };
        let got = if reserves(&s) {
            d.rc = RECT {
                left: want.left,
                top: want.top,
                right: want.right,
                bottom: want.bottom,
            };
            unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut d) };
            // QUERYPOS may move the strip (another AppBar on that edge); keep our thickness from where it landed.
            let (hgt, wid) = (want.height(), want.width());
            match s.edge {
                Edge::Top => d.rc.bottom = d.rc.top + hgt,
                Edge::Bottom => d.rc.top = d.rc.bottom - hgt,
                Edge::Left => d.rc.right = d.rc.left + wid,
                Edge::Right => d.rc.left = d.rc.right - wid,
            }
            unsafe { SHAppBarMessage(ABM_SETPOS, &mut d) };
            Rect {
                left: d.rc.left,
                top: d.rc.top,
                right: d.rc.right,
                bottom: d.rc.bottom,
            }
        } else {
            // Floating (smart/autohide): stay registered for fullscreen notices, but reserve a zero-size
            // strip, so maximized windows reach the screen edge and the dock floats over them.
            d.rc = match s.edge {
                Edge::Top => RECT {
                    left: want.left,
                    top: want.top,
                    right: want.right,
                    bottom: want.top,
                },
                Edge::Bottom => RECT {
                    left: want.left,
                    top: want.bottom,
                    right: want.right,
                    bottom: want.bottom,
                },
                Edge::Left => RECT {
                    left: want.left,
                    top: want.top,
                    right: want.left,
                    bottom: want.bottom,
                },
                Edge::Right => RECT {
                    left: want.right,
                    top: want.top,
                    right: want.right,
                    bottom: want.bottom,
                },
            };
            unsafe { SHAppBarMessage(ABM_SETPOS, &mut d) };
            want
        };
        let r = window_rect(
            got,
            s.edge,
            s.shape,
            s.pill_width,
            px(s.margin),
            content.map(px),
            s.offset,
        );
        unsafe {
            let _ = SetWindowPos(
                h,
                Some(HWND_TOPMOST),
                r.left,
                r.top,
                r.width(),
                r.height(),
                SWP_NOACTIVATE,
            );
        }
        BUSY.store(false, SeqCst);
        crate::notify("geometry-changed");
    }

    unsafe extern "system" fn subclass(
        h: HWND,
        msg: u32,
        wp: WPARAM,
        lp: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        // Tauri rebuilds the extended style from its own flags whenever click-through is switched
        // (Smart and Auto-hide do that every time the dock tucks or shows), which dropped these two.
        // The dock then took focus on every click, so a flyout opened by that click lost focus at once
        // and closed: nothing on the dock seemed to work. Keep them on whatever rewrites the style.
        if msg == WM_STYLECHANGING && wp.0 as i32 == GWL_EXSTYLE.0 {
            let st = lp.0 as *mut STYLESTRUCT;
            if !st.is_null() {
                unsafe { (*st).styleNew |= WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 };
            }
        }
        if msg == callback_msg() {
            match wp.0 as u32 {
                ABN_FULLSCREENAPP => unsafe {
                    // A game or video went fullscreen (lp = 1) or left it (lp = 0).
                    let step_aside = DOCK
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .as_ref()
                        .is_none_or(|d| d.settings.hide_fullscreen);
                    let _ = ShowWindow(
                        h,
                        if lp.0 != 0 && step_aside {
                            SW_HIDE
                        } else {
                            SW_SHOWNOACTIVATE
                        },
                    );
                    if lp.0 == 0 {
                        layout();
                    }
                },
                ABN_POSCHANGED => layout(),
                _ => {}
            }
            return LRESULT(0);
        }
        if msg == shellhook_msg() {
            const HSHELL_WINDOWCREATED: u32 = 1;
            const HSHELL_WINDOWDESTROYED: u32 = 2;
            const HSHELL_WINDOWACTIVATED: u32 = 4;
            const HSHELL_RUDEAPPACTIVATED: u32 = 0x8004;
            const HSHELL_FLASH: u32 = 0x8006;
            let target = lp.0 as i64;
            match wp.0 as u32 {
                HSHELL_FLASH => crate::apps::win::flash(target, true),
                HSHELL_WINDOWACTIVATED | HSHELL_RUDEAPPACTIVATED | HSHELL_WINDOWDESTROYED => {
                    crate::apps::win::flash(target, false)
                }
                HSHELL_WINDOWCREATED => {
                    crate::apps::win::CHANGED.store(true, std::sync::atomic::Ordering::SeqCst)
                }
                _ => {}
            }
            return LRESULT(0);
        }
        match msg {
            WM_SETTINGCHANGE if wp.0 as u32 == SPI_SETDESKWALLPAPER.0 => {
                crate::notify("wallpaper-changed")
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED | WM_SETTINGCHANGE => layout(),
            WM_ACTIVATE => unsafe {
                let mut d = data(h);
                SHAppBarMessage(ABM_ACTIVATE, &mut d);
            },
            WM_WINDOWPOSCHANGED => unsafe {
                let mut d = data(h);
                SHAppBarMessage(ABM_WINDOWPOSCHANGED, &mut d);
            },
            _ => {}
        }
        unsafe { DefSubclassProc(h, msg, wp, lp) }
    }

    pub fn register(raw: isize, settings: Settings) {
        let h = hwnd(raw);
        unsafe {
            // Never takes focus from the window the user is typing in, and never shows in Alt-Tab.
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
            SetWindowLongPtrW(
                h,
                GWL_EXSTYLE,
                ex | (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as isize,
            );
            let mut d = data(h);
            d.uCallbackMessage = callback_msg();
            SHAppBarMessage(ABM_NEW, &mut d);
            let _ = SetWindowSubclass(h, Some(subclass), 1, 0);
            let _ = RegisterShellHookWindow(h);
        }
        *DOCK.lock().unwrap_or_else(|e| e.into_inner()) = Some(Dock {
            hwnd: raw,
            settings,
            content: None,
        });
        layout();
        crate::taskbar::win::refresh();
    }

    /// Follow the pointer with the dock while the left button is held, then land on the nearest edge.
    /// Runs on its own thread; `done` gets the new edge and offset.
    pub fn drag(done: impl FnOnce(Edge, f64, Option<String>) + Send + 'static) {
        use std::sync::atomic::Ordering::SeqCst;
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MENU};
        use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect, SWP_NOSIZE};
        let Some(raw) = DOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|d| d.hwnd)
        else {
            return;
        };
        if DRAGGING.swap(true, SeqCst) {
            return;
        }
        std::thread::spawn(move || {
            let h = hwnd(raw);
            let (mut p, mut r) = (POINT::default(), RECT::default());
            unsafe {
                let _ = GetCursorPos(&mut p);
                let _ = GetWindowRect(h, &mut r);
            }
            let grab = (p.x - r.left, p.y - r.top);
            // Alt+drag: a lone Alt press and release would open the focused app's menu bar. An unassigned
            // key in between (0xE8, the usual "menu mask" key) makes Windows treat it as a combination.
            if unsafe { GetAsyncKeyState(VK_MENU.0 as i32) } as u16 & 0x8000 != 0 {
                use windows::Win32::UI::Input::KeyboardAndMouse::{
                    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
                    VIRTUAL_KEY,
                };
                let key = |flags| INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(0xE8),
                            dwFlags: flags,
                            ..Default::default()
                        },
                    },
                };
                unsafe {
                    SendInput(
                        &[key(Default::default()), key(KEYEVENTF_KEYUP)],
                        size_of::<INPUT>() as i32,
                    )
                };
            }
            let size = (r.right - r.left, r.bottom - r.top);
            let started = std::time::Instant::now();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(10));
                unsafe {
                    let _ = GetCursorPos(&mut p);
                }
                let held = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } as u16 & 0x8000 != 0;
                // Give up after a minute in case the button-up was never seen.
                if !held || started.elapsed().as_secs() > 60 {
                    break;
                }
                unsafe {
                    let _ = SetWindowPos(
                        h,
                        Some(HWND_TOPMOST),
                        p.x - grab.0,
                        p.y - grab.1,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
            }
            let centre = (p.x - grab.0 + size.0 / 2, p.y - grab.1 + size.1 / 2);
            // Dropped on another monitor: the dock moves there.
            let under = monitor_info(unsafe { MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST) });
            let mon = under.as_ref().map(|m| m.1).unwrap_or_else(primary_monitor);
            let (edge, offset) = snap(mon, (p.x, p.y), centre);
            {
                // The orientation may change: the UI measures its tiles again.
                let mut g = DOCK.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(d) = g.as_mut() {
                    if d.settings.edge.vertical() != edge.vertical() {
                        d.content = None;
                    }
                }
            }
            DRAGGING.store(false, SeqCst);
            done(
                edge,
                offset,
                under.map(|m| if m.2 { String::new() } else { m.0 }),
            );
        });
    }

    /// The UI measured its tiles: resize the pill to fit them (ignored when unchanged).
    pub fn fit_content(width: u32) {
        {
            let mut g = DOCK.lock().unwrap_or_else(|e| e.into_inner());
            let Some(d) = g.as_mut() else { return };
            if d.content == Some(width) {
                return;
            }
            d.content = Some(width);
        }
        layout();
    }

    /// New settings from the panel: move and resize the reserved strip right away.
    pub fn update_settings(settings: Settings) {
        {
            let mut g = DOCK.lock().unwrap_or_else(|e| e.into_inner());
            let Some(d) = g.as_mut() else { return };
            if d.settings == settings {
                return;
            }
            // A full-width bar ignores the measured content; a pill measures again after it re-renders.
            if d.settings.shape != settings.shape {
                d.content = None;
            }
            d.settings = settings;
        }
        layout();
        // The dock may have moved to another monitor: hide the taskbar there, show the others.
        crate::taskbar::win::refresh();
    }

    pub fn unregister() {
        let taken = DOCK.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(dock) = taken {
            let mut d = data(hwnd(dock.hwnd));
            unsafe { SHAppBarMessage(ABM_REMOVE, &mut d) };
        }
    }
}

#[cfg(windows)]
pub use win::{drag, fit_content, register, unregister, update_settings};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_chosen_monitor_or_the_main_one() {
        let r = |l| Rect {
            left: l,
            top: 0,
            right: l + 100,
            bottom: 100,
        };
        let list = vec![
            ("A".to_string(), r(0), false),
            ("B".to_string(), r(100), true),
        ];
        assert_eq!(pick_monitor(&list, "A"), Some(r(0)));
        assert_eq!(pick_monitor(&list, ""), Some(r(100)), "empty = main");
        assert_eq!(
            pick_monitor(&list, "gone"),
            Some(r(100)),
            "unplugged = main"
        );
        assert_eq!(pick_monitor(&[], "A"), None);
    }

    const MON: Rect = Rect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };

    #[test]
    fn strips_hug_their_edge() {
        assert_eq!(
            strip(MON, Edge::Bottom, 56, 8),
            Rect {
                left: 0,
                top: 1008,
                right: 1920,
                bottom: 1080
            }
        );
        assert_eq!(
            strip(MON, Edge::Top, 56, 8),
            Rect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 72
            }
        );
        assert_eq!(
            strip(MON, Edge::Left, 56, 8),
            Rect {
                left: 0,
                top: 0,
                right: 72,
                bottom: 1080
            }
        );
        assert_eq!(
            strip(MON, Edge::Right, 56, 8),
            Rect {
                left: 1848,
                top: 0,
                right: 1920,
                bottom: 1080
            }
        );
    }

    #[test]
    fn pill_is_centred_inside_the_strip() {
        let s = strip(MON, Edge::Bottom, 56, 8);
        let p = window_rect(s, Edge::Bottom, Shape::Pill, 0.5, 8, None, 0.5);
        assert_eq!((p.left, p.right, p.top, p.bottom), (480, 1440, 1016, 1072));
        assert_eq!(
            window_rect(s, Edge::Bottom, Shape::Full, 0.5, 8, None, 0.5),
            s
        );
        let hug = window_rect(s, Edge::Bottom, Shape::Pill, 0.9, 8, Some(600), 0.5);
        assert_eq!(
            (hug.left, hug.right),
            (660, 1260),
            "a pill hugs its content, centred"
        );
        assert_eq!(
            window_rect(s, Edge::Bottom, Shape::Pill, 0.5, 8, Some(5000), 0.5).width(),
            960,
            "capped at the max share"
        );
    }

    #[test]
    fn offset_slides_the_pill_but_keeps_it_on_screen() {
        let s = strip(MON, Edge::Bottom, 56, 8);
        let p = window_rect(s, Edge::Bottom, Shape::Pill, 0.9, 8, Some(600), 0.25);
        assert_eq!((p.left, p.right), (180, 780));
        let end = window_rect(s, Edge::Bottom, Shape::Pill, 0.9, 8, Some(600), 1.0);
        assert_eq!(
            (end.left, end.right),
            (1320, 1920),
            "pinned to the right end, not past it"
        );
    }

    #[test]
    fn side_pills_stand_upright() {
        let s = strip(MON, Edge::Left, 56, 8);
        let p = window_rect(s, Edge::Left, Shape::Pill, 0.9, 8, Some(400), 0.5);
        assert_eq!((p.left, p.right, p.top, p.bottom), (8, 64, 340, 740));
        let r = window_rect(
            strip(MON, Edge::Right, 56, 8),
            Edge::Right,
            Shape::Pill,
            0.9,
            8,
            Some(400),
            0.0,
        );
        assert_eq!((r.left, r.right, r.top, r.bottom), (1856, 1912, 0, 400));
    }

    #[test]
    fn drops_land_on_the_nearest_edge() {
        assert_eq!(snap(MON, (960, 1060), (960, 1040)), (Edge::Bottom, 0.5));
        assert_eq!(snap(MON, (20, 500), (40, 270)), (Edge::Left, 0.25));
        assert_eq!(
            snap(MON, (1900, 300), (1880, 540)),
            (Edge::Right, 0.5),
            "near the middle snaps to centred"
        );
        assert_eq!(snap(MON, (480, 5), (480, 30)), (Edge::Top, 0.25));
    }
}
