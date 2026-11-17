//! Sticker images the user picks for the Sticker widget (a GIF, PNG, WebP...). Each is copied into
//! %APPDATA%\freedock\stickers under a name made from its content, so the dock keeps working if
//! the original moves, and handed back to the dock as a data URL.

use base64::Engine;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Big enough for a long GIF, small enough that the dock stays light.
pub const MAX_BYTES: usize = 12 << 20;

/// Allowed image types: (MIME type, file extension).
const TYPES: [(&str, &str); 6] = [
    ("image/gif", "gif"),
    ("image/png", "png"),
    ("image/apng", "png"),
    ("image/webp", "webp"),
    ("image/jpeg", "jpg"),
    ("image/svg+xml", "svg"),
];

/// "data:image/gif;base64,AAAA" -> ("image/gif", bytes).
pub fn parse_data_url(url: &str) -> Result<(String, Vec<u8>), String> {
    let rest = url.strip_prefix("data:").ok_or("not a data URL")?;
    let (meta, data) = rest.split_once(',').ok_or("not a data URL")?;
    let mime = meta
        .strip_suffix(";base64")
        .ok_or("expected base64 data")?
        .to_ascii_lowercase();
    if !TYPES.iter().any(|(m, _)| *m == mime) {
        return Err("Pick a GIF, PNG, WebP, JPEG or SVG image".into());
    }
    if data.len() > MAX_BYTES / 3 * 4 + 4 {
        return Err(format!("That image is over {} MB", MAX_BYTES >> 20));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| "the image data is damaged")?;
    Ok((mime, bytes))
}

/// The stored file's name: a hash of the content plus the type's extension.
pub fn file_name(mime: &str, bytes: &[u8]) -> String {
    // FNV-1a, 64-bit: plenty to tell the user's stickers apart.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    let ext = TYPES
        .iter()
        .find(|(m, _)| *m == mime)
        .map(|(_, e)| *e)
        .unwrap_or("img");
    format!("{h:016x}.{ext}")
}

/// Only names this module made: 16 hex digits and a known extension (so no paths get through).
pub fn valid_name(name: &str) -> bool {
    let Some((stem, ext)) = name.split_once('.') else {
        return false;
    };
    stem.len() == 16
        && stem.chars().all(|c| c.is_ascii_hexdigit())
        && TYPES.iter().any(|(_, e)| *e == ext)
}

fn dir() -> Result<PathBuf, String> {
    let d = crate::settings::dir().join("stickers");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

pub fn save(data_url: &str) -> Result<String, String> {
    let (mime, bytes) = parse_data_url(data_url)?;
    if bytes.len() > MAX_BYTES {
        return Err(format!("That image is over {} MB", MAX_BYTES >> 20));
    }
    let name = file_name(&mime, &bytes);
    std::fs::write(dir()?.join(&name), &bytes).map_err(|e| e.to_string())?;
    CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(name.clone(), data_url.to_string());
    Ok(name)
}

pub fn get(name: &str) -> Result<String, String> {
    if !valid_name(name) {
        return Err("unknown sticker".into());
    }
    if let Some(u) = CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|c| c.get(name))
    {
        return Ok(u.clone());
    }
    let bytes = std::fs::read(dir()?.join(name))
        .map_err(|_| "That sticker's image is gone; pick it again in Settings")?;
    let ext = name.rsplit('.').next().unwrap_or("");
    let mime = TYPES
        .iter()
        .find(|(_, e)| *e == ext)
        .map(|(m, _)| *m)
        .unwrap_or("image/png");
    let url = format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    );
    CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(name.to_string(), url.clone());
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_urls() {
        let (m, b) = parse_data_url("data:image/gif;base64,R0lGODlh").unwrap();
        assert_eq!(m, "image/gif");
        assert_eq!(&b[..3], b"GIF");
        assert!(parse_data_url("data:text/html;base64,PGI+").is_err());
        assert!(parse_data_url("data:image/png,raw").is_err());
        assert!(parse_data_url("https://example.com/a.gif").is_err());
    }

    #[test]
    fn names() {
        let n = file_name("image/gif", b"GIF89a");
        assert!(valid_name(&n), "{n}");
        assert!(n.ends_with(".gif"));
        assert_eq!(n, file_name("image/gif", b"GIF89a"));
        assert!(!valid_name("../../secrets.bin"));
        assert!(!valid_name("0123456789abcdef.exe"));
        assert!(!valid_name("0123456789abcdeg.gif"));
    }
}
