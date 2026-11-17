//! Clipboard history for the Clipboard widget. Text only, kept in memory only (never written to
//! disk), at most 30 items, cleared when the widget goes away. Copies that apps mark as private
//! (password managers set ExcludeClipboardContentFromMonitorProcessing or
//! CanIncludeInClipboardHistory = 0) are never recorded, nor are Quick Ask's own selection grabs.

use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Clip {
    pub id: u64,
    pub text: String,
    /// Unix seconds.
    pub at: i64,
}

const MAX_ITEMS: usize = 30;
/// Longer copies are kept whole up to this many characters, then cut.
const MAX_CHARS: usize = 20_000;

static WATCH: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicBool = AtomicBool::new(false);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
/// Clipboard changes up to this sequence number are ours (Quick Ask's grab and restore).
pub static IGNORE_UPTO: AtomicU32 = AtomicU32::new(0);
pub static GRABBING: AtomicBool = AtomicBool::new(false);
static HISTORY: Mutex<Vec<Clip>> = Mutex::new(Vec::new());

/// Add a copy to the front; an identical earlier copy moves up instead of repeating.
pub fn push(list: &mut Vec<Clip>, text: String, at: i64, id: u64) {
    if text.trim().is_empty() {
        return;
    }
    let text: String = if text.chars().count() > MAX_CHARS {
        text.chars().take(MAX_CHARS).collect()
    } else {
        text
    };
    list.retain(|c| c.text != text);
    list.insert(0, Clip { id, text, at });
    list.truncate(MAX_ITEMS);
}

pub fn list() -> Vec<Clip> {
    HISTORY.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn get(id: u64) -> Option<String> {
    HISTORY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.text.clone())
}

pub fn delete(id: u64) {
    HISTORY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|c| c.id != id);
}

pub fn clear() {
    HISTORY.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// Turn watching on or off; off also forgets everything.
pub fn watch(on: bool) {
    WATCH.store(on, Ordering::SeqCst);
    if !on {
        clear();
        return;
    }
    #[cfg(windows)]
    if !STARTED.swap(true, Ordering::SeqCst) {
        std::thread::spawn(win::run);
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable,
        OpenClipboard, RegisterClipboardFormatW,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::core::w;

    /// The clipboard holds something its owner asked monitors and history to skip.
    fn private() -> bool {
        unsafe {
            let exclude =
                RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
            let ignore = RegisterClipboardFormatW(w!("Clipboard Viewer Ignore"));
            if IsClipboardFormatAvailable(exclude).is_ok()
                || IsClipboardFormatAvailable(ignore).is_ok()
            {
                return true;
            }
            let history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
            if IsClipboardFormatAvailable(history).is_ok() {
                // A DWORD: 0 means "keep out of history". Unreadable counts as private.
                if OpenClipboard(None).is_err() {
                    return true;
                }
                let allowed = (|| {
                    let h = GetClipboardData(history).ok()?;
                    if GlobalSize(HGLOBAL(h.0)) < 4 {
                        return None;
                    }
                    let p = GlobalLock(HGLOBAL(h.0)) as *const u32;
                    if p.is_null() {
                        return None;
                    }
                    let v = *p;
                    let _ = GlobalUnlock(HGLOBAL(h.0));
                    Some(v != 0)
                })()
                .unwrap_or(false);
                let _ = CloseClipboard();
                return !allowed;
            }
            false
        }
    }

    pub fn run() {
        let mut seen = unsafe { GetClipboardSequenceNumber() };
        loop {
            std::thread::sleep(std::time::Duration::from_millis(
                if WATCH.load(Ordering::SeqCst) {
                    400
                } else {
                    2000
                },
            ));
            if !WATCH.load(Ordering::SeqCst) {
                seen = unsafe { GetClipboardSequenceNumber() };
                continue;
            }
            let now = unsafe { GetClipboardSequenceNumber() };
            if now == seen {
                continue;
            }
            seen = now;
            if GRABBING.load(Ordering::SeqCst) || now <= IGNORE_UPTO.load(Ordering::SeqCst) {
                continue;
            }
            // Let the copying app finish putting all its formats on the clipboard.
            std::thread::sleep(std::time::Duration::from_millis(60));
            if private() {
                continue;
            }
            let Some(text) = crate::ask::read_clipboard_text() else {
                continue;
            };
            // Something new was copied between the privacy check and the read (it might be a
            // password), or history was switched off meanwhile: skip it rather than guess.
            if unsafe { GetClipboardSequenceNumber() } != now || !WATCH.load(Ordering::SeqCst) {
                continue;
            }
            let at = crate::usage::now_secs();
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            push(
                &mut HISTORY.lock().unwrap_or_else(|e| e.into_inner()),
                text,
                at,
                id,
            );
            crate::notify("clips-changed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushes_dedupes_and_caps() {
        let mut v = vec![];
        push(&mut v, "a".into(), 1, 1);
        push(&mut v, "b".into(), 2, 2);
        push(&mut v, "a".into(), 3, 3);
        assert_eq!(
            v.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(v[0].id, 3);
        push(&mut v, "   ".into(), 4, 4);
        assert_eq!(v.len(), 2);
        for i in 0..40 {
            push(&mut v, format!("x{i}"), 5, 10 + i);
        }
        assert_eq!(v.len(), MAX_ITEMS);
        assert_eq!(v[0].text, "x39");
        push(&mut v, "y".repeat(MAX_CHARS + 5), 6, 99);
        assert_eq!(v[0].text.chars().count(), MAX_CHARS);
    }
}
