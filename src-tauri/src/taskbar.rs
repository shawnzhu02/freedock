//! The Windows taskbar: hidden while the dock runs, put back exactly as it was when the dock quits.
//!
//! Two layers. Auto-hide (a remembered Windows setting) frees the screen space the taskbar reserves.
//! Auto-hide alone still slides the old taskbar back in on edge hover, Start and notifications, so its
//! windows are also hidden outright, and `rehide_if_tray` hides them again whenever Explorer shows them.
//! A guard process (`--guard <pid>`) waits for the dock to exit by any route, a Task Manager kill
//! included, and then restores; hidden tray windows would otherwise stay hidden until Explorer restarts.
//!
//! Auto-hide is a setting Windows remembers, so putting it back must never fail. The state we found is
//! written to `taskbar.json` before anything changes, with `dirty: true`. A clean quit restores it and
//! clears the flag. A crash or a Task Manager kill leaves the flag set, and the next start restores it
//! before doing anything else.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    /// The ABM_GETSTATE bits we found (ABS_AUTOHIDE = 1, ABS_ALWAYSONTOP = 2).
    pub state: u32,
    /// True while our change is in effect; still true on disk after a crash.
    pub dirty: bool,
    /// The dock process that made the change. A guard only restores for its own dock, so an old guard
    /// can't undo a newer dock's takeover (a restart, or the dev watcher relaunching).
    #[serde(default)]
    pub owner: u32,
}

pub const ABS_AUTOHIDE: u32 = 1;

/// What to write back on startup, if a previous run didn't restore.
pub fn pending_restore(file: &Path) -> Option<u32> {
    read(file).filter(|s| s.dirty).map(|s| s.state)
}

fn read(file: &Path) -> Option<Saved> {
    serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
}

/// Whether the dock process `pid` still owns the takeover (0 = an old file without an owner).
pub fn owned_by(file: &Path, pid: u32) -> bool {
    read(file).is_none_or(|s| s.owner == 0 || s.owner == pid)
}

/// Whether the taskbar was handed back from outside this run (`--restore-taskbar`, the guard): the
/// note reads clean. An unreadable note doesn't count, so a locked file never brings the bar back.
pub fn released(file: &Path) -> bool {
    read(file).is_some_and(|s| !s.dirty)
}

pub fn state_file() -> PathBuf {
    crate::settings::dir().join("taskbar.json")
}

/// Returns false when the file couldn't be written.
fn write(file: &Path, saved: Saved) -> bool {
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Written to a temp file and renamed, so a crash mid-write never leaves a half file.
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(&saved).unwrap_or_default()).is_ok()
        && std::fs::rename(&tmp, file).is_ok()
}

/// Startup: undo a crashed run, then remember the current state and auto-hide.
pub fn take_over() {
    let file = state_file();
    // After a crash the note holds the user's real setting; reading it back from Windows could give
    // the crashed run's auto-hide instead if Explorer isn't ready to take the replay yet.
    let state = match pending_restore(&file) {
        Some(state) => {
            win::set_state(state);
            write(
                &file,
                Saved {
                    state,
                    dirty: false,
                    owner: 0,
                },
            );
            state
        }
        None => win::get_state(),
    };
    // Only hide what we can promise to bring back: without a written note, a crash would leave the
    // taskbar hidden with nothing to restore it from.
    if !write(
        &file,
        Saved {
            state,
            dirty: true,
            owner: std::process::id(),
        },
    ) {
        eprintln!("taskbar: couldn't save its state, so it stays as it is");
        return;
    }
    win::set_state(state | ABS_AUTOHIDE);
    win::show_trays(false);
}

/// Quit (and the panic hook, the guard, `--restore-taskbar`): put the taskbar back as we found it.
/// Safe to call more than once. Showing the tray windows is unconditional: it is harmless when they
/// are already visible, and the only way back after a hard kill.
///
/// Order matters when another dock is still running (`--restore-taskbar` from a second process): the
/// clean note is written before the trays are shown, so that dock's show-event hook (`rehide_if_tray`)
/// already reads "released" and leaves them alone instead of hiding them again.
pub fn restore() {
    let file = state_file();
    if let Some(state) = pending_restore(&file) {
        // The user's own taskbar was auto-hide too: switching it off and on again makes Explorer
        // list it as the edge's auto-hide bar again (freedock took it off the list).
        if state & ABS_AUTOHIDE != 0 {
            win::set_state(state & !ABS_AUTOHIDE);
        }
        win::set_state(state);
        // Explorer down or restarting: the change didn't take, so leave the note for the next try
        // (the next start, or `--restore-taskbar`). The trays are still shown below.
        if win::get_state() == state {
            write(
                &file,
                Saved {
                    state,
                    dirty: false,
                    owner: 0,
                },
            );
        } else {
            eprintln!("taskbar: Windows didn't take the restored state; will retry next start");
        }
    }
    win::show_trays(true);
}

/// Start the guard: a second copy of this exe that restores the taskbar once `pid` exits.
pub fn spawn_guard() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    // Started through a short-lived middle process (`--guard-launch`), so the guard isn't the dock's
    // child: Task Manager's "End task" on freedock ends the dock's process tree, and a child guard
    // would go with it, leaving nothing to restore the taskbar.
    let mut cmd = std::process::Command::new(exe);
    cmd.args(["--guard-launch", &std::process::id().to_string()]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window, and not killed along with the dock's console in dev runs.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    if let Err(e) = cmd.spawn() {
        eprintln!("taskbar guard did not start: {e}");
    }
}

/// `--guard-launch <pid>`: start the real guard for `pid`, then exit right away.
pub fn guard_launch(pid: u32) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.args(["--guard", &pid.to_string()]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        let flags = CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        // Leave the dock's job object when allowed; when it isn't, start inside it instead.
        cmd.creation_flags(flags | CREATE_BREAKAWAY_FROM_JOB);
        if cmd.spawn().is_ok() {
            return;
        }
        cmd.creation_flags(flags);
    }
    if let Err(e) = cmd.spawn() {
        eprintln!("taskbar guard did not start: {e}");
    }
}

/// The guard's whole job: wait for the dock process to end, then restore.
pub fn guard(pid: u32) {
    win::wait_for_exit(pid);
    // A newer dock took over in the meantime: the taskbar is its business now.
    if owned_by(&state_file(), pid) {
        restore();
    }
}

pub use win::rehide_if_tray;

#[cfg(windows)]
pub mod win {
    use std::sync::atomic::{AtomicBool, Ordering};

    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::System::Threading::{
        INFINITE, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };
    use windows::Win32::UI::Shell::{ABM_GETSTATE, ABM_SETSTATE, APPBARDATA, SHAppBarMessage};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowW, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible,
        SW_HIDE, SW_SHOWNA, ShowWindow,
    };
    use windows::core::{BOOL, w};

    /// True while the dock wants the taskbar hidden (between take_over and restore).
    static HIDDEN: AtomicBool = AtomicBool::new(false);

    /// A taskbar window that isn't ours: the tray-icon spy (tray.rs) shares the `Shell_TrayWnd` class.
    fn is_real_tray(h: HWND) -> bool {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
        pid != std::process::id() && is_tray(h)
    }

    fn is_tray(h: HWND) -> bool {
        let mut buf = [0u16; 64];
        let n = unsafe { GetClassNameW(h, &mut buf) }.max(0) as usize;
        matches!(
            String::from_utf16_lossy(&buf[..n]).as_str(),
            "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        )
    }

    /// Every taskbar: the main one and one per extra monitor.
    fn trays() -> Vec<HWND> {
        let mut out: Vec<HWND> = vec![];
        unsafe extern "system" fn each(h: HWND, lp: LPARAM) -> BOOL {
            if is_real_tray(h) {
                unsafe { (*(lp.0 as *mut Vec<HWND>)).push(h) };
            }
            BOOL(1)
        }
        unsafe {
            let _ = EnumWindows(Some(each), LPARAM(&mut out as *mut _ as isize));
        }
        out
    }

    /// Whether this taskbar goes while the dock runs: the one on the dock's monitor always; the
    /// others only if the user asked (otherwise those monitors would have no taskbar at all).
    fn should_hide(h: HWND) -> bool {
        use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow};
        match crate::appbar::with_settings(|d, s| (d, s.hide_other_taskbars)) {
            None | Some((_, true)) => true,
            Some((d, false)) => unsafe {
                MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST)
                    == MonitorFromWindow(HWND(d as *mut _), MONITOR_DEFAULTTONEAREST)
            },
        }
    }

    pub fn show_trays(show: bool) {
        HIDDEN.store(!show, Ordering::SeqCst);
        for h in trays() {
            let hide = !show && should_hide(h);
            unsafe {
                let _ = ShowWindow(h, if hide { SW_HIDE } else { SW_SHOWNA });
            }
        }
        if !show {
            release_autohide_slots();
        }
    }

    /// The hidden taskbar is in auto-hide mode, so Windows lists it as that edge's auto-hide bar.
    /// Browsers and Electron apps then stop a maximized window 1-2 px short of that edge, to leave
    /// room for the pointer to reveal it; with the taskbar gone that strip just looks broken. Take
    /// the hidden taskbars off the list. (Restoring the taskbar's state puts Explorer back in charge
    /// of the list; see `restore`.)
    pub fn release_autohide_slots() {
        use windows::Win32::Graphics::Gdi::{
            EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
        };
        use windows::Win32::UI::Shell::{ABM_GETAUTOHIDEBAREX, ABM_SETAUTOHIDEBAREX};
        unsafe extern "system" fn each(
            m: HMONITOR,
            _: HDC,
            _: *mut windows::Win32::Foundation::RECT,
            lp: LPARAM,
        ) -> BOOL {
            let mut mi = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if unsafe { GetMonitorInfoW(m, &mut mi) }.as_bool() {
                unsafe {
                    (*(lp.0 as *mut Vec<windows::Win32::Foundation::RECT>)).push(mi.rcMonitor)
                };
            }
            BOOL(1)
        }
        let mut monitors: Vec<windows::Win32::Foundation::RECT> = vec![];
        unsafe {
            let _ = EnumDisplayMonitors(
                None,
                None,
                Some(each),
                LPARAM(&mut monitors as *mut _ as isize),
            );
        }
        for rc in monitors {
            for edge in 0..4u32 {
                let mut d = data();
                d.uEdge = edge;
                d.rc = rc;
                let bar = HWND(unsafe { SHAppBarMessage(ABM_GETAUTOHIDEBAREX, &mut d) } as *mut _);
                if !bar.is_invalid()
                    && is_real_tray(bar)
                    && should_hide(bar)
                    && !unsafe { IsWindowVisible(bar) }.as_bool()
                {
                    let mut r = data();
                    r.hWnd = bar;
                    r.uEdge = edge;
                    r.rc = rc;
                    r.lParam = LPARAM(0);
                    unsafe { SHAppBarMessage(ABM_SETAUTOHIDEBAREX, &mut r) };
                }
            }
        }
    }

    /// The dock moved or the setting changed: hide and show the taskbars again.
    pub fn refresh() {
        if HIDDEN.load(Ordering::SeqCst) {
            show_trays(false);
        }
    }

    /// Called from the window-event hook (apps.rs) on every EVENT_OBJECT_SHOW: Explorer brought a
    /// taskbar back (Start, Win key, an Explorer restart), so hide it again.
    /// Hide any real taskbar that came back without a show event we saw (Explorer re-shows it when
    /// apps are asked to re-add tray icons). Cheap: one window enumeration.
    pub fn enforce() {
        // Restored from outside (`--restore-taskbar` while this dock still runs): stop hiding it, and
        // show the trays ourselves, since a show the other process made may have been undone by
        // `rehide_if_tray` before the note turned clean. show_trays(true) also clears HIDDEN.
        if HIDDEN.load(Ordering::SeqCst) && super::released(&super::state_file()) {
            show_trays(true);
            return;
        }
        if HIDDEN.load(Ordering::SeqCst) {
            // Explorer may sign the taskbar up as an auto-hide bar again (display changes, its own
            // restarts); check every few seconds rather than every call.
            static TICKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            if TICKS.fetch_add(1, Ordering::Relaxed) % 5 == 0 {
                release_autohide_slots();
            }
            for h in trays() {
                if unsafe { IsWindowVisible(h) }.as_bool() && should_hide(h) {
                    unsafe {
                        let _ = ShowWindow(h, SW_HIDE);
                    }
                }
            }
        }
    }

    pub fn rehide_if_tray(raw: isize) {
        let h = HWND(raw as *mut _);
        // The released check reads taskbar.json, so it goes last: only real taskbar shows reach it.
        // Released means `--restore-taskbar` brought the bar back on purpose; `enforce` then stops
        // hiding for good.
        if HIDDEN.load(Ordering::SeqCst)
            && is_real_tray(h)
            && should_hide(h)
            && !super::released(&super::state_file())
        {
            unsafe {
                let _ = ShowWindow(h, SW_HIDE);
            }
            release_autohide_slots();
        }
    }

    pub fn wait_for_exit(pid: u32) {
        unsafe {
            if let Ok(p) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
                WaitForSingleObject(p, INFINITE);
                let _ = CloseHandle(p);
            }
        }
    }

    fn data() -> APPBARDATA {
        APPBARDATA {
            cbSize: size_of::<APPBARDATA>() as u32,
            ..Default::default()
        }
    }

    pub fn get_state() -> u32 {
        let mut d = data();
        unsafe { SHAppBarMessage(ABM_GETSTATE, &mut d) as u32 }
    }

    /// ABM_SETSTATE applies to the whole taskbar, secondary monitors included.
    pub fn set_state(state: u32) {
        let mut d = data();
        if let Some(tray) = trays()
            .into_iter()
            .next()
            .or_else(|| unsafe { FindWindowW(w!("Shell_TrayWnd"), None) }.ok())
        {
            d.hWnd = tray;
        }
        d.lParam = windows::Win32::Foundation::LPARAM(state as isize);
        unsafe { SHAppBarMessage(ABM_SETSTATE, &mut d) };
    }
}

#[cfg(not(windows))]
pub mod win {
    pub fn get_state() -> u32 {
        0
    }
    pub fn set_state(_: u32) {}
    pub fn show_trays(_: bool) {}
    pub fn refresh() {}
    pub fn rehide_if_tray(_: isize) {}
    pub fn enforce() {}
    pub fn release_autohide_slots() {}
    pub fn wait_for_exit(_: u32) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_leaves_a_restore_behind() {
        let dir = std::env::temp_dir().join(format!("freedock-test-{}", std::process::id()));
        let file = dir.join("taskbar.json");
        assert_eq!(pending_restore(&file), None, "no file, nothing to restore");
        write(
            &file,
            Saved {
                state: 2,
                dirty: true,
                owner: 7,
            },
        );
        assert_eq!(
            pending_restore(&file),
            Some(2),
            "dirty file restores the saved state"
        );
        assert!(
            owned_by(&file, 7) && !owned_by(&file, 8),
            "only the owning dock's guard restores"
        );
        write(
            &file,
            Saved {
                state: 2,
                dirty: false,
                owner: 0,
            },
        );
        assert_eq!(
            pending_restore(&file),
            None,
            "clean quit, nothing to restore"
        );
        std::fs::write(&file, "{half").unwrap();
        assert_eq!(
            pending_restore(&file),
            None,
            "a broken file is ignored, never panics"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
