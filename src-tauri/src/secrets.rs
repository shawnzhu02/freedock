//! Small secrets the dock keeps: an MCP server link (which usually carries an access token) and
//! private calendar links. They are encrypted for this Windows user with DPAPI in
//! `%APPDATA%\freedock\secrets.bin`, never written to settings.json, and never put in error messages.

use std::collections::BTreeMap;

#[cfg(windows)]
mod dpapi {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CryptProtectData, CryptUnprotectData,
    };

    pub fn run(data: &[u8], protect: bool) -> Option<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out = CRYPT_INTEGER_BLOB::default();
        unsafe {
            let ok = if protect {
                CryptProtectData(
                    &input,
                    windows::core::w!("freedock"),
                    None,
                    None,
                    None,
                    0,
                    &mut out,
                )
            } else {
                CryptUnprotectData(&input, None, None, None, None, 0, &mut out)
            };
            ok.ok()?;
            let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
            let _ = LocalFree(Some(HLOCAL(out.pbData as *mut _)));
            Some(v)
        }
    }
}

#[cfg(not(windows))]
mod dpapi {
    pub fn run(_: &[u8], _: bool) -> Option<Vec<u8>> {
        None
    }
}

fn file() -> std::path::PathBuf {
    crate::settings::dir().join("secrets.bin")
}

fn load() -> BTreeMap<String, String> {
    read().unwrap_or_default()
}

/// The saved secrets, or None when secrets.bin exists but can't be read or decrypted (then nothing
/// may save over it: one new key would erase the others).
fn read() -> Option<BTreeMap<String, String>> {
    let mut m: BTreeMap<String, String> = match std::fs::read(file()) {
        Ok(b) => serde_json::from_slice(&dpapi::run(&b, false)?).ok()?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(_) => return None,
    };
    // Older versions saved one server under "<name>_mcp".
    let old: Vec<String> = m
        .keys()
        .filter(|k| k.ends_with("_mcp") && *k != "agenda_mcp")
        .cloned()
        .collect();
    for k in old {
        let url = m.remove(&k).unwrap_or_default();
        if !m.contains_key("agenda_mcp") {
            m.insert("agenda_mcp".into(), url);
            m.insert(
                "agenda_mcp_name".into(),
                k.trim_end_matches("_mcp").to_string(),
            );
        }
    }
    Some(m)
}

static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn store(map: &BTreeMap<String, String>) -> Result<(), String> {
    let plain = serde_json::to_vec(map).map_err(|e| e.to_string())?;
    let sealed = dpapi::run(&plain, true).ok_or("couldn't encrypt the secret")?;
    let f = file();
    let _ = std::fs::create_dir_all(crate::settings::dir());
    let tmp = f.with_extension("tmp");
    std::fs::write(&tmp, sealed)
        .and_then(|_| std::fs::rename(&tmp, &f))
        .map_err(|e| e.to_string())
}

pub fn get(key: &str) -> Option<String> {
    load().remove(key)
}

/// Set (Some) or remove (None) one secret.
pub fn set(key: &str, value: Option<String>) -> Result<(), String> {
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut m = read().ok_or("Your saved links (secrets.bin) couldn't be decrypted, so they aren't overwritten. Delete secrets.bin in the freedock settings folder (%APPDATA%, freedock) to start over.")?;
    match value {
        Some(v) => m.insert(key.into(), v),
        None => m.remove(key),
    };
    store(&m)
}
