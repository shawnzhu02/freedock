//! When the dock shows and when it tucks away into its handle, the way the Mac dock's auto-hide works:
//! the pointer pushing the screen edge (or resting on the handle) brings it back, and it slides away
//! a moment after the pointer leaves, as long as the reason to hide still holds.
//!
//! Modes: "always" (reserves its strip, never hides), "smart" (floats over windows and hides while a
//! maximized window is on its monitor), "autohide" (always hidden until the pointer asks for it).

use crate::appbar::Rect;
use crate::settings::Edge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Always,
    Smart,
    Autohide,
}

impl Mode {
    pub fn parse(s: &str) -> Mode {
        match s {
            "always" => Mode::Always,
            "autohide" => Mode::Autohide,
            _ => Mode::Smart,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub shown: bool,
    /// When the pointer started pushing the edge (hidden), or left the dock (shown), in ms.
    since: Option<u64>,
}

impl Default for State {
    fn default() -> Self {
        State {
            shown: true,
            since: None,
        }
    }
}

pub struct Inputs {
    pub cursor: (i32, i32),
    /// The dock window and its monitor, physical px.
    pub dock: Rect,
    pub mon: Rect,
    pub edge: Edge,
    pub mode: Mode,
    pub maximized: bool,
    /// Start is open, the settings panel is open, or the dock is being edited: stay shown.
    pub hold: bool,
    pub now: u64,
    /// ms the pointer must stay in the reveal zone before the dock shows.
    pub show_delay: u64,
    /// How deep from the screen edge the reveal zone reaches, physical px (it covers the handle).
    pub show_distance: i32,
    /// px of slack around the dock before the pointer counts as gone (scaled for DPI by the caller).
    pub hide_distance: i32,
    /// ms the pointer must stay away before the dock hides.
    pub hide_delay: u64,
}

fn inside(r: Rect, (x, y): (i32, i32), pad: i32) -> bool {
    x >= r.left - pad && x < r.right + pad && y >= r.top - pad && y < r.bottom + pad
}

/// The strip along the screen edge, as wide as the dock (plus a little), that brings it back. It
/// reaches as far past the edge as inside it: on an edge shared with another monitor the pointer
/// slides on across, and would otherwise be in the strip for only a moment.
fn reveal_zone(i: &Inputs) -> Rect {
    let along = 40;
    let d = i.show_distance.max(1);
    match i.edge {
        Edge::Bottom => Rect {
            left: i.dock.left - along,
            right: i.dock.right + along,
            top: i.mon.bottom - d,
            bottom: i.mon.bottom + d,
        },
        Edge::Top => Rect {
            left: i.dock.left - along,
            right: i.dock.right + along,
            top: i.mon.top - d,
            bottom: i.mon.top + d,
        },
        Edge::Left => Rect {
            left: i.mon.left - d,
            right: i.mon.left + d,
            top: i.dock.top - along,
            bottom: i.dock.bottom + along,
        },
        Edge::Right => Rect {
            left: i.mon.right - d,
            right: i.mon.right + d,
            top: i.dock.top - along,
            bottom: i.dock.bottom + along,
        },
    }
}

pub fn next(s: State, i: &Inputs) -> State {
    let want_hidden = match i.mode {
        Mode::Always => false,
        Mode::Smart => i.maximized,
        Mode::Autohide => true,
    };
    if !want_hidden || i.hold {
        return State {
            shown: true,
            since: None,
        };
    }
    if s.shown {
        if inside(i.dock, i.cursor, i.hide_distance) {
            return State {
                shown: true,
                since: None,
            };
        }
        let since = s.since.unwrap_or(i.now);
        if i.now.saturating_sub(since) >= i.hide_delay {
            return State {
                shown: false,
                since: None,
            };
        }
        return State {
            shown: true,
            since: Some(since),
        };
    }
    if inside(reveal_zone(i), i.cursor, 0) {
        let since = s.since.unwrap_or(i.now);
        if i.now.saturating_sub(since) >= i.show_delay {
            return State {
                shown: true,
                since: None,
            };
        }
        return State {
            shown: false,
            since: Some(since),
        };
    }
    State {
        shown: false,
        since: None,
    }
}

#[cfg(windows)]
pub mod win {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use tauri::{Emitter, Manager};
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetCursorPos, GetWindowRect, IsIconic, IsWindowVisible, IsZoomed,
    };
    use windows::core::BOOL;

    use super::*;

    /// The settings panel is open: keep the dock shown.
    pub static HOLD: AtomicBool = AtomicBool::new(false);
    /// The dock is in move mode: keep it shown.
    pub static EDIT: AtomicBool = AtomicBool::new(false);
    /// An agent popup (Quick Ask, Claude, a widget flyout) is open: keep it shown.
    pub static POPUP: AtomicBool = AtomicBool::new(false);
    static SHOWN: AtomicBool = AtomicBool::new(true);

    pub fn shown() -> bool {
        SHOWN.load(Ordering::SeqCst)
    }

    fn rect(r: RECT) -> Rect {
        Rect {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }

    /// Is a maximized app window on the dock's monitor? Only windows that belong in a taskbar count
    /// (the apps.rs filter), and minimized ones don't.
    fn maximized_on(dock: HWND) -> bool {
        struct Ctx {
            mon: windows::Win32::Graphics::Gdi::HMONITOR,
            me: u32,
            found: bool,
        }
        unsafe extern "system" fn each(h: HWND, lp: LPARAM) -> BOOL {
            let c = unsafe { &mut *(lp.0 as *mut Ctx) };
            unsafe {
                if IsWindowVisible(h).as_bool()
                    && IsZoomed(h).as_bool()
                    && !IsIconic(h).as_bool()
                    && MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST) == c.mon
                    && crate::apps::win::is_app_window(h, c.me)
                {
                    c.found = true;
                    return BOOL(0);
                }
            }
            BOOL(1)
        }
        let mut c = Ctx {
            mon: unsafe { MonitorFromWindow(dock, MONITOR_DEFAULTTONEAREST) },
            me: std::process::id(),
            found: false,
        };
        unsafe {
            let _ = EnumWindows(Some(each), LPARAM(&mut c as *mut _ as isize));
        }
        c.found
    }

    /// Runs for the life of the dock. While the dock can hide (or is hidden) it checks the pointer 50
    /// times a second; while nothing can hide it (Always, or Smart with nothing maximized) it wakes only
    /// 4 times a second for the maximized-window check. The taskbar check runs once a second.
    pub fn start(app: tauri::AppHandle) {
        std::thread::spawn(move || {
            let t0 = Instant::now();
            let mut state = State::default();
            let mut maximized = false;
            let mut idle = false;
            let mut last_enforce = Instant::now();
            let mut last_scan: Option<Instant> = None;
            loop {
                std::thread::sleep(Duration::from_millis(if idle { 240 } else { 20 }));
                if last_enforce.elapsed() >= Duration::from_secs(1) {
                    crate::taskbar::win::enforce();
                    last_enforce = Instant::now();
                }
                let s = crate::appbar::with_settings(|raw, s| {
                    (
                        raw,
                        Mode::parse(&s.visibility),
                        s.edge,
                        (
                            s.show_delay,
                            s.show_distance.clamp(1, 300),
                            s.hide_distance.min(400),
                            s.hide_delay.min(5000),
                        ),
                    )
                });
                let Some((raw, mode, edge, (show_delay, show_distance, hide_distance, hide_delay))) =
                    s
                else {
                    continue;
                };
                let h = HWND(raw as *mut _);
                if mode == Mode::Smart
                    && last_scan.is_none_or(|t| t.elapsed() >= Duration::from_millis(240))
                {
                    maximized = maximized_on(h);
                    last_scan = Some(Instant::now());
                }
                // Nothing can hide the dock right now (the usual case): skip the pointer and monitor
                // lookups, and sleep longer, until a maximized window or another mode changes that.
                idle = state.shown && (mode == Mode::Always || (mode == Mode::Smart && !maximized));
                if idle {
                    continue;
                }
                let mut pt = POINT::default();
                let mut wr = RECT::default();
                let mut mi = MONITORINFO {
                    cbSize: size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                unsafe {
                    let _ = GetCursorPos(&mut pt);
                    let _ = GetWindowRect(h, &mut wr);
                    let _ =
                        GetMonitorInfoW(MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST), &mut mi);
                }
                let scale = unsafe { GetDpiForWindow(h) }.max(96) as f64 / 96.0;
                let inputs = Inputs {
                    cursor: (pt.x, pt.y),
                    dock: rect(wr),
                    mon: rect(mi.rcMonitor),
                    edge,
                    mode,
                    maximized,
                    hold: HOLD.load(Ordering::SeqCst)
                        || EDIT.load(Ordering::SeqCst)
                        || POPUP.load(Ordering::SeqCst)
                        || crate::apps::win::START_OPEN.load(Ordering::SeqCst),
                    now: t0.elapsed().as_millis() as u64,
                    show_delay: show_delay.min(2000) as u64,
                    show_distance: (show_distance as f64 * scale).round() as i32,
                    hide_distance: (hide_distance as f64 * scale).round() as i32,
                    hide_delay: hide_delay as u64,
                };
                let was = state.shown;
                state = next(state, &inputs);
                if state.shown != was {
                    SHOWN.store(state.shown, Ordering::SeqCst);
                    if let Some(w) = app.get_webview_window("dock") {
                        // Hidden, clicks pass through to the window underneath; the pointer is watched here.
                        let _ = w.set_ignore_cursor_events(!state.shown);
                    }
                    let _ = app.emit("dock-visibility", state.shown);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MON: Rect = Rect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    const DOCK: Rect = Rect {
        left: 600,
        top: 1010,
        right: 1320,
        bottom: 1074,
    };

    fn at(cursor: (i32, i32), now: u64) -> Inputs {
        Inputs {
            cursor,
            dock: DOCK,
            mon: MON,
            edge: Edge::Bottom,
            mode: Mode::Smart,
            maximized: true,
            hold: false,
            now,
            show_delay: 150,
            show_distance: 10,
            hide_distance: 12,
            hide_delay: 150,
        }
    }

    fn run(mut s: State, steps: &[Inputs]) -> State {
        for i in steps {
            s = next(s, i);
        }
        s
    }

    #[test]
    fn always_mode_and_no_maximized_window_stay_shown() {
        let hidden = State {
            shown: false,
            since: None,
        };
        assert!(
            next(
                hidden,
                &Inputs {
                    mode: Mode::Always,
                    ..at((10, 10), 0)
                }
            )
            .shown
        );
        assert!(
            next(
                hidden,
                &Inputs {
                    maximized: false,
                    ..at((10, 10), 0)
                }
            )
            .shown,
            "smart: nothing maximized"
        );
    }

    #[test]
    fn hides_after_the_pointer_leaves() {
        let s = run(State::default(), &[at((10, 10), 0), at((10, 10), 100)]);
        assert!(s.shown, "not yet: 100 ms");
        assert!(
            !run(s, &[at((10, 10), 160)]).shown,
            "gone after 150 ms away"
        );
        let back = run(
            State::default(),
            &[at((10, 10), 0), at((900, 1040), 300), at((10, 10), 500)],
        );
        assert!(back.shown, "coming back over the dock restarts the clock");
    }

    #[test]
    fn edge_push_reveals_after_the_delay() {
        let hidden = State {
            shown: false,
            since: None,
        };
        let s = run(hidden, &[at((900, 1079), 0), at((900, 1079), 100)]);
        assert!(!s.shown, "100 ms is under the 150 ms delay");
        assert!(run(s, &[at((900, 1079), 160)]).shown);
        assert!(
            !run(hidden, &[at((100, 1079), 0), at((100, 1079), 500)]).shown,
            "edge far from the dock does nothing"
        );
    }

    #[test]
    fn hold_keeps_it_shown() {
        let s = run(
            State::default(),
            &[
                Inputs {
                    hold: true,
                    ..at((10, 10), 0)
                },
                Inputs {
                    hold: true,
                    ..at((10, 10), 900)
                },
            ],
        );
        assert!(s.shown, "settings panel or Start open");
    }

    #[test]
    fn top_edge_zone() {
        let hidden = State {
            shown: false,
            since: None,
        };
        let top = |now| Inputs {
            edge: Edge::Top,
            dock: Rect {
                left: 600,
                top: 6,
                right: 1320,
                bottom: 70,
            },
            mode: Mode::Autohide,
            ..at((900, 0), now)
        };
        assert!(run(hidden, &[top(0), top(200)]).shown);
    }

    #[test]
    fn show_distance_reaches_further_from_the_edge() {
        let hidden = State {
            shown: false,
            since: None,
        };
        // 60 px above the bottom edge: outside a 10 px zone, inside 75 and 100 px ones.
        let p = |d, now| Inputs {
            show_distance: d,
            ..at((900, 1020), now)
        };
        assert!(!run(hidden, &[p(10, 0), p(10, 400)]).shown, "10 px zone");
        assert!(run(hidden, &[p(75, 0), p(75, 400)]).shown, "75 px zone");
        assert!(run(hidden, &[p(100, 0), p(100, 400)]).shown, "100 px zone");
        // Every edge honours it.
        let top = |now| Inputs {
            edge: Edge::Top,
            dock: Rect {
                left: 600,
                top: 6,
                right: 1320,
                bottom: 70,
            },
            mode: Mode::Autohide,
            show_distance: 50,
            ..at((900, 45), now)
        };
        assert!(run(hidden, &[top(0), top(200)]).shown, "top, 50 px");
        let right = |now| Inputs {
            edge: Edge::Right,
            dock: Rect {
                left: 1850,
                top: 300,
                right: 1914,
                bottom: 780,
            },
            mode: Mode::Autohide,
            show_distance: 50,
            ..at((1880, 500), now)
        };
        assert!(run(hidden, &[right(0), right(200)]).shown, "right, 50 px");
        let left = |now| Inputs {
            edge: Edge::Left,
            dock: Rect {
                left: 6,
                top: 300,
                right: 70,
                bottom: 780,
            },
            mode: Mode::Autohide,
            show_distance: 50,
            ..at((60, 500), now)
        };
        assert!(
            !run(hidden, &[left(0), left(200)]).shown,
            "left, 60 px is past a 50 px zone"
        );
    }

    #[test]
    fn hide_distance_and_delay() {
        // 40 px above the dock's top: inside a 50 px slack, outside the default 12.
        let p = |slack, now| Inputs {
            hide_distance: slack,
            ..at((900, 970), now)
        };
        assert!(
            run(State::default(), &[p(50, 0), p(50, 1000)]).shown,
            "50 px slack keeps it"
        );
        assert!(
            !run(State::default(), &[p(12, 0), p(12, 1000)]).shown,
            "12 px slack lets it go"
        );
        let slow = |now| Inputs {
            hide_delay: 800,
            ..at((10, 10), now)
        };
        assert!(
            run(State::default(), &[slow(0), slow(700)]).shown,
            "800 ms delay: still shown at 700"
        );
        assert!(
            !run(State::default(), &[slow(0), slow(700), slow(810)]).shown,
            "gone after 800 ms"
        );
        let quick = |now| Inputs {
            show_delay: 0,
            ..at((900, 1079), now)
        };
        assert!(
            next(
                State {
                    shown: false,
                    since: None
                },
                &quick(0)
            )
            .shown,
            "0 ms show delay is instant"
        );
    }

    #[test]
    fn side_edge_zone() {
        let hidden = State {
            shown: false,
            since: None,
        };
        let left = |cursor, now| Inputs {
            edge: Edge::Left,
            dock: Rect {
                left: 6,
                top: 300,
                right: 70,
                bottom: 780,
            },
            mode: Mode::Autohide,
            ..at(cursor, now)
        };
        assert!(run(hidden, &[left((0, 500), 0), left((0, 500), 200)]).shown);
        assert!(
            !run(hidden, &[left((0, 100), 0), left((0, 100), 200)]).shown,
            "above the dock's span"
        );
    }
}
