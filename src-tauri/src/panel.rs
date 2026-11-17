//! The settings panel: a small popup window above the dock's right end. It opens from the tray icon,
//! a right-click on the dock, the global shortcut, or launching freedock again, and hides when it loses focus.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::settings::Edge;
use tauri::{
    AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

pub const LABEL: &str = "settings";
const WIDTH: f64 = 460.0;
const HEIGHT: f64 = 620.0;

/// When the panel last hid because it lost focus. Clicking the tray icon steals focus first, so without this
/// the click that should close the panel would hide it and then open it again.
static BLUR_HIDDEN: Mutex<Option<Instant>> = Mutex::new(None);
/// Whether the panel has had focus since it was last shown. A brand-new WebView2 window reports a blur
/// while it starts up; hiding on that would close the panel the moment it opens.
static HAD_FOCUS: AtomicBool = AtomicBool::new(false);
/// Bumped on every show and hide; a pending "close the hidden panel" only runs if nothing happened since.
static GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// The panel is rarely open, so its window (and WebView2 renderer) is closed this long after it hides.
const KEEP_HIDDEN: Duration = Duration::from_secs(60);

/// After a hide: tell WebView2 to trim, and close the window if it stays hidden for a minute.
fn retire(w: &WebviewWindow) {
    crate::webmem::set_low(w, true);
    let g = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let w = w.clone();
    std::thread::spawn(move || {
        std::thread::sleep(KEEP_HIDDEN);
        if GEN.load(Ordering::SeqCst) == g && !w.is_visible().unwrap_or(true) {
            let _ = w.destroy();
        }
    });
}

/// Where the panel goes: beside the dock on the screen side, right-aligned with a bottom or top dock and
/// bottom-aligned with a side dock, kept on the monitor. All values in physical pixels.
pub fn place(
    dock: (i32, i32, i32, i32),
    mon: (i32, i32, i32, i32),
    panel: (i32, i32),
    gap: i32,
    edge: Edge,
) -> (i32, i32) {
    let (dl, dt, dr, db) = dock;
    let (ml, mt, mr, mb) = mon;
    let (w, h) = panel;
    let (x, y) = match edge {
        Edge::Bottom => (dr - w, dt - h - gap),
        Edge::Top => (dr - w, db + gap),
        Edge::Left => (dr + gap, db - h),
        Edge::Right => (dl - w - gap, db - h),
    };
    (
        x.clamp(ml + gap, (mr - w - gap).max(ml + gap)),
        y.clamp(mt + gap, (mb - h - gap).max(mt + gap)),
    )
}

fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let w = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html#settings".into()))
        .title("freedock settings")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()?;
    let me = w.clone();
    w.on_window_event(move |e| match e {
        WindowEvent::Focused(true) => HAD_FOCUS.store(true, Ordering::SeqCst),
        WindowEvent::Focused(false) if HAD_FOCUS.swap(false, Ordering::SeqCst) => {
            *BLUR_HIDDEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
            #[cfg(windows)]
            crate::visibility::win::HOLD.store(false, Ordering::SeqCst);
            let _ = me.hide();
            retire(&me);
        }
        _ => {}
    });
    Ok(w)
}

fn position(app: &AppHandle, w: &WebviewWindow) {
    let Some(dock) = app.get_webview_window("dock") else {
        return;
    };
    let (Ok(p), Ok(s), Ok(Some(m))) = (
        dock.outer_position(),
        dock.outer_size(),
        dock.current_monitor(),
    ) else {
        return;
    };
    let scale = m.scale_factor();
    let (mp, ms) = (m.position(), m.size());
    let edge = crate::settings::load().edge;
    let (x, y) = place(
        (p.x, p.y, p.x + s.width as i32, p.y + s.height as i32),
        (mp.x, mp.y, mp.x + ms.width as i32, mp.y + ms.height as i32),
        ((WIDTH * scale) as i32, (HEIGHT * scale) as i32),
        (8.0 * scale) as i32,
        edge,
    );
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

/// Show the panel, building its window first if it was closed. Building happens on its own thread:
/// creating a WebView2 window while a command holds the main thread would deadlock.
pub fn open(app: &AppHandle) {
    match app.get_webview_window(LABEL) {
        Some(w) => show(app, &w),
        None => {
            let app = app.clone();
            std::thread::spawn(move || match create(&app) {
                Ok(w) => show(&app, &w),
                Err(e) => eprintln!("settings panel: {e}"),
            });
        }
    }
}

fn show(app: &AppHandle, w: &WebviewWindow) {
    GEN.fetch_add(1, Ordering::SeqCst);
    crate::webmem::set_low(w, false);
    position(app, w);
    HAD_FOCUS.store(false, Ordering::SeqCst);
    #[cfg(windows)]
    crate::visibility::win::HOLD.store(true, Ordering::SeqCst);
    let _ = w.show();
    let _ = w.set_focus();
    // The dock never takes focus, so Windows may refuse to bring the panel forward; push it.
    #[cfg(windows)]
    if let Ok(h) = w.hwnd() {
        crate::apps::win::focus(h.0 as i64);
    }
}

pub fn hide(app: &AppHandle) {
    #[cfg(windows)]
    crate::visibility::win::HOLD.store(false, Ordering::SeqCst);
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
        retire(&w);
    }
}

pub fn toggle(app: &AppHandle) {
    let visible = app
        .get_webview_window(LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    let just_blurred = BLUR_HIDDEN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some_and(|t| t.elapsed() < Duration::from_millis(350));
    if visible || just_blurred {
        hide(app)
    } else {
        open(app)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MON: (i32, i32, i32, i32) = (0, 0, 1920, 1080);

    #[test]
    fn sits_above_a_bottom_dock_right_aligned() {
        assert_eq!(
            place((200, 1010, 1720, 1074), MON, (460, 620), 8, Edge::Bottom),
            (1260, 382)
        );
    }

    #[test]
    fn sits_below_a_top_dock() {
        assert_eq!(
            place((200, 6, 1720, 60), MON, (460, 620), 8, Edge::Top),
            (1260, 68)
        );
    }

    #[test]
    fn sits_beside_a_side_dock() {
        assert_eq!(
            place((6, 300, 70, 780), MON, (460, 620), 8, Edge::Left),
            (78, 160)
        );
        assert_eq!(
            place((1850, 300, 1914, 780), MON, (460, 620), 8, Edge::Right),
            (1382, 160)
        );
    }

    #[test]
    fn stays_on_the_monitor() {
        let (x, _) = place((0, 1010, 300, 1074), MON, (460, 620), 8, Edge::Bottom);
        assert_eq!(x, 8, "a dock narrower than the panel at the left edge");
        let (x, _) = place((1500, 1010, 1930, 1074), MON, (460, 620), 8, Edge::Bottom);
        assert_eq!(x, 1920 - 460 - 8);
    }
}
