//! Long-lived worker threads ("lanes") with a fixed COM apartment.
//!
//! `SHELL` is a single-threaded apartment (STA) for shell calls: icons, shortcuts, the Applications
//! folder. Shell objects (IShellItemImageFactory, IShellLink, shell folders) expect an STA. Tauri's
//! blocking pool reuses threads, and the audio and radio modules initialize COM as multithreaded on
//! them, so a shell call that lands on such a thread silently runs in the wrong apartment and fails
//! for some items (it showed as missing app icons). Everything shell-related goes through `run`.
//!
//! winsearch.rs has its own lane, so a slow search never holds up an icon.

use std::sync::mpsc::{Sender, channel, sync_channel};
use std::sync::{Mutex, OnceLock};

type Job = Box<dyn FnOnce() + Send>;

pub struct Lane {
    name: &'static str,
    sta: bool,
    tx: OnceLock<Mutex<Sender<Job>>>,
}

impl Lane {
    /// A lane named `name` (its thread's name). `sta`: single-threaded apartment, else multithreaded.
    pub const fn new(name: &'static str, sta: bool) -> Self {
        Lane {
            name,
            sta,
            tx: OnceLock::new(),
        }
    }

    fn sender(&self) -> &Mutex<Sender<Job>> {
        self.tx.get_or_init(|| Mutex::new(self.spawn()))
    }

    /// Start the lane's worker thread; returns where to send it jobs.
    fn spawn(&self) -> Sender<Job> {
        {
            let (tx, rx) = channel::<Job>();
            let sta = self.sta;
            std::thread::Builder::new()
                .name(self.name.into())
                .spawn(move || {
                    #[cfg(windows)]
                    unsafe {
                        use windows::Win32::System::Com::{
                            COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, COINIT_MULTITHREADED,
                            CoInitializeEx,
                        };
                        let _ = CoInitializeEx(
                            None,
                            if sta {
                                COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE
                            } else {
                                COINIT_MULTITHREADED
                            },
                        );
                    }
                    #[cfg(not(windows))]
                    let _ = sta;
                    for job in rx {
                        // A panicking job must not take the lane down with it. This only holds
                        // where panics unwind (dev builds and tests). Release builds use
                        // panic = "abort", so there a panicking job ends the dock, and the panic
                        // hook (lib.rs) and the guard process restore the taskbar.
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                    }
                })
                .expect("start a worker thread");
            tx
        }
    }

    /// Run `f` on this lane and wait for its result. Blocks the caller, so call it from a blocking
    /// context (`spawn_blocking`), never from the UI thread. Returns None if `f` panicked or took longer
    /// than JOB_TIMEOUT.
    pub fn run<T: Send + 'static>(&self, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
        // Already on this lane (a job calling another helper that uses it): run inline, or it would
        // wait on itself forever.
        if std::thread::current().name() == Some(self.name) {
            return Some(f());
        }
        let (tx, rx) = sync_channel(1);
        let job: Job = Box::new(move || {
            let _ = tx.send(f());
        });
        self.sender()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .send(job)
            .ok()?;
        match rx.recv_timeout(JOB_TIMEOUT) {
            Ok(v) => Some(v),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // The job is stuck (a pin on an offline network share, a hung thumbnail handler).
                // Leave that thread to it and give later callers a fresh one, so one bad file can't
                // freeze every icon and the Start menu behind it.
                eprintln!(
                    "{}: a job took over {JOB_TIMEOUT:?}; starting a new lane thread",
                    self.name
                );
                *self.sender().lock().unwrap_or_else(|e| e.into_inner()) = self.spawn();
                None
            }
            Err(_) => None,
        }
    }
}

/// How long a caller waits for its job before giving up on the lane's thread.
const JOB_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

/// The shell's STA lane.
pub static SHELL: Lane = Lane::new("shell-sta", true);

/// Run `f` on the shell's STA lane (see the module notes).
pub fn run<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    SHELL.run(f)
}

#[cfg(test)]
mod tests {
    #[test]
    fn runs_jobs_in_order_on_one_thread() {
        let a = super::run(|| std::thread::current().name().map(String::from)).flatten();
        let b = super::run(|| std::thread::current().name().map(String::from)).flatten();
        assert_eq!(a.as_deref(), Some("shell-sta"));
        assert_eq!(a, b);
        assert_eq!(super::run(|| 2 + 2), Some(4));
    }

    #[test]
    fn survives_a_panicking_job() {
        assert_eq!(super::run(|| -> u8 { panic!("boom") }), None);
        assert_eq!(super::run(|| 7), Some(7));
    }

    #[test]
    fn nested_runs_do_not_deadlock() {
        assert_eq!(super::run(|| super::run(|| 5)).flatten(), Some(5));
    }
}
