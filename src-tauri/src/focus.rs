//! Focus sessions and timers. One countdown at a time, held here so the dock cell and the flyout
//! agree; when it runs out the flyout opens to say so (the cell pulses too).

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct Focus {
    /// "focus", "break" or "timer"; empty when idle.
    pub kind: String,
    /// What it's for, shown on the cell ("Deep work", "Tea").
    pub label: String,
    /// Total length in seconds.
    pub total: u64,
    /// Unix ms when it ends while running.
    pub ends_at: Option<u64>,
    /// Seconds left while paused.
    pub paused_left: Option<u64>,
    /// Unix ms when the last one finished (the cell pulses for a while).
    pub done_at: Option<u64>,
    /// Focus sessions finished today.
    pub today: u32,
    pub day: String,
}

static STATE: Mutex<Option<Focus>> = Mutex::new(None);
/// Bumped on every change, so the timer only ends the countdown it was waiting for.
static GEN: AtomicU64 = AtomicU64::new(0);
/// Wakes the timer thread when the state changes.
static WAKE: Condvar = Condvar::new();
static TIMER: OnceLock<()> = OnceLock::new();

/// One thread for the whole app: sleeps until the running countdown's end (or until something
/// changes), then ends it. Before, every start, pause and resume spawned its own sleeping thread.
fn timer_loop() {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        let end = g.as_ref().and_then(|f| f.ends_at);
        let generation = GEN.load(Ordering::SeqCst);
        match end {
            None => g = WAKE.wait(g).unwrap_or_else(|e| e.into_inner()),
            Some(e) => {
                let now = now_ms();
                if now >= e {
                    drop(g);
                    finish(generation);
                    g = STATE.lock().unwrap_or_else(|e| e.into_inner());
                } else {
                    g = WAKE
                        .wait_timeout(g, std::time::Duration::from_millis(e - now))
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                }
            }
        }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Local calendar day as "YYYY-MM-DD".
fn today() -> String {
    #[cfg(windows)]
    unsafe {
        let t = windows::Win32::System::SystemInformation::GetLocalTime();
        return format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay);
    }
    #[allow(unreachable_code)]
    String::new()
}

/// Seconds left: from the end time while running, the stored remainder while paused.
pub fn left(f: &Focus, now: u64) -> u64 {
    match (f.ends_at, f.paused_left) {
        (Some(end), _) => end.saturating_sub(now).div_ceil(1000),
        (None, Some(p)) => p,
        _ => 0,
    }
}

pub fn get() -> Focus {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let f = g.get_or_insert_with(Focus::default);
    let d = today();
    if f.day != d {
        f.day = d;
        f.today = 0;
    }
    f.clone()
}

fn set(f: Focus) -> Focus {
    // State and generation change together, so a countdown thread can't pair an old generation
    // with new state.
    {
        let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
        *g = Some(f.clone());
        GEN.fetch_add(1, Ordering::SeqCst);
    }
    WAKE.notify_all();
    crate::notify("focus-changed");
    if f.ends_at.is_some() {
        TIMER.get_or_init(|| {
            std::thread::spawn(timer_loop);
        });
    }
    f
}

pub fn start(kind: &str, minutes: f64, label: String) -> Result<Focus, String> {
    if !["focus", "break", "timer"].contains(&kind) {
        return Err("unknown kind".into());
    }
    if !(minutes > 0.0 && minutes <= 24.0 * 60.0) {
        return Err("Pick between a minute and a day".into());
    }
    let total = (minutes * 60.0).round() as u64;
    let mut f = get();
    f.kind = kind.into();
    f.label = label.chars().take(40).collect();
    f.total = total;
    f.ends_at = Some(now_ms() + total * 1000);
    f.paused_left = None;
    f.done_at = None;
    Ok(set(f))
}

pub fn pause() -> Focus {
    let mut f = get();
    if f.ends_at.is_some() {
        f.paused_left = Some(left(&f, now_ms()));
        f.ends_at = None;
    } else if let Some(p) = f.paused_left.take() {
        f.ends_at = Some(now_ms() + p * 1000);
    }
    set(f)
}

pub fn stop() -> Focus {
    let mut f = get();
    f.kind.clear();
    f.label.clear();
    f.ends_at = None;
    f.paused_left = None;
    f.done_at = None;
    set(f)
}

/// Ends the countdown started at `generation`, unless anything changed since. Checked and applied
/// under the state lock, so a start or pause landing at the same moment isn't overwritten.
fn finish(generation: u64) {
    {
        let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if GEN.load(Ordering::SeqCst) != generation {
            return;
        }
        let f = g.get_or_insert_with(Focus::default);
        let d = today();
        if f.day != d {
            f.day = d;
            f.today = 0;
        }
        if f.kind == "focus" {
            f.today += 1;
        }
        // Idle again; done_at makes the cell pulse and the flyout say it's done.
        f.kind.clear();
        f.ends_at = None;
        f.paused_left = None;
        f.done_at = Some(now_ms());
        GEN.fetch_add(1, Ordering::SeqCst);
    }
    crate::notify("focus-changed");
    if let Some(f) = ON_DONE.get() {
        f();
    }
}

/// What to do when a countdown runs out (open the flyout). Set by the app at start-up; a hook rather
/// than a direct call keeps the window code out of the unit-test binary.
pub static ON_DONE: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> = std::sync::OnceLock::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_left() {
        let mut f = Focus {
            ends_at: Some(10_500),
            ..Default::default()
        };
        assert_eq!(left(&f, 10_000), 1); // rounds a part-second up
        assert_eq!(left(&f, 20_000), 0);
        f.ends_at = None;
        f.paused_left = Some(90);
        assert_eq!(left(&f, 0), 90);
        f.paused_left = None;
        assert_eq!(left(&f, 0), 0);
    }

    #[test]
    fn rejects_odd_lengths() {
        assert!(start("focus", 0.0, String::new()).is_err());
        assert!(start("focus", 2000.0, String::new()).is_err());
        assert!(start("nap", 5.0, String::new()).is_err());
    }
}
