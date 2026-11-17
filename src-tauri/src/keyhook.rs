//! Optional keyboard hook for two Windows-key features, both off by default:
//! - "Win key opens freedock Start": a Win press and release with no other key in between opens our
//!   Start menu instead of Windows'. Win+R, Win+E and every other shortcut pass through untouched.
//! - "Win+1-9 opens dock apps": Win+number opens or switches to the dock's Nth app.
//!
//! The hook (WH_KEYBOARD_LL) lives on its own thread and only while one of the two is on; turning both
//! off removes it. It never blocks: decisions are a few flag checks, and the actions run elsewhere.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// What the hook asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ask {
    Start,
    App(u8),
}

/// The Windows key state machine, kept apart from the hook so it can be tested.
#[derive(Debug, Default)]
pub struct Keys {
    win_down: bool,
    used: bool,
}

pub const VK_LWIN: u32 = 0x5B;
pub const VK_RWIN: u32 = 0x5C;

/// What to do with one key event: (swallow it, ask for an action, mask the Win release so Windows'
/// Start doesn't open).
pub fn step(
    k: &mut Keys,
    vk: u32,
    down: bool,
    lone_win: bool,
    numbers: bool,
) -> (bool, Option<Ask>, bool) {
    if vk == VK_LWIN || vk == VK_RWIN {
        if down {
            if !k.win_down {
                k.win_down = true;
                k.used = false;
            }
            return (false, None, false);
        }
        k.win_down = false;
        if !k.used && lone_win {
            return (false, Some(Ask::Start), true);
        }
        return (false, None, false);
    }
    if k.win_down {
        k.used = true;
        if numbers && (0x31..=0x39).contains(&vk) {
            return (true, down.then_some(Ask::App((vk - 0x30) as u8)), false);
        }
    }
    (false, None, false)
}

static LONE_WIN: AtomicBool = AtomicBool::new(false);
static NUMBERS: AtomicBool = AtomicBool::new(false);
/// The hook thread's id while it runs (0 = none).
static THREAD: AtomicU32 = AtomicU32::new(0);

/// Turn the features on or off, installing or removing the hook as needed.
pub fn apply(app: &tauri::AppHandle, lone_win: bool, numbers: bool) {
    LONE_WIN.store(lone_win, Ordering::SeqCst);
    NUMBERS.store(numbers, Ordering::SeqCst);
    #[cfg(windows)]
    win::ensure(app, lone_win || numbers);
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::sync::Mutex;
    use std::sync::OnceLock;
    use tauri::Manager;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
        MSG, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL,
        WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
    };

    static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
    static KEYS: Mutex<Keys> = Mutex::new(Keys {
        win_down: false,
        used: false,
    });

    pub fn ensure(app: &tauri::AppHandle, on: bool) {
        let _ = APP.set(app.clone());
        let running = THREAD.load(Ordering::SeqCst);
        if on && running == 0 {
            std::thread::spawn(run);
        } else if !on && running != 0 {
            unsafe {
                let _ = PostThreadMessageW(running, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }

    fn run() {
        unsafe {
            let Ok(hook) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(proc), None, 0) else {
                return;
            };
            THREAD.store(GetCurrentThreadId(), Ordering::SeqCst);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                DispatchMessageW(&msg);
            }
            let _ = UnhookWindowsHookEx(hook);
            THREAD.store(0, Ordering::SeqCst);
        }
    }

    /// A key no app uses (0xE8, "unassigned"), tapped before the lone Win release so Windows sees
    /// Win as part of a combination and doesn't open its own Start.
    fn mask() {
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
            );
        }
    }

    unsafe extern "system" fn proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let k = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            if k.flags.0 & LLKHF_INJECTED.0 == 0 {
                let down = wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == WM_SYSKEYDOWN;
                let mut keys = KEYS.lock().unwrap_or_else(|e| e.into_inner());
                // Win+L (or anything else that switches to the secure desktop) lets go of Win where
                // this hook can't see it. Check the real key state before treating a key as Win+key.
                if keys.win_down && down && k.vkCode != VK_LWIN && k.vkCode != VK_RWIN {
                    let held =
                        |vk: u32| unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000 != 0;
                    if !held(VK_LWIN) && !held(VK_RWIN) {
                        *keys = Keys::default();
                    }
                }
                let (swallow, ask, masked) = step(
                    &mut keys,
                    k.vkCode,
                    down,
                    LONE_WIN.load(Ordering::Relaxed),
                    NUMBERS.load(Ordering::Relaxed),
                );
                if masked {
                    mask();
                }
                if let (Some(ask), Some(app)) = (ask, APP.get()) {
                    let a = app.clone();
                    let _ = app.run_on_main_thread(move || match ask {
                        Ask::Start => crate::popup::toggle(&a, "start", String::new(), None),
                        Ask::App(n) => crate::app_number(&a, n as usize),
                    });
                }
                if swallow {
                    return LRESULT(1);
                }
            }
        }
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    #[allow(dead_code)]
    fn _uses(app: &tauri::AppHandle) {
        let _ = app.get_webview_window("dock");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lone_win_opens_start_and_combos_pass() {
        let mut k = Keys::default();
        assert_eq!(
            step(&mut k, VK_LWIN, true, true, false),
            (false, None, false)
        );
        assert_eq!(
            step(&mut k, VK_LWIN, false, true, false),
            (false, Some(Ask::Start), true)
        );
        // Win+R: R passes through, and the release doesn't open our Start.
        step(&mut k, VK_LWIN, true, true, false);
        assert_eq!(step(&mut k, 0x52, true, true, false), (false, None, false));
        assert_eq!(
            step(&mut k, VK_LWIN, false, true, false),
            (false, None, false)
        );
        // Off: a lone Win is left to Windows.
        step(&mut k, VK_LWIN, true, false, false);
        assert_eq!(
            step(&mut k, VK_LWIN, false, false, false),
            (false, None, false)
        );
    }

    #[test]
    fn win_numbers_open_dock_apps() {
        let mut k = Keys::default();
        step(&mut k, VK_LWIN, true, false, true);
        assert_eq!(
            step(&mut k, 0x33, true, false, true),
            (true, Some(Ask::App(3)), false)
        );
        assert_eq!(
            step(&mut k, 0x33, false, false, true),
            (true, None, false),
            "the release is swallowed too"
        );
        step(&mut k, VK_LWIN, false, false, true);
        assert_eq!(
            step(&mut k, 0x33, true, false, true),
            (false, None, false),
            "plain 3 types a 3"
        );
    }
}
