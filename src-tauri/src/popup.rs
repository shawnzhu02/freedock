//! The flyout: one window above the dock that shows Quick Ask, the Claude session launcher, the MCP
//! agent or a widget's flyout. It opens from the dock's cells or the hotkeys and hides when it loses
//! focus (except while a tray app's own menu is open).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

use crate::settings::Edge;

const LABEL: &str = "ask";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Mode {
    /// "ask", "claude", "agent", or one of the widget flyouts.
    pub mode: String,
    /// Text to start Quick Ask with (the selection, for the selection hotkey).
    pub prefill: String,
}

static MODE: Mutex<Option<Mode>> = Mutex::new(None);
static HAD_FOCUS: AtomicBool = AtomicBool::new(false);
static BLUR_HIDDEN: Mutex<Option<Instant>> = Mutex::new(None);
/// Set just before a click is passed to a tray app: if that app's menu takes focus, the popup (and the
/// dock under it) stays up while the menu is open, instead of closing on the blur.
static MENU_GRACE: Mutex<Option<Instant>> = Mutex::new(None);

/// Called before a tray click goes out: the next blur, within 3 s, is probably the app's own menu.
pub fn expect_app_menu() {
    *MENU_GRACE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
}

fn in_menu_grace() -> bool {
    MENU_GRACE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .is_some_and(|t| t.elapsed() < Duration::from_secs(3))
}

/// Hides the popup window. Hover previews are shown with a direct `ShowWindow` (so they don't take
/// focus), which Tauri doesn't know about: it still thinks the window is hidden, and its own `hide()`
/// then does nothing. So this also hides it directly.
fn hide_window(win: &WebviewWindow) {
    let _ = win.hide();
    #[cfg(windows)]
    if let Ok(h) = win.hwnd() {
        use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
        unsafe {
            let _ = ShowWindow(h, SW_HIDE);
        }
    }
}

/// Close the popup the way a blur does.
fn blur_close(win: &WebviewWindow) {
    log("blur close");
    *BLUR_HIDDEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    hidden(win.app_handle());
    hide_window(win);
}

/// True while `watch_app_menu` keeps the popup up for a tray app's menu: clicks in that menu must not
/// close it.
static IN_APP_MENU: AtomicBool = AtomicBool::new(false);
/// Bumped on every open, so an older outside-click watcher knows to stop.
static OPEN_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Closes the flyout when the user clicks anywhere outside it and the dock. Normally losing focus does
/// that, but Windows sometimes refuses to give the flyout focus (another app holds the foreground),
/// and then nothing would close it. Polls the mouse buttons only while the flyout is open.
#[cfg(windows)]
fn watch_outside_clicks(win: WebviewWindow) {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect};
    let g = OPEN_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let dock = win
        .app_handle()
        .get_webview_window("dock")
        .and_then(|d| d.hwnd().ok())
        .map(|h| h.0 as isize);
    let Ok(me) = win.hwnd().map(|h| h.0 as isize) else {
        return;
    };
    std::thread::spawn(move || {
        let down = || unsafe {
            [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON]
                .iter()
                .any(|k| GetAsyncKeyState(k.0 as i32) as u16 & 0x8000 != 0)
        };
        let inside = |h: isize, p: POINT| {
            let mut r = RECT::default();
            unsafe { GetWindowRect(HWND(h as *mut _), &mut r) }.is_ok()
                && p.x >= r.left
                && p.x < r.right
                && p.y >= r.top
                && p.y < r.bottom
        };
        // The press that opened the flyout may still be held: only a new press counts.
        let mut was = down();
        loop {
            std::thread::sleep(Duration::from_millis(40));
            if OPEN_GEN.load(Ordering::SeqCst) != g || !win.is_visible().unwrap_or(false) {
                return;
            }
            let now = down();
            if now && !was && !IN_APP_MENU.load(Ordering::SeqCst) {
                let mut p = POINT::default();
                let _ = unsafe { GetCursorPos(&mut p) };
                if !inside(me, p) && !dock.is_some_and(|d| inside(d, p)) {
                    blur_close(&win);
                    return;
                }
            }
            was = now;
        }
    });
}

/// Bumped on every show, so a late fallback doesn't uncloak a later open early.
static SHOW_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Hides or reveals the popup's pixels without hiding the window (DWM cloaking). The window is shown
/// cloaked, so the webview can paint the new flyout first; otherwise the old flyout's last frame
/// flashes before the new one appears.
#[cfg(windows)]
fn cloak(win: &WebviewWindow, on: bool) {
    use windows::Win32::Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute};
    CLOAKED.store(on, Ordering::SeqCst);
    if let Ok(h) = win.hwnd() {
        let v: i32 = on as i32;
        unsafe {
            let _ = DwmSetWindowAttribute(
                windows::Win32::Foundation::HWND(h.0),
                DWMWA_CLOAK,
                &v as *const i32 as *const _,
                4,
            );
        }
    }
}

/// Cloaks the popup before it is shown, and uncloaks it when the view says it has painted
/// (`ready`), or after 300 ms if it never does.
#[cfg(windows)]
fn cloak_until_ready(win: &WebviewWindow) {
    cloak(win, true);
    let g = SHOW_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let w = win.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        if SHOW_GEN.load(Ordering::SeqCst) == g {
            reveal(&w, "fallback");
        }
    });
}

/// True while the popup is cloaked (opening): the watchdog leaves it alone then.
static CLOAKED: AtomicBool = AtomicBool::new(false);

/// Uncloak the popup. `why` (the page said it painted, or the fallback) goes to popup.log.
#[cfg(windows)]
fn reveal(win: &WebviewWindow, why: &str) {
    log(&format!("reveal ({why})"));
    cloak(win, false);
    // The page finishes its open animation on this too, in case its own timing never ran.
    let _ = win.emit_to(LABEL, "popup-revealed", ());
}

/// The view painted the flyout it was asked for: show it.
pub fn ready(app: &AppHandle) {
    #[cfg(windows)]
    if let Some(w) = app.get_webview_window(LABEL) {
        SHOW_GEN.fetch_add(1, Ordering::SeqCst);
        reveal(&w, "ready");
    }
}

/// Runs `f` when dropped (the end of a scope, however it ends).
fn scopeguard(f: impl FnOnce()) -> impl Drop {
    struct G<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for G<F> {
        fn drop(&mut self) {
            if let Some(f) = self.0.take() {
                f();
            }
        }
    }
    G(Some(f))
}

/// Whether the front window looks like a tray app's menu (rather than a window the app opened).
/// Classic menus are found separately (class `#32768`). Apps that draw their own menus use small
/// caption-less popups that are tool windows or always-on-top. Frameless main windows (Electron apps
/// like Discord or Spotify) are also caption-less popups, but they are big and neither, and counting
/// them kept the flyout open over the app the click just opened.
fn menu_like(style: u32, exstyle: u32, w: i32, h: i32, mon_w: i32, mon_h: i32) -> bool {
    const WS_POPUP: u32 = 0x8000_0000;
    const WS_CAPTION: u32 = 0x00C0_0000;
    const WS_EX_TOPMOST: u32 = 0x0000_0008;
    const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
    style & WS_POPUP != 0
        && style & WS_CAPTION != WS_CAPTION
        && exstyle & (WS_EX_TOOLWINDOW | WS_EX_TOPMOST) != 0
        && w * 2 <= mon_w
        && h * 2 <= mon_h
}

/// A tray app's menu has focus: keep the popup until the user is done with the menu. It closes when a
/// normal window comes to the front (they picked something, or clicked elsewhere), and stops watching
/// if the popup itself gets focus back.
#[cfg(windows)]
fn watch_app_menu(win: WebviewWindow) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetForegroundWindow, GetWindowLongW, GetWindowRect,
        IsWindowVisible,
    };
    IN_APP_MENU.store(true, Ordering::SeqCst);
    std::thread::spawn(move || {
        let start = Instant::now();
        let _done = scopeguard(|| IN_APP_MENU.store(false, Ordering::SeqCst));
        loop {
            std::thread::sleep(Duration::from_millis(150));
            if HAD_FOCUS.load(Ordering::SeqCst) || !win.is_visible().unwrap_or(false) {
                return;
            }
            let in_menu = unsafe {
                // A classic menu (#32768) is up, or the front window is invisible (the app's hidden
                // owner window holds focus while its menu shows), or it looks like the app's own
                // drawn menu (see `menu_like`).
                let classic = FindWindowW(windows::core::w!("#32768"), None)
                    .is_ok_and(|h| IsWindowVisible(h).as_bool());
                let fg = GetForegroundWindow();
                classic || fg.0.is_null() || !IsWindowVisible(fg).as_bool() || {
                    let mut r = RECT::default();
                    let _ = GetWindowRect(fg, &mut r);
                    let mut mi = MONITORINFO {
                        cbSize: size_of::<MONITORINFO>() as u32,
                        ..Default::default()
                    };
                    let _ =
                        GetMonitorInfoW(MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST), &mut mi);
                    let m = mi.rcMonitor;
                    menu_like(
                        GetWindowLongW(fg, GWL_STYLE) as u32,
                        GetWindowLongW(fg, GWL_EXSTYLE) as u32,
                        r.right - r.left,
                        r.bottom - r.top,
                        m.right - m.left,
                        m.bottom - m.top,
                    )
                }
            };
            if !in_menu || start.elapsed() > Duration::from_secs(60) {
                blur_close(&win);
                return;
            }
        }
    });
}
/// Where along the dock's edge the popup was last opened (physical px), so a resize keeps its place.
static ANCHOR: Mutex<Option<i32>> = Mutex::new(None);

/// Tallest a flyout may grow to fit its content, in logical px.
pub const FIT_MAX: f64 = 640.0;

/// Logical size per mode.
pub fn size_for(mode: &str) -> (f64, f64) {
    match mode {
        "tray" => tray_size(tray_count()),
        "quick" => (360.0, 300.0),
        "calendar" => (340.0, 460.0),
        "media" => (300.0, 420.0),
        "usage" => (320.0, 240.0),
        "stats" => (320.0, 700.0),
        "weather" => (360.0, 520.0),
        "projects" => (420.0, 460.0),
        "clipboard" => (400.0, 460.0),
        "focus" => (320.0, 360.0),
        "capture" => (360.0, 200.0),
        "downloads" => (380.0, 460.0),
        "screenshots" => (420.0, 460.0),
        "recent" => (380.0, 460.0),
        "notes" => (380.0, 420.0),
        "github" => (400.0, 480.0),
        "docker" => (380.0, 420.0),
        "power" => (300.0, 260.0),
        "todos" => (360.0, 460.0),
        "sites" => (340.0, 360.0),
        "calc" => (320.0, 420.0),
        "moods" => (260.0, 300.0),
        "start" => (560.0, 640.0),
        "appmenu" => (280.0, 320.0),
        "sessions" => (340.0, 400.0),
        "screentime" => (360.0, 480.0),
        "transit" => (340.0, 360.0),
        "stocks" => (360.0, 420.0),
        "currency" => (300.0, 260.0),
        "news" => (400.0, 480.0),
        "claude" => (360.0, 440.0),
        "agent" => (420.0, 540.0),
        _ => (620.0, 480.0),
    }
}

/// Background apps' icons in the tray flyout, five to a row like Windows' own overflow.
pub const TRAY_COLS: usize = 5;

/// The tray flyout's size for `n` icons: 32 px buttons, 8 px apart, 9 px padding plus the border.
pub fn tray_size(n: usize) -> (f64, f64) {
    if n == 0 {
        return (200.0, 60.0);
    }
    let cols = n.min(TRAY_COLS);
    let rows = n.div_ceil(TRAY_COLS);
    ((cols * 40 + 12) as f64, (rows * 40 + 12) as f64)
}

fn tray_count() -> usize {
    #[cfg(windows)]
    return crate::tray::win::list().len();
    #[cfg(not(windows))]
    0
}

/// Where the popup goes, in physical px: beside the dock on the screen side, centred on `anchor` (a
/// point along the dock's edge, e.g. the cell that opened it), kept on the monitor.
pub fn place(
    dock: (i32, i32, i32, i32),
    mon: (i32, i32, i32, i32),
    size: (i32, i32),
    gap: i32,
    edge: Edge,
    anchor: i32,
) -> (i32, i32) {
    let (dl, dt, dr, db) = dock;
    let (ml, mt, mr, mb) = mon;
    let (w, h) = size;
    let (x, y) = match edge {
        Edge::Bottom => (anchor - w / 2, dt - h - gap),
        Edge::Top => (anchor - w / 2, db + gap),
        Edge::Left => (dr + gap, anchor - h / 2),
        Edge::Right => (dl - w - gap, anchor - h / 2),
    };
    (
        x.clamp(ml + gap, (mr - w - gap).max(ml + gap)),
        y.clamp(mt + gap, (mb - h - gap).max(mt + gap)),
    )
}

fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let (w, h) = size_for("ask");
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html#ask".into()))
        .title("freedock")
        .inner_size(w, h)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()?;
    // No fade or zoom when it opens and closes: Windows' own flyouts appear and go at once, and a
    // fading window shows every last-moment change (it looked like the card jumped on close).
    #[cfg(windows)]
    if let Ok(h) = win.hwnd() {
        use windows::Win32::Graphics::Dwm::{
            DWMWA_TRANSITIONS_FORCEDISABLED, DwmSetWindowAttribute,
        };
        let on: i32 = 1;
        unsafe {
            let _ = DwmSetWindowAttribute(
                windows::Win32::Foundation::HWND(h.0),
                DWMWA_TRANSITIONS_FORCEDISABLED,
                &on as *const i32 as *const _,
                4,
            );
        }
    }
    let me = win.clone();
    win.on_window_event(move |e| {
        on_event(&me, e)
    });
    Ok(win)
}

fn on_event(me: &WebviewWindow, e: &WindowEvent) {
    match e {
        WindowEvent::Focused(true) => HAD_FOCUS.store(true, Ordering::SeqCst),
        WindowEvent::Focused(false) if HAD_FOCUS.swap(false, Ordering::SeqCst) => {
            #[cfg(windows)]
            if in_menu_grace() {
                watch_app_menu(me.clone());
                return;
            }
            blur_close(me);
        }
        _ => {}
    }
}

/// Build the (hidden) popup at startup; building windows from callbacks later stalls on Windows.
pub fn prepare(app: &AppHandle) {
    if app.get_webview_window(LABEL).is_none() {
        if let Err(e) = create(app) {
            eprintln!("popup: {e}");
        }
    }
    #[cfg(windows)]
    watchdog(app.clone());
}

/// Appends one line to `%APPDATA%\freedock\popup.log` (kept under 64 KB), so a flyout that got
/// stuck can be traced afterwards. Written: opens, the page saying it is ready, reveals, closes,
/// safety-net closes and the self-test.
fn log(what: &str) {
    use std::io::Write;
    let path = crate::settings::dir().join("popup.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 64 * 1024) {
        let _ = std::fs::rename(&path, path.with_extension("log.old"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = writeln!(f, "{t} {what}");
    }
}

/// A step of the page's open path, for popup.log (see `selftest`).
pub fn trace(msg: &str) {
    log(&format!("page {msg}"));
}

/// Opens a few flyouts one after another, the way a click would, and logs whether each appeared and
/// stayed open (see popup.log). Started by creating an empty `selftest` file in the settings folder;
/// the watchdog picks it up. Used to check on a real desktop that flyouts appear.
#[cfg(windows)]
fn selftest(app: AppHandle) {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    std::thread::spawn(move || {
        let before = unsafe { GetForegroundWindow() }.0 as i64;
        log("selftest start");
        for mode in ["quick", "calendar", "start", "tray", "power", "quick"] {
            let a = app.clone();
            let _ = app.run_on_main_thread(move || open(&a, mode, String::new(), None));
            std::thread::sleep(Duration::from_millis(1500));
            let win = app.get_webview_window(LABEL);
            let up = win
                .as_ref()
                .and_then(|w| w.is_visible().ok())
                .unwrap_or(false);
            let me = win.and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize);
            let fg = unsafe { GetForegroundWindow() }.0 as isize;
            log(&format!(
                "selftest {mode}: up after 1.5 s={up} focused={} cloaked={}",
                Some(fg) == me,
                CLOAKED.load(Ordering::SeqCst)
            ));
            let a = app.clone();
            let _ = app.run_on_main_thread(move || hide(&a));
            std::thread::sleep(Duration::from_millis(400));
        }
        crate::apps::win::focus(before);
        log("selftest end");
    });
}

/// Safety net for every way a flyout could be left behind. The normal paths (losing focus, the
/// outside-click watcher, the preview's own pointer watcher) should close it first; this catches
/// whatever slips past them:
/// - a flyout that is up while another window is in front (and no tray app menu is open) for 0.4 s;
/// - a hover preview whose pointer has been away from it and the dock for 0.6 s.
/// It looks at the window directly every 100 ms and does nothing while the flyout is closed.
#[cfg(windows)]
fn watchdog(app: AppHandle) {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetForegroundWindow, GetWindowRect, IsWindowVisible,
    };
    let (Some(me), dock) = (
        app.get_webview_window(LABEL)
            .and_then(|w| w.hwnd().ok())
            .map(|h| h.0 as isize),
        app.get_webview_window("dock")
            .and_then(|w| w.hwnd().ok())
            .map(|h| h.0 as isize),
    ) else {
        return;
    };
    std::thread::spawn(move || {
        let inside = |h: isize, p: POINT| {
            let mut r = RECT::default();
            unsafe { GetWindowRect(HWND(h as *mut _), &mut r) }.is_ok()
                && p.x >= r.left - 6
                && p.x < r.right + 6
                && p.y >= r.top - 6
                && p.y < r.bottom + 6
        };
        let mut bad_since: Option<Instant> = None;
        loop {
            std::thread::sleep(Duration::from_millis(100));
            if std::fs::remove_file(crate::settings::dir().join("selftest")).is_ok() {
                selftest(app.clone());
            }
            let up = unsafe { IsWindowVisible(HWND(me as *mut _)) }.as_bool();
            // Still cloaked (opening): it can't be in front yet.
            if !up || CLOAKED.load(Ordering::SeqCst) {
                bad_since = None;
                continue;
            }
            let preview = state().is_some_and(|m| m.mode == "thumbs");
            let stranded = if preview {
                let mut p = POINT::default();
                let _ = unsafe { GetCursorPos(&mut p) };
                !inside(me, p) && !dock.is_some_and(|d| inside(d, p))
            } else {
                let fg = unsafe { GetForegroundWindow() }.0 as isize;
                fg != me && !IN_APP_MENU.load(Ordering::SeqCst)
            };
            if !stranded {
                bad_since = None;
                continue;
            }
            let since = *bad_since.get_or_insert_with(Instant::now);
            let limit = Duration::from_millis(if preview { 600 } else { 400 });
            if since.elapsed() >= limit {
                bad_since = None;
                let mode = state().map(|m| m.mode).unwrap_or_default();
                log(&format!("watchdog closed {mode} (left behind)"));
                let a = app.clone();
                let _ = app.run_on_main_thread(move || hide(&a));
            }
        }
    });
}

fn hidden(app: &AppHandle) {
    #[cfg(windows)]
    crate::visibility::win::POPUP.store(false, Ordering::SeqCst);
    #[cfg(windows)]
    crate::thumbs::win::clear();
    let _ = app.emit("popup-hidden", ());
    if let Some(w) = app.get_webview_window(LABEL) {
        crate::webmem::set_low(&w, true);
    }
    crate::ask::popup_hidden();
}

pub fn state() -> Option<Mode> {
    MODE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Open in `mode`. `anchor` is a point along the dock's edge in the dock window's logical px
/// (None = the dock's middle).
pub fn open(app: &AppHandle, mode: &str, prefill: String, anchor: Option<f64>) {
    let Some(win) = app.get_webview_window(LABEL) else {
        return;
    };
    log(&format!("open {mode}"));
    // A hover preview's live thumbnails are drawn by DWM over the page; another view would open
    // underneath them.
    #[cfg(windows)]
    if mode != "thumbs" {
        crate::thumbs::win::clear();
    }
    let m = Mode {
        mode: mode.into(),
        prefill,
    };
    *MODE.lock().unwrap_or_else(|e| e.into_inner()) = Some(m.clone());
    let _ = win.emit_to(LABEL, "popup-mode", &m);
    let (w, h) = size_for(mode);
    *ANCHOR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    position(app, &win, w, h, anchor);
    HAD_FOCUS.store(false, Ordering::SeqCst);
    crate::ask::popup_shown();
    #[cfg(windows)]
    crate::visibility::win::POPUP.store(true, Ordering::SeqCst);
    if mode == "ask" {
        // Start Claude while the user types; it takes a few seconds to come up.
        let app2 = app.clone();
        std::thread::spawn(move || {
            if let Err(e) = crate::ask::ensure(&app2) {
                let _ = app2.emit_to(LABEL, "ask-err", e);
            }
        });
    }
    crate::webmem::set_low(&win, false);
    #[cfg(windows)]
    cloak_until_ready(&win);
    let _ = win.show();
    let _ = win.set_focus();
    #[cfg(windows)]
    if let Ok(h) = win.hwnd() {
        crate::apps::win::focus(h.0 as i64);
    }
    #[cfg(windows)]
    watch_outside_clicks(win.clone());
}

/// The view asks for a new height to fit its content (width stays). Stays at the same place along
/// the dock and keeps hugging the dock's side.
pub fn fit(app: &AppHandle, height: f64) {
    let Some(win) = app.get_webview_window(LABEL) else {
        return;
    };
    // Closing empties the view, and its last "fit" arrives while Windows is still fading the
    // window out; resizing then made the card visibly shrink and jump as it closed.
    if !win.is_visible().unwrap_or(false) {
        return;
    }
    let Ok(Some(mon)) = win.current_monitor() else {
        return;
    };
    let Ok(size) = win.inner_size() else { return };
    let w = size.width as f64 / mon.scale_factor();
    position(app, &win, w, height.clamp(60.0, FIT_MAX), None);
}

/// Size the popup and put it beside the dock. `anchor` (dock logical px) is used on open; a resize
/// reuses the anchor from the last open.
fn position(app: &AppHandle, win: &WebviewWindow, w: f64, h: f64, anchor: Option<f64>) {
    let Some((x, y, pw, ph)) = target_rect(app, w, h, anchor) else {
        let _ = win.set_size(LogicalSize::new(w, h));
        return;
    };
    // One call for size and place: two separate calls showed a frame where the window had grown
    // away from the dock before it jumped back, which flashed on every height change.
    #[cfg(windows)]
    if let Ok(hwnd) = win.hwnd() {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowRect, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
        };
        let mut r = RECT::default();
        let same = unsafe { GetWindowRect(hwnd, &mut r) }.is_ok()
            && (r.left, r.top, r.right - r.left, r.bottom - r.top) == (x, y, pw, ph);
        if !same {
            unsafe {
                let _ = SetWindowPos(hwnd, None, x, y, pw, ph, SWP_NOZORDER | SWP_NOACTIVATE);
            }
            // The glass reads the window's place from geometry; tell it now rather than letting it
            // draw one frame at the old place.
            let _ = app.emit_to(LABEL, "geometry-changed", ());
        }
        return;
    }
    let _ = win.set_size(tauri::PhysicalSize::new(pw as u32, ph as u32));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

/// Where the popup goes for a `w` x `h` (logical px) card: next to the dock, centred on `anchor`
/// (or the last anchor). Physical px: (x, y, width, height). None when the dock isn't there.
fn target_rect(
    app: &AppHandle,
    w: f64,
    h: f64,
    anchor: Option<f64>,
) -> Option<(i32, i32, i32, i32)> {
    let dock = app.get_webview_window("dock")?;
    let (Ok(p), Ok(s), Ok(Some(mon))) = (
        dock.outer_position(),
        dock.outer_size(),
        dock.current_monitor(),
    ) else {
        return None;
    };
    let scale = mon.scale_factor();
    let edge = crate::settings::load().edge;
    let along = if edge.vertical() {
        (p.y, s.height as i32)
    } else {
        (p.x, s.width as i32)
    };
    let mut saved = ANCHOR.lock().unwrap_or_else(|e| e.into_inner());
    let anchor = match (anchor, *saved) {
        (Some(a), _) => along.0 + (a * scale) as i32,
        (None, Some(a)) => a,
        (None, None) => along.0 + along.1 / 2,
    };
    *saved = Some(anchor);
    let (mp, ms) = (mon.position(), mon.size());
    let size = ((w * scale).round() as i32, (h * scale).round() as i32);
    let (x, y) = place(
        (p.x, p.y, p.x + s.width as i32, p.y + s.height as i32),
        (mp.x, mp.y, mp.x + ms.width as i32, mp.y + ms.height as i32),
        size,
        (10.0 * scale) as i32,
        edge,
        anchor,
    );
    Some((x, y, size.0, size.1))
}

pub fn hide(app: &AppHandle) {
    log("hide");
    if let Some(w) = app.get_webview_window(LABEL) {
        hide_window(&w);
    }
    hidden(app);
}

/// Bumped on every preview, so an older preview's watcher knows to stop.
static PREVIEW_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Hover previews of an app's windows (`n` of them): shown without taking focus (the user is only
/// pointing), and closed once the pointer has been away from both the dock and the previews for a
/// moment. A flyout the user opened is never replaced by a preview.
#[cfg(windows)]
pub fn preview(app: &AppHandle, key: String, n: usize, anchor: Option<f64>) {
    let Some(win) = app.get_webview_window(LABEL) else {
        return;
    };
    let visible = win.is_visible().unwrap_or(false);
    if n == 0 || (visible && state().is_some_and(|m| m.mode != "thumbs")) {
        return;
    }
    let m = Mode {
        mode: "thumbs".into(),
        prefill: key,
    };
    *MODE.lock().unwrap_or_else(|e| e.into_inner()) = Some(m.clone());
    let _ = win.emit_to(LABEL, "popup-mode", &m);
    let (w, h) = (n.min(4) as f64 * 212.0 + 16.0, 178.0);
    *ANCHOR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    position(app, &win, w, h, anchor);
    crate::visibility::win::POPUP.store(true, Ordering::SeqCst);
    crate::webmem::set_low(&win, false);
    let Ok(hwnd) = win.hwnd() else { return };
    cloak_until_ready(&win);
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{SW_SHOWNOACTIVATE, ShowWindow};
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    // The dock closes the preview as soon as the pointer moves to another of its widgets.
    let _ = app.emit_to("dock", "preview-shown", ());
    let g = PREVIEW_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    let raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        use windows::Win32::Foundation::{HWND, POINT, RECT};
        use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect};
        let inside = |h: isize, p: POINT| {
            let mut r = RECT::default();
            unsafe { GetWindowRect(HWND(h as *mut _), &mut r) }.is_ok()
                && p.x >= r.left - 6
                && p.x < r.right + 6
                && p.y >= r.top - 6
                && p.y < r.bottom + 6
        };
        let dock = app
            .get_webview_window("dock")
            .and_then(|d| d.hwnd().ok())
            .map(|h| h.0 as isize);
        let mut away = 0u32;
        loop {
            std::thread::sleep(Duration::from_millis(80));
            let still = PREVIEW_GEN.load(Ordering::SeqCst) == g
                && state().is_some_and(|m| m.mode == "thumbs");
            if !still
                || !app
                    .get_webview_window(LABEL)
                    .and_then(|w| w.is_visible().ok())
                    .unwrap_or(false)
            {
                return;
            }
            let mut p = POINT::default();
            let _ = unsafe { GetCursorPos(&mut p) };
            if inside(raw, p) || dock.is_some_and(|d| inside(d, p)) {
                away = 0;
            } else {
                away += 80;
                if away >= 320 {
                    let a = app.clone();
                    let _ = app.run_on_main_thread(move || hide(&a));
                    return;
                }
            }
        }
    });
}

/// The pointer left an app icon for another part of the dock: drop its preview.
pub fn preview_end(app: &AppHandle) {
    if state().is_some_and(|m| m.mode == "thumbs")
        && app
            .get_webview_window(LABEL)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
    {
        hide(app);
    }
}

/// The same mode again closes it (the click that blurred it counts); another mode switches.
pub fn toggle(app: &AppHandle, mode: &str, prefill: String, anchor: Option<f64>) {
    let visible = app
        .get_webview_window(LABEL)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    // A view opened for something (the app menu for one app) toggles only for that same thing.
    let same =
        state().is_some_and(|m| m.mode == mode && (m.mode != "appmenu" || m.prefill == prefill));
    let just_blurred = BLUR_HIDDEN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some_and(|t| t.elapsed() < Duration::from_millis(350));
    if same && (visible || just_blurred) && (prefill.is_empty() || mode == "appmenu") {
        hide(app);
    } else {
        open(app, mode, prefill, anchor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_small_tool_popups_count_as_an_app_menu() {
        const POPUP: u32 = 0x8000_0000;
        const CAPTION: u32 = 0x00C0_0000;
        const TOOL: u32 = 0x80;
        const TOPMOST: u32 = 0x8;
        // A Qt or custom-drawn tray menu.
        assert!(menu_like(POPUP, TOOL | TOPMOST, 220, 300, 1920, 1080));
        // A frameless app window (Electron) the tray click opened: big, not a tool window.
        assert!(!menu_like(POPUP, 0, 1200, 800, 1920, 1080));
        // Even always-on-top, a big window isn't a menu.
        assert!(!menu_like(POPUP, TOPMOST, 1200, 800, 1920, 1080));
        // A normal captioned window.
        assert!(!menu_like(CAPTION, TOOL, 200, 200, 1920, 1080));
    }

    const MON: (i32, i32, i32, i32) = (0, 0, 1920, 1080);

    #[test]
    fn above_a_bottom_dock_centred_on_the_cell() {
        assert_eq!(
            place(
                (400, 1010, 1520, 1074),
                MON,
                (600, 400),
                10,
                Edge::Bottom,
                900
            ),
            (600, 600)
        );
    }

    #[test]
    fn kept_on_screen_and_beside_side_docks() {
        assert_eq!(
            place(
                (400, 1010, 1520, 1074),
                MON,
                (600, 400),
                10,
                Edge::Bottom,
                100
            )
            .0,
            10
        );
        assert_eq!(
            place((6, 300, 70, 780), MON, (400, 500), 10, Edge::Left, 540),
            (80, 290)
        );
        assert_eq!(
            place(
                (1850, 300, 1914, 780),
                MON,
                (400, 500),
                10,
                Edge::Right,
                1060
            ),
            (1440, 570)
        );
    }

    #[test]
    fn tray_flyout_grows_by_rows_of_five() {
        assert_eq!(tray_size(3), (132.0, 52.0));
        assert_eq!(tray_size(5), (212.0, 52.0));
        assert_eq!(tray_size(7), (212.0, 92.0));
        assert_eq!(tray_size(0), (200.0, 60.0));
    }
}
