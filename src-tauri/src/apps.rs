//! The Apps tile: pinned apps plus every open app window, grouped by program. Clicking focuses an
//! app, minimizes it when it's already in front, or launches a pin that isn't running.
//!
//! Pins are freedock's own list (`pins.json` in the settings folder: shortcut or program paths, in
//! dock order). The first time, it is filled from the Windows taskbar's pins; after that, pinning,
//! unpinning and reordering happen on the dock and never touch the Windows taskbar.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Win {
    /// The window handle as a number (stable while the window lives).
    pub hwnd: i64,
    pub title: String,
    pub active: bool,
    pub minimized: bool,
    pub flashing: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct App {
    /// Lowercased exe path, or the shortcut path for a pin we couldn't resolve.
    pub key: String,
    pub name: String,
    pub pinned: bool,
    /// The pinned shortcut, which is what we launch (it keeps its arguments and working folder).
    pub lnk: Option<String>,
    pub exe: Option<String>,
    pub windows: Vec<Win>,
    /// Where the icon comes from (the exe or the shortcut). The UI asks for each icon once
    /// (`app_icon`) and caches it, so a list update carries no image data.
    pub icon: Option<String>,
}

/// Unpinned apps in the order they were first seen, not window z-order, so icons don't reshuffle
/// every time another window comes to the front. `seen` remembers first-seen numbers across calls.
pub fn stable_order(
    apps: &mut [App],
    seen: &mut std::collections::HashMap<String, u64>,
    next: &mut u64,
) {
    for a in apps.iter().filter(|a| !a.pinned) {
        seen.entry(a.key.clone()).or_insert_with(|| {
            *next += 1;
            *next
        });
    }
    // Forget apps that have closed, so reopening one puts it at the end again.
    seen.retain(|k, _| apps.iter().any(|a| &a.key == k));
    let first_unpinned = apps.iter().position(|a| !a.pinned).unwrap_or(apps.len());
    apps[first_unpinned..].sort_by_key(|a| seen.get(&a.key).copied().unwrap_or(u64::MAX));
}

/// Which open windows belong in a taskbar: visible, unowned, titled, not a tool window, not cloaked
/// (other virtual desktops, suspended UWP apps), not shell furniture, not the dock itself.
#[derive(Debug, Clone, Default)]
pub struct Candidate {
    pub visible: bool,
    pub owned: bool,
    pub tool_window: bool,
    pub app_window: bool,
    pub cloaked: bool,
    pub title: String,
    pub class: String,
    pub own_process: bool,
}

const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "Windows.UI.Core.CoreWindow",
];

pub fn belongs(c: &Candidate) -> bool {
    c.visible
        && !c.cloaked
        && !c.own_process
        && !c.title.trim().is_empty()
        && !SHELL_CLASSES.contains(&c.class.as_str())
        && (c.app_window || (!c.owned && !c.tool_window))
}

/// Pins first in their taskbar order, then running apps that aren't pinned, in the order first seen.
pub fn merge(
    pins: Vec<(String, Option<String>, String)>,
    open: Vec<(String, String, Win)>,
) -> Vec<App> {
    let mut apps: Vec<App> = pins
        .into_iter()
        .map(|(lnk, exe, name)| App {
            key: exe
                .clone()
                .map(|e| e.to_lowercase())
                .unwrap_or_else(|| lnk.to_lowercase()),
            name,
            pinned: true,
            lnk: Some(lnk),
            exe,
            windows: vec![],
            icon: None,
        })
        .collect();
    for (exe, name, w) in open {
        let key = exe.to_lowercase();
        match apps.iter_mut().find(|a| a.key == key) {
            Some(a) => a.windows.push(w),
            None => apps.push(App {
                key,
                name,
                pinned: false,
                lnk: None,
                exe: Some(exe),
                windows: vec![w],
                icon: None,
            }),
        }
    }
    apps
}

/// Command-line arguments for files dropped on an app, quoted the way programs split them
/// (CommandLineToArgvW): a run of backslashes right before the closing quote is doubled, so a folder
/// such as `C:\dir\` can't escape its quote. Paths with a quote in them (not valid on Windows) are skipped.
pub fn file_args(paths: &[String]) -> String {
    paths
        .iter()
        .filter(|p| !p.is_empty() && !p.contains('"'))
        .map(|p| {
            let tail = p.len() - p.trim_end_matches('\\').len();
            format!("\"{p}{}\"", "\\".repeat(tail))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Pin (`on`) or unpin a program. `path` is what launches it (a shortcut or the exe); `key` is the
/// app's key (its lowercased exe), which also matches a pinned shortcut that points at that exe.
pub fn pins_edit(
    pins: &[String],
    targets: &[Option<String>],
    path: &str,
    key: &str,
    on: bool,
) -> Vec<String> {
    let same = |i: usize| {
        pins[i].to_lowercase() == key
            || targets
                .get(i)
                .cloned()
                .flatten()
                .is_some_and(|t| t.to_lowercase() == key)
    };
    if on {
        let mut out = pins.to_vec();
        if !(0..pins.len()).any(same) {
            out.push(path.to_string());
        }
        out
    } else {
        (0..pins.len())
            .filter(|&i| !same(i))
            .map(|i| pins[i].clone())
            .collect()
    }
}

/// Pins put in the order of `keys` (the app keys as the user arranged them); pins not named keep
/// their place at the end.
pub fn pins_reorder(pins: &[String], pin_keys: &[String], keys: &[String]) -> Vec<String> {
    let mut idx: Vec<usize> = (0..pins.len()).collect();
    let rank = |i: &usize| {
        keys.iter()
            .position(|k| *k == pin_keys[*i])
            .unwrap_or(usize::MAX)
    };
    idx.sort_by_key(rank);
    idx.into_iter().map(|i| pins[i].clone()).collect()
}

/// What a click does: the app's front window minimizes, the next of its windows comes forward,
/// or the pin launches.
#[derive(Debug, PartialEq)]
pub enum Click {
    Minimize(i64),
    Focus(i64),
    Launch,
}

/// The Start tile: open Start only if it isn't open and wasn't closed a moment ago. Clicking the dock while
/// Start is open already closes it (the click lands outside Start), so tapping Win again would reopen it.
pub fn start_should_open(start_open: bool, closed_ago: Option<std::time::Duration>) -> bool {
    !start_open && closed_ago.is_none_or(|d| d > std::time::Duration::from_millis(400))
}

pub fn click(app: &App) -> Click {
    let ws = &app.windows;
    if ws.is_empty() {
        return Click::Launch;
    }
    match ws.iter().position(|w| w.active) {
        Some(i) if ws.len() == 1 => Click::Minimize(ws[i].hwnd),
        Some(i) => Click::Focus(ws[(i + 1) % ws.len()].hwnd),
        None => Click::Focus(ws.iter().find(|w| w.flashing).unwrap_or(&ws[0]).hwnd),
    }
}

#[cfg(windows)]
pub mod win {
    use std::collections::{HashMap, HashSet};
    use std::ffi::c_void;
    use std::path::Path;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    use base64::Engine;
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH, SIZE};
    use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDIBits, HBITMAP, HGDIOBJ,
    };
    use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        IPersistFile, STGM_READ,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_LWIN, VK_MENU,
    };
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, IShellLinkW, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK,
        SIIGBF_ICONONLY, ShellExecuteW, ShellLink,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GW_OWNER, GWL_EXSTYLE, GetClassNameW, GetForegroundWindow,
        GetWindow, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, PostMessageW, SW_MINIMIZE, SW_RESTORE,
        SW_SHOWNORMAL, SetForegroundWindow, ShowWindow, WINEVENT_OUTOFCONTEXT,
        WINEVENT_SKIPOWNPROCESS, WM_CLOSE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };
    use windows::core::{BOOL, HSTRING, PCWSTR, w};

    use super::*;

    pub fn hwnd(raw: i64) -> HWND {
        HWND(raw as *mut c_void)
    }

    /// Windows that asked for attention (HSHELL_FLASH) and haven't been looked at since.
    pub static FLASHING: Mutex<Option<HashSet<i64>>> = Mutex::new(None);
    /// Set by the WinEvent hook, read by the watcher thread, which tells the UI to re-read the list.
    pub static CHANGED: AtomicBool = AtomicBool::new(true);
    /// Only window titles changed: the list is re-sent at most every 2 s for these.
    pub static TITLES_CHANGED: AtomicBool = AtomicBool::new(false);

    fn text(h: HWND) -> String {
        let n = unsafe { GetWindowTextLengthW(h) };
        if n <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; n as usize + 1];
        let got = unsafe { GetWindowTextW(h, &mut buf) };
        String::from_utf16_lossy(&buf[..got as usize])
    }

    fn class(h: HWND) -> String {
        let mut buf = [0u16; 256];
        let n = unsafe { GetClassNameW(h, &mut buf) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    fn pid(h: HWND) -> u32 {
        let mut p = 0u32;
        unsafe { GetWindowThreadProcessId(h, Some(&mut p)) };
        p
    }

    fn exe_of_pid(p: u32) -> Option<String> {
        unsafe {
            let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, p).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                proc,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut len,
            );
            let _ = CloseHandle(proc);
            ok.ok()?;
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    /// A Store app's window belongs to ApplicationFrameHost; the real program owns a child window.
    pub fn real_exe(h: HWND) -> Option<String> {
        let exe = exe_of_pid(pid(h))?;
        if !exe.to_lowercase().ends_with("applicationframehost.exe") {
            return Some(exe);
        }
        let host = pid(h);
        let mut found: (u32, u32) = (host, 0);
        unsafe extern "system" fn each(child: HWND, lp: LPARAM) -> BOOL {
            let f = unsafe { &mut *(lp.0 as *mut (u32, u32)) };
            let p = pid(child);
            if p != f.0 {
                f.1 = p;
                return BOOL(0);
            }
            BOOL(1)
        }
        unsafe {
            let _ = EnumChildWindows(Some(h), Some(each), LPARAM(&mut found as *mut _ as isize));
        }
        if found.1 != 0 {
            exe_of_pid(found.1)
        } else {
            Some(exe)
        }
    }

    fn cloaked(h: HWND) -> bool {
        let mut v = 0u32;
        unsafe { DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut v as *mut _ as *mut c_void, 4) }
            .is_ok()
            && v != 0
    }

    fn display_name(exe: &str, title: &str) -> String {
        // The program's own description would be nicer; the file stem is good enough for a tooltip.
        let stem = Path::new(exe)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if stem.is_empty() {
            title.to_string()
        } else {
            stem
        }
    }

    fn candidate(h: HWND, me: u32) -> Candidate {
        let ex = unsafe { GetWindowLongPtrW(h, GWL_EXSTYLE) } as u32;
        Candidate {
            visible: unsafe { IsWindowVisible(h) }.as_bool(),
            owned: unsafe { GetWindow(h, GW_OWNER) }
                .map(|o| !o.is_invalid())
                .unwrap_or(false),
            tool_window: ex & WS_EX_TOOLWINDOW.0 != 0,
            app_window: ex & WS_EX_APPWINDOW.0 != 0,
            cloaked: cloaked(h),
            title: text(h),
            class: class(h),
            own_process: pid(h) == me,
        }
    }

    /// A window the taskbar would list (used by visibility.rs to find maximized apps).
    pub fn is_app_window(h: HWND, me: u32) -> bool {
        belongs(&candidate(h, me))
    }

    fn open_windows() -> Vec<(String, String, Win)> {
        let mut out: Vec<(String, String, Win)> = vec![];
        let me = std::process::id();
        let fg = unsafe { GetForegroundWindow() };
        let flashing = FLASHING
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or_default();
        let mut handles: Vec<HWND> = vec![];
        unsafe extern "system" fn each(h: HWND, lp: LPARAM) -> BOOL {
            unsafe { (*(lp.0 as *mut Vec<HWND>)).push(h) };
            BOOL(1)
        }
        unsafe {
            let _ = EnumWindows(Some(each), LPARAM(&mut handles as *mut _ as isize));
        }
        for h in handles {
            let c = candidate(h, me);
            if !belongs(&c) {
                continue;
            }
            let Some(exe) = real_exe(h) else { continue };
            let raw = h.0 as i64;
            let name = display_name(&exe, &c.title);
            out.push((
                exe,
                name,
                Win {
                    hwnd: raw,
                    title: c.title,
                    active: h == fg,
                    minimized: unsafe { IsIconic(h) }.as_bool(),
                    flashing: flashing.contains(&raw),
                },
            ));
        }
        out
    }

    /// Each pin: what launches it (shortcut or exe), the program it starts, and its name.
    type Pins = Vec<(String, Option<String>, String)>;
    /// The pin list as last read, keyed by the file's contents (reading a shortcut costs a COM call).
    static PINS: Mutex<Option<(Vec<String>, Pins)>> = Mutex::new(None);

    fn pins_file() -> std::path::PathBuf {
        crate::settings::dir().join("pins.json")
    }

    /// Held while pins.json is read (and maybe imported) and across every load-modify-save, so two
    /// edits at once (a pin and a reorder from the dock) can't each save over the other's change.
    static PINS_FILE: Mutex<()> = Mutex::new(());

    /// The pinned paths in dock order. The first time, they come from the Windows taskbar's pins.
    /// Call with PINS_FILE held. Err when pins.json exists but can't be read right now (locked by a
    /// sync tool or antivirus): callers show no pins, and edits refuse to save, so a passing read
    /// error never wipes the user's pins.
    fn pin_paths() -> Result<Vec<String>, String> {
        // Windows' taskbar pins are imported only on the very first run (no pins file yet). One
        // that's damaged is kept as pins.json.bad and the import runs again.
        match std::fs::read_to_string(pins_file()) {
            Ok(s) => match serde_json::from_str(&s) {
                Ok(v) => return Ok(v),
                Err(_) => {
                    let _ = std::fs::copy(pins_file(), pins_file().with_extension("json.bad"));
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("the pins file can't be read right now: {e}")),
        }
        let Some(appdata) = std::env::var_os("APPDATA") else {
            return Ok(vec![]);
        };
        let dir = Path::new(&appdata)
            .join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar");
        let mut lnks: Vec<String> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "lnk"))
                    .map(|p| p.to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        lnks.sort();
        let _ = save_pins(&lnks);
        Ok(lnks)
    }

    /// Only called with PINS_FILE held (pin_paths, edit_pins).
    fn save_pins(paths: &[String]) -> Result<(), String> {
        let _ = std::fs::create_dir_all(crate::settings::dir());
        let f = pins_file();
        let tmp = f.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_string_pretty(paths).map_err(|e| e.to_string())?,
        )
        .and_then(|_| std::fs::rename(&tmp, &f))
        .map_err(|e| e.to_string())?;
        CHANGED.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// The pins to show. An unreadable pins file shows none (see pin_paths).
    pub fn pins() -> Pins {
        let paths = {
            let _held = PINS_FILE.lock().unwrap_or_else(|e| e.into_inner());
            pin_paths()
        };
        resolve(paths.unwrap_or_default())
    }

    /// Change the pins: `f` gets the current pins and returns the new path list, which is saved. The
    /// whole read-change-save runs under PINS_FILE, and an unreadable pins file refuses the change
    /// instead of saving `f`'s edit of an empty list over the real one.
    pub fn edit_pins(f: impl FnOnce(&Pins) -> Vec<String>) -> Result<(), String> {
        let _held = PINS_FILE.lock().unwrap_or_else(|e| e.into_inner());
        let pins = resolve(pin_paths()?);
        save_pins(&f(&pins))
    }

    /// Each pin's target and name; shortcut targets cost a COM call, so the last answer is kept.
    fn resolve(paths: Vec<String>) -> Pins {
        if let Some((at, p)) = PINS.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if *at == paths {
                return p.clone();
            }
        }
        let got: Pins = paths
            .iter()
            .map(|p| {
                let name = Path::new(p)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let target = if p.to_lowercase().ends_with(".lnk") {
                    lnk_target(p)
                } else {
                    Some(p.clone())
                };
                (p.clone(), target, name)
            })
            .collect();
        *PINS.lock().unwrap_or_else(|e| e.into_inner()) = Some((paths, got.clone()));
        got
    }

    /// Ask a window to close, as its own close button would (it may still ask to save first).
    pub fn close(raw: i64) {
        unsafe {
            let _ = PostMessageW(
                Some(hwnd(raw)),
                WM_CLOSE,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            );
        }
    }

    /// What a .lnk shortcut points at (COM is initialised here, so any thread can call it).
    pub fn lnk_target(lnk: &str) -> Option<String> {
        let lnk = lnk.to_string();
        crate::sta::run(move || lnk_target_here(&lnk)).flatten()
    }

    fn lnk_target_here(lnk: &str) -> Option<String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
            let file: IPersistFile = windows::core::Interface::cast(&link).ok()?;
            file.Load(&HSTRING::from(lnk), STGM_READ).ok()?;
            let mut buf = [0u16; MAX_PATH as usize];
            let mut fd = WIN32_FIND_DATAW::default();
            link.GetPath(&mut buf, &mut fd, 0).ok()?;
            let s = String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(0)]);
            // File Explorer's pin has no target path; it maps to explorer.exe windows.
            if s.is_empty() && lnk.to_lowercase().ends_with("file explorer.lnk") {
                return std::env::var("WINDIR")
                    .ok()
                    .map(|w| format!(r"{w}\explorer.exe"));
            }
            (!s.is_empty()).then_some(s)
        }
    }

    static SEEN: Mutex<Option<(HashMap<String, u64>, u64)>> = Mutex::new(None);

    pub fn list() -> Vec<App> {
        let mut apps = merge(pins(), open_windows());
        let mut g = SEEN.lock().unwrap_or_else(|e| e.into_inner());
        let (seen, next) = g.get_or_insert_with(|| (HashMap::new(), 0));
        stable_order(&mut apps, seen, next);
        for a in &mut apps {
            a.icon = a.exe.clone().or_else(|| a.lnk.clone());
        }
        apps
    }

    // ---------------------------------------------------------------- icons

    static ICONS: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

    /// A 64 px icon for a program or shortcut as a PNG data URL, cached per path. Drawn on the shell
    /// thread (see sta.rs). A failure isn't cached, so the next ask tries again.
    pub fn icon(path: &str) -> Option<String> {
        if let Some(hit) = ICONS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_or_insert_with(HashMap::new)
            .get(path)
        {
            return Some(hit.clone());
        }
        let p = path.to_string();
        let got = crate::sta::run(move || unsafe { render_icon(&p) }).flatten()?;
        ICONS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_or_insert_with(HashMap::new)
            .insert(path.into(), got.clone());
        Some(got)
    }

    unsafe fn render_icon(path: &str) -> Option<String> {
        let factory: IShellItemImageFactory =
            unsafe { SHCreateItemFromParsingName(&HSTRING::from(path), None) }.ok()?;
        let bmp = unsafe {
            factory.GetImage(
                SIZE { cx: 64, cy: 64 },
                SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK,
            )
        }
        .ok()?;
        let png = unsafe { bitmap_png(bmp) };
        unsafe {
            let _ = DeleteObject(HGDIOBJ(bmp.0));
        }
        Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png?)
        ))
    }

    /// A thumbnail (photos, videos, PDFs) or else the file's icon, as a PNG data URL. Not cached.
    pub fn thumbnail(path: &str, size: i32) -> Option<String> {
        let path = path.to_string();
        crate::sta::run(move || thumbnail_here(&path, size)).flatten()
    }

    fn thumbnail_here(path: &str, size: i32) -> Option<String> {
        unsafe {
            let factory: IShellItemImageFactory =
                SHCreateItemFromParsingName(&HSTRING::from(path), None).ok()?;
            let bmp = factory
                .GetImage(SIZE { cx: size, cy: size }, SIIGBF_BIGGERSIZEOK)
                .ok()?;
            let png = bitmap_png(bmp);
            let _ = DeleteObject(HGDIOBJ(bmp.0));
            Some(format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(png?)
            ))
        }
    }

    unsafe fn bitmap_png(bmp: HBITMAP) -> Option<Vec<u8>> {
        let dc = unsafe { CreateCompatibleDC(None) };
        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        // First call fills in the size; the second copies the pixels as top-down 32-bit BGRA.
        unsafe { GetDIBits(dc, bmp, 0, 0, None, &mut bi, DIB_RGB_COLORS) };
        let (w, h) = (bi.bmiHeader.biWidth, bi.bmiHeader.biHeight.abs());
        if w <= 0 || h <= 0 {
            unsafe {
                let _ = DeleteDC(dc);
            }
            return None;
        }
        bi.bmiHeader.biHeight = -h;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB.0;
        let mut px = vec![0u8; (w * h * 4) as usize];
        let rows = unsafe {
            GetDIBits(
                dc,
                bmp,
                0,
                h as u32,
                Some(px.as_mut_ptr() as *mut c_void),
                &mut bi,
                DIB_RGB_COLORS,
            )
        };
        unsafe {
            let _ = DeleteDC(dc);
        }
        if rows == 0 {
            return None;
        }
        // Photo thumbnails often come without alpha (all zero): those are opaque, just swap channels.
        if px.chunks_exact(4).all(|p| p[3] == 0) {
            for p in px.chunks_exact_mut(4) {
                p.swap(0, 2);
                p[3] = 255;
            }
        } else {
            // BGRA (premultiplied) to RGBA.
            for p in px.chunks_exact_mut(4) {
                let a = p[3] as u32;
                let (b, g, r) = (p[0] as u32, p[1] as u32, p[2] as u32);
                let un = |c: u32| {
                    if a == 0 {
                        0
                    } else {
                        ((c * 255 + a / 2) / a).min(255) as u8
                    }
                };
                p[0] = un(r);
                p[1] = un(g);
                p[2] = un(b);
            }
        }
        let mut out = vec![];
        {
            let mut enc = png::Encoder::new(&mut out, w as u32, h as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut wr = enc.write_header().ok()?;
            wr.write_image_data(&px).ok()?;
        }
        Some(out)
    }

    // ---------------------------------------------------------------- actions

    /// Bring a window forward. The dock never takes focus itself, so Windows' foreground lock can refuse;
    /// a synthetic Alt tap is the usual way taskbar replacements get past it.
    pub fn focus(raw: i64) {
        let h = hwnd(raw);
        unsafe {
            if IsIconic(h).as_bool() {
                let _ = ShowWindow(h, SW_RESTORE);
            }
            if !SetForegroundWindow(h).as_bool() {
                let key = |flags| INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_MENU,
                            dwFlags: flags,
                            ..Default::default()
                        },
                    },
                };
                SendInput(
                    &[key(Default::default()), key(KEYEVENTF_KEYUP)],
                    size_of::<INPUT>() as i32,
                );
                let _ = SetForegroundWindow(h);
            }
        }
        if let Some(set) = FLASHING.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            set.remove(&raw);
        }
    }

    /// A Windows key tap: opens Start (and shows the auto-hidden taskbar's Start, as Windows does).
    /// True while Start or Windows search is the foreground window (set by the foreground hook).
    pub static START_OPEN: AtomicBool = AtomicBool::new(false);
    static START_CLOSED_AT: Mutex<Option<std::time::Instant>> = Mutex::new(None);

    fn is_start(h: HWND) -> bool {
        exe_of_pid(pid(h)).is_some_and(|e| {
            let e = e.to_lowercase();
            e.ends_with("startmenuexperiencehost.exe") || e.ends_with("searchhost.exe")
        })
    }

    fn foreground_changed(h: HWND) {
        let open = is_start(h);
        if START_OPEN.swap(open, Ordering::SeqCst) && !open {
            *START_CLOSED_AT.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(std::time::Instant::now());
        }
        crate::notify("start-changed");
    }

    /// The Start tile: toggles Start instead of replaying its opening animation.
    pub fn start_toggle() {
        let closed_ago = START_CLOSED_AT
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .map(|t| t.elapsed());
        if super::start_should_open(START_OPEN.load(Ordering::SeqCst), closed_ago) {
            press_win();
        }
    }

    /// Win + a letter: 'A' opens Quick Settings, 'N' notifications and the calendar.
    /// The keyboard language of the window in front ("EN", "FR", ...) and how many layouts are
    /// installed (the widget hides itself when there is only one).
    pub fn keyboard() -> (String, usize) {
        use windows::Win32::Globalization::GetLocaleInfoW;
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetKeyboardLayout, GetKeyboardLayoutList,
        };
        unsafe {
            let tid = GetWindowThreadProcessId(GetForegroundWindow(), None);
            let lang = (GetKeyboardLayout(tid).0 as usize & 0xffff) as u32;
            let n = GetKeyboardLayoutList(None).max(0) as usize;
            let mut buf = [0u16; 16];
            // LOCALE_SISO639LANGNAME: the two-letter language code.
            let len = GetLocaleInfoW(lang, 0x59, Some(&mut buf)).max(1) as usize - 1;
            (String::from_utf16_lossy(&buf[..len]).to_uppercase(), n)
        }
    }

    pub fn press_win_with(letter: u16) {
        use windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY;
        let key = |vk, flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        };
        let up = KEYEVENTF_KEYUP;
        let l = VIRTUAL_KEY(letter);
        unsafe {
            SendInput(
                &[
                    key(VK_LWIN, Default::default()),
                    key(l, Default::default()),
                    key(l, up),
                    key(VK_LWIN, up),
                ],
                size_of::<INPUT>() as i32,
            )
        };
    }

    pub fn press_win() {
        let key = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_LWIN,
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

    pub fn minimize(raw: i64) {
        unsafe {
            let _ = ShowWindow(hwnd(raw), SW_MINIMIZE);
        }
    }

    /// Open dropped files with this app (its program, with the files as arguments).
    pub fn open_with(app: &App, paths: &[String]) {
        let Some(exe) = app.exe.clone() else { return };
        // Only real files and folders that exist; anything else is not a dropped file.
        let paths: Vec<String> = paths
            .iter()
            .filter(|p| Path::new(p.as_str()).is_absolute() && Path::new(p.as_str()).exists())
            .cloned()
            .collect();
        let args = super::file_args(&paths);
        if args.is_empty() {
            return;
        }
        // On the shell's STA lane: callers are pool or plain threads, possibly in the multithreaded
        // apartment or with no COM at all, where ShellExecute fails for some targets (sta.rs).
        crate::sta::run(move || unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                &HSTRING::from(exe),
                &HSTRING::from(args),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        });
    }

    pub fn launch(app: &App) {
        let target = app.lnk.clone().or(app.exe.clone()).unwrap_or_default();
        if target.is_empty() {
            return;
        }
        // On the shell's STA lane, as in open_with: Win+N clicks come from a plain thread with no
        // COM, and app clicks from the blocking pool.
        crate::sta::run(move || unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                &HSTRING::from(target),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        });
    }

    // ---------------------------------------------------------------- live updates

    unsafe extern "system" fn on_event(
        _: HWINEVENTHOOK,
        ev: u32,
        h: HWND,
        id_object: i32,
        id_child: i32,
        _: u32,
        _: u32,
    ) {
        if id_object == 0 && id_child == 0 {
            // EVENT_OBJECT_SHOW: the old taskbar may be coming back.
            if ev == 0x8002 {
                crate::taskbar::rehide_if_tray(h.0 as isize);
            }
            // EVENT_SYSTEM_FOREGROUND: track whether Start is open.
            if ev == 0x0003 {
                foreground_changed(h);
            }
            // EVENT_OBJECT_NAMECHANGE fires constantly (terminal spinners, browser tabs, clocks in
            // titles). It only matters for the tooltip of a visible top-level window, so it goes on
            // the slow lane.
            if ev == 0x800C {
                if unsafe { IsWindowVisible(h) }.as_bool()
                    && unsafe { GetWindow(h, GW_OWNER) }.is_err()
                {
                    TITLES_CHANGED.store(true, Ordering::SeqCst);
                }
                return;
            }
            CHANGED.store(true, Ordering::SeqCst);
        }
    }

    /// Window create/show/hide/destroy/rename/foreground/minimize/cloak all mark the list stale.
    /// Must run on a thread with a message loop (the Tauri main thread).
    pub fn watch() {
        const RANGES: &[(u32, u32)] = &[
            (0x0003, 0x0003),
            (0x0016, 0x0017),
            (0x8001, 0x8003),
            (0x800C, 0x800C),
            (0x8017, 0x8018),
        ];
        for &(lo, hi) in RANGES {
            unsafe {
                SetWinEventHook(
                    lo,
                    hi,
                    None,
                    Some(on_event),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                );
            }
        }
    }

    /// HSHELL_FLASH from the shell hook (appbar.rs's window procedure): this window wants attention.
    pub fn flash(raw: i64, on: bool) {
        let mut g = FLASHING.lock().unwrap_or_else(|e| e.into_inner());
        let set = g.get_or_insert_with(HashSet::new);
        if on {
            set.insert(raw);
        } else {
            set.remove(&raw);
        }
        CHANGED.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_files_become_quoted_arguments() {
        let p = [
            r"C:\a b\x.txt".to_string(),
            "bad\"name".to_string(),
            r"D:\y.png".to_string(),
        ];
        assert_eq!(file_args(&p), r#""C:\a b\x.txt" "D:\y.png""#);
        assert_eq!(
            file_args(&[r"C:\dir\".to_string()]),
            r#""C:\dir\\""#,
            "a trailing backslash is doubled"
        );
    }

    #[test]
    fn pin_and_unpin() {
        let pins = vec![r"C:\x\Code.lnk".to_string()];
        let targets = vec![Some(r"C:\Apps\Code.exe".to_string())];
        let same = pins_edit(
            &pins,
            &targets,
            r"C:\Apps\Code.exe",
            r"c:\apps\code.exe",
            true,
        );
        assert_eq!(same, pins, "already pinned through its shortcut");
        let more = pins_edit(
            &pins,
            &targets,
            r"C:\Apps\Note.exe",
            r"c:\apps\note.exe",
            true,
        );
        assert_eq!(more.len(), 2);
        let none = pins_edit(&pins, &targets, "", r"c:\apps\code.exe", false);
        assert!(none.is_empty(), "unpinning matches the shortcut's target");
    }

    #[test]
    fn reorder_pins_by_key() {
        let pins = ["a.lnk", "b.exe", "c.lnk"].map(String::from);
        let keys = ["ka", "kb", "kc"].map(String::from);
        let order = ["kc", "ka"].map(String::from);
        assert_eq!(
            pins_reorder(&pins, &keys, &order),
            ["c.lnk", "a.lnk", "b.exe"]
        );
    }

    fn base() -> Candidate {
        Candidate {
            visible: true,
            title: "Doc".into(),
            class: "Notepad".into(),
            ..Default::default()
        }
    }

    #[test]
    fn taskbar_window_filter() {
        assert!(belongs(&base()));
        assert!(
            !belongs(&Candidate {
                visible: false,
                ..base()
            }),
            "hidden"
        );
        assert!(
            !belongs(&Candidate {
                cloaked: true,
                ..base()
            }),
            "other desktop / suspended"
        );
        assert!(
            !belongs(&Candidate {
                title: " ".into(),
                ..base()
            }),
            "untitled"
        );
        assert!(
            !belongs(&Candidate {
                owned: true,
                ..base()
            }),
            "dialog owned by another window"
        );
        assert!(
            !belongs(&Candidate {
                tool_window: true,
                ..base()
            }),
            "tool window"
        );
        assert!(
            belongs(&Candidate {
                tool_window: true,
                app_window: true,
                ..base()
            }),
            "APPWINDOW wins"
        );
        assert!(
            !belongs(&Candidate {
                class: "Progman".into(),
                ..base()
            }),
            "the desktop"
        );
        assert!(
            !belongs(&Candidate {
                own_process: true,
                ..base()
            }),
            "the dock itself"
        );
    }

    fn win(hwnd: i64, active: bool) -> Win {
        Win {
            hwnd,
            title: String::new(),
            active,
            minimized: false,
            flashing: false,
        }
    }

    #[test]
    fn unpinned_apps_keep_first_seen_order() {
        let app = |k: &str, pinned| App {
            key: k.into(),
            name: k.into(),
            pinned,
            lnk: None,
            exe: None,
            windows: vec![],
            icon: None,
        };
        let mut seen = std::collections::HashMap::new();
        let mut next = 0;
        let mut a = vec![app("pin", true), app("b", false), app("a", false)];
        stable_order(&mut a, &mut seen, &mut next);
        // Window z-order changes (a is now in front): the order stays b, a.
        let mut z = vec![
            app("pin", true),
            app("a", false),
            app("b", false),
            app("c", false),
        ];
        stable_order(&mut z, &mut seen, &mut next);
        assert_eq!(
            z.iter().map(|x| x.key.as_str()).collect::<Vec<_>>(),
            ["pin", "b", "a", "c"]
        );
        // b closes and comes back: it goes to the end.
        let mut gone = vec![app("pin", true), app("a", false), app("c", false)];
        stable_order(&mut gone, &mut seen, &mut next);
        let mut back = vec![
            app("pin", true),
            app("b", false),
            app("a", false),
            app("c", false),
        ];
        stable_order(&mut back, &mut seen, &mut next);
        assert_eq!(
            back.iter().map(|x| x.key.as_str()).collect::<Vec<_>>(),
            ["pin", "a", "c", "b"]
        );
    }

    #[test]
    fn pins_and_windows_merge_by_program() {
        let apps = merge(
            vec![(
                "C:/p/Code.lnk".into(),
                Some(r"C:\Code\Code.exe".into()),
                "Code".into(),
            )],
            vec![
                (r"C:\code\code.exe".into(), "code".into(), win(1, false)),
                (r"C:\x\Firefox.exe".into(), "Firefox".into(), win(2, true)),
            ],
        );
        assert_eq!(apps.len(), 2);
        assert!(
            apps[0].pinned && apps[0].windows.len() == 1,
            "the open window joins its pin (case-insensitive)"
        );
        assert!(!apps[1].pinned && apps[1].name == "Firefox");
    }

    #[test]
    fn start_tile_toggles() {
        use std::time::Duration;
        assert!(
            start_should_open(false, None),
            "closed for a while: open it"
        );
        assert!(!start_should_open(true, None), "open: leave it to close");
        assert!(
            !start_should_open(false, Some(Duration::from_millis(80))),
            "our click just closed it"
        );
        assert!(
            start_should_open(false, Some(Duration::from_secs(3))),
            "closed long ago: open it"
        );
    }

    #[test]
    fn click_toggles_cycles_or_launches() {
        let mut a = merge(
            vec![("C:/p/A.lnk".into(), Some("a.exe".into()), "A".into())],
            vec![],
        )
        .remove(0);
        assert_eq!(click(&a), Click::Launch);
        a.windows = vec![win(1, true)];
        assert_eq!(
            click(&a),
            Click::Minimize(1),
            "its only window is in front: minimize"
        );
        a.windows = vec![win(1, true), win(2, false)];
        assert_eq!(click(&a), Click::Focus(2), "several windows: the next one");
        a.windows = vec![win(1, false), win(2, false)];
        assert_eq!(click(&a), Click::Focus(1), "not in front: bring it");
    }
}
