//! The tray icons (background apps) that the hidden Windows taskbar would normally show, through
//! `systray-util` (Zebar's MIT crate): a spy `Shell_TrayWnd` window sees every app's tray-icon messages
//! and forwards them to Explorer, so both stay in sync. The UI lists the icons and sends clicks back.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TrayIcon {
    /// `systray_util::StableId` as text ("handle:uid" or a GUID).
    pub id: String,
    pub tooltip: String,
    /// PNG data URL.
    pub icon: Option<String>,
}

/// What the UI asks for, by name.
pub fn action_name(name: &str) -> Option<&'static str> {
    match name {
        "left" => Some("left"),
        "right" => Some("right"),
        "double" => Some("double"),
        "middle" => Some("middle"),
        _ => None,
    }
}

#[cfg(windows)]
pub mod win {
    use std::collections::BTreeMap;
    use std::str::FromStr;
    use std::sync::Mutex;

    use base64::Engine;
    use systray_util::{StableId, Systray, SystrayEvent, SystrayIcon, SystrayIconAction};
    use tokio::sync::mpsc;

    use super::*;

    /// The icons as the UI sees them, in the order they arrived.
    static ICONS: Mutex<BTreeMap<String, (u64, TrayIcon)>> = Mutex::new(BTreeMap::new());
    static ACTIONS: Mutex<Option<mpsc::UnboundedSender<(String, String)>>> = Mutex::new(None);

    fn png(icon: &SystrayIcon) -> Option<String> {
        let img = icon.icon_image.as_ref()?;
        let mut out = vec![];
        {
            let mut enc = png::Encoder::new(&mut out, img.width(), img.height());
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut wr = enc.write_header().ok()?;
            wr.write_image_data(img.as_raw()).ok()?;
        }
        Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(out)
        ))
    }

    fn view(icon: &SystrayIcon) -> TrayIcon {
        TrayIcon {
            id: icon.stable_id.to_string(),
            tooltip: icon.tooltip.clone(),
            icon: png(icon),
        }
    }

    pub fn list() -> Vec<TrayIcon> {
        let g = ICONS.lock().unwrap_or_else(|e| e.into_inner());
        let mut v: Vec<_> = g.values().cloned().collect();
        v.sort_by_key(|(order, _)| *order);
        // An app that re-registers under a new id (handle:uid, then a GUID) would show twice.
        let mut seen = std::collections::HashSet::new();
        v.into_iter()
            .map(|(_, t)| t)
            .filter(|t| seen.insert((t.tooltip.clone(), t.icon.clone())))
            .collect()
    }

    pub fn count() -> usize {
        list().len()
    }

    pub fn act(id: String, action: String) {
        if let Some(tx) = ACTIONS.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let _ = tx.send((id, action));
        }
    }

    /// "TaskbarCreated": the message Explorer sends after it restarts; every app re-adds its tray icons.
    fn rebroadcast() {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{
            HWND_BROADCAST, RegisterWindowMessageW, SendNotifyMessageW,
        };
        unsafe {
            let msg = RegisterWindowMessageW(windows::core::w!("TaskbarCreated"));
            if msg != 0 {
                let _ = SendNotifyMessageW(HWND_BROADCAST, msg, WPARAM(0), LPARAM(0));
            }
        }
    }

    /// Owns the `Systray` on one task: icon events in, click actions out.
    pub fn start() {
        let (tx, mut rx) = mpsc::unbounded_channel::<(String, String)>();
        *ACTIONS.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
        tauri::async_runtime::spawn(async move {
            let mut tray = match Systray::new() {
                Ok(t) => t,
                Err(e) => return eprintln!("tray icons unavailable: {e:?}"),
            };
            // The crate asks apps to re-add their icons the moment its spy window exists, before the spy
            // has moved on top, so most apps answer Explorer instead. Ask again once the spy leads.
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(1500));
                rebroadcast();
                std::thread::sleep(std::time::Duration::from_millis(300));
                crate::taskbar::win::enforce();
            });
            let mut order = 0u64;
            loop {
                tokio::select! {
                    ev = tray.events() => {
                        let Some(ev) = ev else { break };
                        let mut g = ICONS.lock().unwrap_or_else(|e| e.into_inner());
                        match ev {
                            SystrayEvent::IconAdd(i) | SystrayEvent::IconUpdate(i) => {
                                let key = i.stable_id.to_string();
                                let keep = g.get(&key).map(|(o, _)| *o).unwrap_or_else(|| { order += 1; order });
                                g.insert(key, (keep, view(&i)));
                            }
                            SystrayEvent::IconRemove(id) => { g.remove(&id.to_string()); }
                        }
                        if std::env::var_os("FREEDOCK_DEBUG_TRAY").is_some() {
                            eprintln!("tray: {:?}", g.values().map(|(_, t)| (&t.id, &t.tooltip)).collect::<Vec<_>>());
                        }
                        drop(g);
                        crate::notify("tray-changed");
                    }
                    Some((id, action)) = rx.recv() => {
                        let Ok(sid) = StableId::from_str(&id) else { continue };
                        let a = match super::action_name(&action) {
                            Some("left") => SystrayIconAction::LeftClick,
                            Some("right") => SystrayIconAction::RightClick,
                            Some("double") => SystrayIconAction::LeftDoubleClick,
                            Some("middle") => SystrayIconAction::MiddleClick,
                            _ => continue,
                        };
                        if let Err(e) = tray.send_action(&sid, &a) {
                            eprintln!("tray action {action} on {id}: {e:?}");
                        }
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_actions_pass() {
        assert_eq!(action_name("right"), Some("right"));
        assert_eq!(action_name("rm -rf"), None);
    }
}
