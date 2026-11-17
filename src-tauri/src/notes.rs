//! The Notes scratch pad (one plain-text file, `%APPDATA%\freedock\notes.md`) and the Recycle Bin
//! readout. Emptying the bin goes through Windows' own confirmation dialog.

use serde::Serialize;

/// Notes stay small: anything past this is refused rather than truncated.
pub const MAX_NOTES: usize = 200_000;

fn path() -> std::path::PathBuf {
    crate::settings::dir().join("notes.md")
}

/// Only a missing file reads as empty. Any other error is returned, so the pad doesn't show blank
/// and then autosave over the user's notes.
pub fn get() -> Result<String, String> {
    match std::fs::read_to_string(path()) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("Couldn't read notes: {e}")),
    }
}

/// Written to a temporary file then renamed, so a crash mid-write can't leave half a file.
pub fn set(text: &str) -> Result<(), String> {
    if text.len() > MAX_NOTES {
        return Err("Notes are too long (200 KB max)".into());
    }
    let p = path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension("md.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Bin {
    pub bytes: i64,
    pub items: i64,
}

#[cfg(windows)]
pub fn bin() -> Option<Bin> {
    use windows::Win32::UI::Shell::{SHQUERYRBINFO, SHQueryRecycleBinW};
    let mut q = SHQUERYRBINFO {
        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
        ..Default::default()
    };
    unsafe { SHQueryRecycleBinW(windows::core::PCWSTR::null(), &mut q) }.ok()?;
    Some(Bin {
        bytes: q.i64Size,
        items: q.i64NumItems,
    })
}

/// Windows asks "are you sure" itself (no SHERB_NOCONFIRMATION).
#[cfg(windows)]
pub fn empty_bin() -> Result<(), String> {
    use windows::Win32::UI::Shell::SHEmptyRecycleBinW;
    unsafe { SHEmptyRecycleBinW(None, windows::core::PCWSTR::null(), 0) }.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn refuses_huge_notes() {
        let big = "x".repeat(super::MAX_NOTES + 1);
        assert!(super::set(&big).is_err());
    }
}
