//! Settings in `%APPDATA%\freedock\settings.json`. Missing or broken files fall back to defaults,
//! and keys added in later versions take their defaults when an older file is loaded.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

impl Edge {
    /// Left and right docks stand upright.
    pub fn vertical(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    #[default]
    Pill,
    Full,
}

/// One widget on the dock, in dock order. `options` belongs to the widget (the UI's registry knows its shape).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WidgetSlot {
    pub id: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub options: serde_json::Value,
}

fn yes() -> bool {
    true
}

fn slot(id: &str) -> WidgetSlot {
    WidgetSlot {
        id: id.into(),
        enabled: true,
        options: serde_json::Value::Null,
    }
}

/// The part of the settings a mood may keep for itself. Everything else is shared by every mood.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    pub style: String,
    pub glass_live: bool,
    pub edge: Edge,
    pub visibility: String,
}

/// A look missing a field (hand-edited, or saved before the field existed) takes the default's.
impl Default for Look {
    fn default() -> Self {
        let d = Settings::default();
        Look {
            style: d.style,
            glass_live: d.glass_live,
            edge: d.edge,
            visibility: d.visibility,
        }
    }
}

impl Look {
    pub fn of(s: &Settings) -> Self {
        Look {
            style: s.style.clone(),
            glass_live: s.glass_live,
            edge: s.edge,
            visibility: s.visibility.clone(),
        }
    }
    fn apply(self, s: &mut Settings) {
        s.style = self.style;
        s.glass_live = self.glass_live;
        s.edge = self.edge;
        s.visibility = self.visibility;
    }
}

/// A saved dock layout to switch between ("Daily", "Coding", ...): its widgets, and optionally its
/// own look. The active mood's widgets live in `Settings::widgets` while it is active (that is what
/// every editor changes); they are copied back into the mood when another one is picked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mood {
    pub id: String,
    pub name: String,
    /// An emoji (or short text) shown by the mood switcher.
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub widgets: Vec<WidgetSlot>,
    /// Its own style, glass, edge and visibility; None uses the look shared by the other moods.
    #[serde(default)]
    pub look: Option<Look>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub edge: Edge,
    /// Where along its edge the dock sits: 0 = left/top end, 0.5 = centred, 1 = right/bottom end.
    pub offset: f64,
    pub shape: Shape,
    /// "glass", "tinted", "midnight", "black", "white" or "clear".
    pub style: String,
    /// Dock height and the gap to the screen edge, in logical pixels.
    pub height: u32,
    pub margin: u32,
    /// The widest a pill may grow, as a share of the screen width (it hugs its tiles below that).
    pub pill_width: f64,
    /// Corner roundness of the bar, in logical pixels.
    pub radius: u32,
    /// Highlight colour (the weekday on the date cell, progress bars), as CSS.
    pub accent: String,
    /// How strongly the style's colour tints the glass (0 clear to 1 solid) and how soft it is (blur px).
    pub glass_tint: f64,
    pub glass_blur: f64,
    /// Glass shows the live screen behind the dock (which keeps the dock out of screenshots and screen
    /// shares); off, it shows the wallpaper.
    pub glass_live: bool,
    /// App icon size in logical px, and how much an icon grows under the pointer (1 = not at all).
    pub icon_size: u32,
    pub magnify: f64,
    /// 12- or 24-hour clocks; None follows the Windows locale.
    pub hour12: Option<bool>,
    /// How flyouts and previews appear: "warp" (grow out of the dock), "fade", or "off".
    pub popup_motion: String,
    /// The global shortcut that opens the settings panel.
    pub hotkey: String,
    /// Quick Ask, and Quick Ask about the selected text.
    pub ask_hotkey: String,
    pub ask_selection_hotkey: String,
    /// Let Quick Ask use every tool (commands, edits, MCP servers) without asking. Off: it can only
    /// read files and search the web, so text it's shown (a web page, an email) can't make it act.
    pub ask_tools: bool,
    /// Snip & ask; empty (off) unless the user picks one.
    pub snip_hotkey: String,
    /// The Quick settings tiles shown, in order (Edit in the flyout changes it).
    pub quick_tiles: Vec<String>,
    /// Customised flyouts: per flyout, its sections in order, "-" before hidden ones.
    pub layouts: std::collections::BTreeMap<String, Vec<String>>,
    /// Quick to-do (adds to the to-do list); empty (off) unless the user picks one.
    pub capture_hotkey: String,
    /// Calculator flyout; empty (off) unless the user picks one.
    pub calc_hotkey: String,
    /// How the Claude cell opens a session in a folder; `{dir}` is the folder.
    pub claude_launch: String,
    /// Weather location.
    pub city: String,
    pub lat: f64,
    pub lon: f64,
    /// Step aside while a game or video is fullscreen.
    pub hide_fullscreen: bool,
    /// "always" (reserves its strip), "smart" (floats; tucks away while a window is maximized) or
    /// "autohide" (tucked away until the pointer asks).
    pub visibility: String,
    /// How long the pointer must push the screen edge before a hidden dock comes back, in ms.
    #[serde(alias = "reveal_delay")]
    pub show_delay: u32,
    /// How close (logical px) to the screen edge the pointer must come to bring a hidden dock back.
    pub show_distance: u32,
    /// How far (logical px) the pointer may wander from a shown dock before it counts as gone.
    pub hide_distance: u32,
    /// How long (ms) the pointer must stay away before the dock hides.
    pub hide_delay: u32,
    /// Show the small handle where the hidden dock sits.
    pub handle: bool,
    pub widgets: Vec<WidgetSlot>,
    /// Saved layouts (see `Mood`). Empty in files from before moods; `normalize` fills it in.
    #[serde(default)]
    pub moods: Vec<Mood>,
    #[serde(default)]
    pub active_mood: String,
    /// The look of moods without their own, kept while a mood with its own look is active.
    #[serde(default)]
    pub shared_look: Option<Look>,
    /// The monitor the dock lives on, by its Windows device name; empty = the main monitor.
    pub monitor: String,
    /// Also hide Windows' own taskbar on the monitors without the dock (off: they keep it).
    pub hide_other_taskbars: bool,
    /// A lone Windows key press opens freedock's Start instead of Windows' (a keyboard hook; off by default).
    pub win_opens_start: bool,
    /// Win+1-9 open or switch to the dock's apps (a keyboard hook; off by default).
    pub win_number_keys: bool,
    /// Where "Search the web" in Start goes; {q} is the search.
    pub web_search: String,
    /// Show live window previews when the pointer rests on an open app.
    pub previews: bool,
    /// Switch to the next mood; empty turns it off.
    pub mood_hotkey: String,
    /// The same keys plus 1-9 pick a mood by its place. Off by default: apps use Ctrl+Alt+digit.
    pub mood_number_keys: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let mut s = Self {
            edge: Edge::Bottom,
            offset: 0.5,
            shape: Shape::Pill,
            style: "glass".into(),
            height: 54,
            margin: 6,
            pill_width: 0.96,
            radius: 16,
            accent: "#ff453a".into(),
            glass_tint: 0.5,
            glass_blur: 11.0,
            icon_size: 30,
            magnify: 1.18,
            hour12: None,
            popup_motion: "warp".into(),
            hotkey: "Ctrl+Alt+D".into(),
            // No city until the user picks one (Settings > Weather); the weather cells say so.
            city: String::new(),
            lat: 0.0,
            lon: 0.0,
            hide_fullscreen: true,
            ask_hotkey: "Ctrl+Alt+Space".into(),
            ask_selection_hotkey: "Ctrl+Alt+A".into(),
            ask_tools: false,
            snip_hotkey: String::new(),
            capture_hotkey: String::new(),
            quick_tiles: [
                "wifi",
                "bluetooth",
                "airplane",
                "hotspot",
                "night",
                "dark",
                "energy",
                "project",
                "access",
                "captions",
            ]
            .map(String::from)
            .to_vec(),
            layouts: Default::default(),
            calc_hotkey: String::new(),
            claude_launch: r#"wt -d "{dir}" claude"#.into(),
            // Live glass copies the screen behind the dock; wallpaper glass is nearly free. Opt in.
            glass_live: false,
            visibility: "smart".into(),
            show_delay: 30,
            show_distance: 10,
            hide_distance: 12,
            hide_delay: 150,
            handle: true,
            widgets: [
                "start", "apps", "spacer", "weather", "tray", "system", "date", "time",
            ]
            .map(slot)
            .to_vec(),
            moods: Vec::new(),
            active_mood: String::new(),
            shared_look: None,
            // Ctrl+Alt+M alone is "insert comment" in Google Docs and Word, so Shift is added.
            mood_hotkey: "Ctrl+Alt+Shift+M".into(),
            mood_number_keys: false,
            previews: true,
            win_opens_start: false,
            monitor: String::new(),
            hide_other_taskbars: false,
            win_number_keys: false,
            web_search: "https://www.google.com/search?q={q}".into(),
        };
        normalize(&mut s);
        s
    }
}

fn widgets(ids: &[&str]) -> Vec<WidgetSlot> {
    ids.iter().map(|id| slot(id)).collect()
}

/// Make sure there is at least one mood and the active one exists. Settings from before moods become
/// the "Daily" mood (with the mood switcher added after the spacer), joined by two starters.
pub fn normalize(s: &mut Settings) {
    if s.moods.is_empty() {
        let mut daily = s.widgets.clone();
        if !daily.iter().any(|w| w.id == "mood") {
            let at = daily
                .iter()
                .position(|w| w.id == "spacer")
                .map(|i| i + 1)
                .unwrap_or(daily.len());
            daily.insert(at, slot("mood"));
        }
        s.widgets = daily.clone();
        let focus = Look {
            visibility: "autohide".into(),
            ..Look::of(s)
        };
        s.moods = vec![
            Mood {
                id: "daily".into(),
                name: "Daily".into(),
                icon: "\u{2600}\u{fe0f}".into(),
                widgets: daily,
                look: None,
            },
            Mood {
                id: "focus".into(),
                name: "Focus".into(),
                icon: "\u{1f3af}".into(),
                widgets: widgets(&["start", "apps", "spacer", "mood", "focus", "time"]),
                look: Some(focus),
            },
            Mood {
                id: "coding".into(),
                name: "Coding".into(),
                icon: "\u{1f4bb}".into(),
                widgets: widgets(&[
                    "start",
                    "apps",
                    "spacer",
                    "mood",
                    "claude-usage",
                    "sessions",
                    "cpu",
                    "ram",
                    "github",
                    "tray",
                    "system",
                    "time",
                ]),
                look: None,
            },
        ];
        s.active_mood = "daily".into();
    }
    if !s.moods.iter().any(|m| m.id == s.active_mood) {
        s.active_mood = s.moods[0].id.clone();
    }
    let active_has_look = s
        .moods
        .iter()
        .any(|m| m.id == s.active_mood && m.look.is_some());
    if s.shared_look.is_none() && !active_has_look {
        s.shared_look = Some(Look::of(s));
    }
}

/// Switch to mood `id`: the current widgets (and look, if the mood keeps its own) are stored back into
/// the mood being left, then the new mood's widgets and look are put in place. False if there is no
/// such mood; switching to the active mood changes nothing.
pub fn switch_mood(s: &mut Settings, id: &str) -> bool {
    normalize(s);
    let Some(to) = s.moods.iter().position(|m| m.id == id) else {
        return false;
    };
    if s.active_mood == id {
        return true;
    }
    let now = Look::of(s);
    let active = s.active_mood.clone();
    if let Some(from) = s.moods.iter_mut().find(|m| m.id == active) {
        from.widgets = s.widgets.clone();
        if from.look.is_some() {
            from.look = Some(now.clone());
        } else {
            s.shared_look = Some(now.clone());
        }
    }
    let m = s.moods[to].clone();
    s.widgets = m.widgets;
    m.look
        .or_else(|| s.shared_look.clone())
        .unwrap_or(now)
        .apply(s);
    s.active_mood = m.id;
    true
}

/// The mood `step` places after the active one (wrapping), e.g. 1 for "next".
pub fn mood_after(s: &Settings, step: i32) -> Option<String> {
    let n = s.moods.len() as i32;
    let i = s.moods.iter().position(|m| m.id == s.active_mood)? as i32;
    Some(s.moods[(i + step).rem_euclid(n.max(1)) as usize].id.clone())
}

pub fn dir() -> PathBuf {
    static ONCE: std::sync::Once = std::sync::Once::new();
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join("freedock");
    ONCE.call_once(|| import_old(&base.join("glassdock"), &d));
    d
}

/// freedock was called glassdock before it was published. The first time it runs, a setup kept under
/// the old name (settings, secrets, stickers, notes...) is copied over, so nothing has to be redone.
fn import_old(old: &std::path::Path, new: &std::path::Path) {
    if new.exists() || !old.is_dir() {
        return;
    }
    fn copy(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            let t = to.join(e.file_name());
            if e.file_type()?.is_dir() {
                copy(&e.path(), &t)?;
            } else {
                std::fs::copy(e.path(), t)?;
            }
        }
        Ok(())
    }
    if copy(old, new).is_err() {
        // A half copy would hide the old setup next time; start clean instead.
        let _ = std::fs::remove_dir_all(new);
    }
}

/// Set when settings.json exists but couldn't be read (locked by antivirus or a sync app). Saving is
/// refused until a later load succeeds, so the defaults used meanwhile never overwrite the real file.
static UNREADABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn load() -> Settings {
    use std::sync::atomic::Ordering;
    let file = dir().join("settings.json");
    let mut text = None;
    for attempt in 0..4 {
        match std::fs::read_to_string(&file) {
            Ok(t) => {
                text = Some(t);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) if attempt < 3 => std::thread::sleep(std::time::Duration::from_millis(60)),
            Err(e) => {
                eprintln!(
                    "settings: couldn't read {}: {e}; using defaults without saving",
                    file.display()
                );
                UNREADABLE.store(true, Ordering::SeqCst);
                let mut s = Settings::default();
                normalize(&mut s);
                return s;
            }
        }
    }
    UNREADABLE.store(false, Ordering::SeqCst);
    let mut s = match text {
        None => Settings::default(),
        Some(t) => serde_json::from_str(&t).unwrap_or_else(|_| {
            // One bad value (a hand edit, a newer version's format) mustn't cost every other setting:
            // keep a copy of the file and salvage each top-level setting that still reads.
            let _ = std::fs::copy(&file, file.with_extension("json.bad"));
            salvage(&t)
        }),
    };
    normalize(&mut s);
    s
}

/// Settings from a file that doesn't parse as a whole: defaults, with every top-level value from
/// the file that fits laid over them.
pub fn salvage(text: &str) -> Settings {
    let Ok(serde_json::Value::Object(file)) = serde_json::from_str::<serde_json::Value>(text)
    else {
        return Settings::default();
    };
    let Ok(serde_json::Value::Object(mut merged)) = serde_json::to_value(Settings::default())
    else {
        return Settings::default();
    };
    let fits = |m: &serde_json::Map<String, serde_json::Value>| {
        serde_json::from_value::<Settings>(serde_json::Value::Object(m.clone())).is_ok()
    };
    for (k, v) in file {
        let before = merged.insert(k.clone(), v.clone());
        if fits(&merged) {
            continue;
        }
        // A list (moods, widgets) with one bad entry keeps its good entries.
        if let serde_json::Value::Array(items) = v {
            let mut kept = Vec::new();
            for item in items {
                kept.push(item);
                merged.insert(k.clone(), serde_json::Value::Array(kept.clone()));
                if !fits(&merged) {
                    kept.pop();
                }
            }
            merged.insert(k.clone(), serde_json::Value::Array(kept));
            if fits(&merged) {
                continue;
            }
        }
        match before {
            Some(b) => merged.insert(k, b),
            None => merged.remove(&k),
        };
    }
    serde_json::from_value(serde_json::Value::Object(merged)).unwrap_or_default()
}

/// Written to a temp file and renamed, so a crash mid-write never leaves half a file.
pub fn save(s: &Settings) -> std::io::Result<()> {
    if UNREADABLE.load(std::sync::atomic::Ordering::SeqCst) && dir().join("settings.json").exists()
    {
        return Err(std::io::Error::other(
            "settings.json couldn't be read, so it isn't overwritten",
        ));
    }
    std::fs::create_dir_all(dir())?;
    let file = dir().join("settings.json");
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(s)?)?;
    std::fs::rename(tmp, file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flyouts_warp_unless_the_file_says_otherwise() {
        assert_eq!(Settings::default().popup_motion, "warp");
        let s: Settings = serde_json::from_str(r#"{"popup_motion":"off"}"#).unwrap();
        assert_eq!(s.popup_motion, "off");
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(old.popup_motion, "warp");
    }

    #[test]
    fn salvages_a_file_with_one_bad_value() {
        let mut good = Settings::default();
        good.city = "Lisbon".into();
        good.lat = 38.7;
        let mut v = serde_json::to_value(&good).unwrap();
        v["height"] = serde_json::json!("tall");
        v["unknown_future_key"] = serde_json::json!(1);
        let text = v.to_string();
        let s = salvage(&text);
        assert_eq!(s.city, "Lisbon");
        assert_eq!(s.lat, 38.7);
        assert_eq!(s.height, Settings::default().height);
        assert_eq!(salvage("not json").city, Settings::default().city);
    }

    #[test]
    fn round_trip_and_partial_files() {
        let s = Settings {
            edge: Edge::Top,
            shape: Shape::Full,
            ..Default::default()
        };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        let partial: Settings = serde_json::from_str(r#"{"edge":"top"}"#).unwrap();
        assert_eq!(partial.edge, Edge::Top);
        assert_eq!(partial.height, 54, "missing keys take defaults");
    }

    #[test]
    fn old_files_get_the_new_keys() {
        // A file from before the settings panel: no widgets, accent or hotkey keys.
        let old: Settings =
            serde_json::from_str(r#"{"edge":"bottom","shape":"pill","style":"midnight","height":56,"margin":8,"pill_width":0.62,"lat":1.0,"lon":2.0}"#)
                .unwrap();
        assert_eq!(old.style, "midnight");
        assert_eq!(old.hotkey, "Ctrl+Alt+D");
        assert_eq!(
            old.widgets
                .iter()
                .map(|w| w.id.as_str())
                .collect::<Vec<_>>(),
            [
                "start", "apps", "spacer", "mood", "weather", "tray", "system", "date", "time"
            ]
        );
    }

    fn ids(w: &[WidgetSlot]) -> Vec<&str> {
        w.iter().map(|w| w.id.as_str()).collect()
    }

    #[test]
    fn settings_from_before_moods_become_daily() {
        let mut s: Settings =
            serde_json::from_str(r#"{"style":"midnight","widgets":[{"id":"apps"},{"id":"time"}]}"#)
                .unwrap();
        assert!(s.moods.is_empty(), "a file without moods reads as none");
        normalize(&mut s);
        assert_eq!(s.active_mood, "daily");
        assert_eq!(
            ids(&s.widgets),
            ["apps", "time", "mood"],
            "no spacer: the switcher goes last"
        );
        assert_eq!(ids(&s.moods[0].widgets), ids(&s.widgets));
        assert_eq!(
            s.moods[1].look.as_ref().unwrap().style,
            "midnight",
            "Focus keeps the user's style"
        );
        assert_eq!(s.shared_look.as_ref().unwrap().style, "midnight");
        let again = s.clone();
        normalize(&mut s);
        assert_eq!(s, again, "normalize is idempotent");
    }

    #[test]
    fn switching_moods_keeps_each_ones_widgets_and_look() {
        let mut s = Settings::default();
        s.widgets.retain(|w| w.id != "weather"); // an edit while in Daily
        assert!(switch_mood(&mut s, "focus"));
        assert_eq!(s.visibility, "autohide", "Focus has its own look");
        assert_eq!(
            ids(&s.widgets),
            ["start", "apps", "spacer", "mood", "focus", "time"]
        );
        s.style = "black".into(); // an edit while in Focus stays with Focus
        assert!(switch_mood(&mut s, "coding"));
        assert_eq!(
            (s.visibility.as_str(), s.style.as_str()),
            ("smart", "glass"),
            "back to the shared look"
        );
        assert!(switch_mood(&mut s, "daily"));
        assert!(
            !s.widgets.iter().any(|w| w.id == "weather"),
            "Daily kept its edit"
        );
        assert!(switch_mood(&mut s, "focus"));
        assert_eq!(s.style, "black");
        assert!(!switch_mood(&mut s, "nope"));
        assert_eq!(mood_after(&s, 1).as_deref(), Some("coding"));
        assert_eq!(mood_after(&s, -2).as_deref(), Some("coding"), "wraps");
    }

    #[test]
    fn widget_order_and_options_round_trip() {
        let s = Settings {
            widgets: vec![
                WidgetSlot {
                    id: "time".into(),
                    enabled: true,
                    options: serde_json::json!({"seconds": true}),
                },
                WidgetSlot {
                    id: "cpu".into(),
                    enabled: false,
                    options: serde_json::Value::Null,
                },
            ],
            ..Default::default()
        };
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.widgets, s.widgets);
        let bare: WidgetSlot = serde_json::from_str(r#"{"id":"cpu"}"#).unwrap();
        assert!(bare.enabled, "a slot without `enabled` is on");
    }

    #[test]
    fn salvage_keeps_good_moods_and_old_looks() {
        // One mood is missing its name; the other mood and its look (saved without `visibility`) stay.
        let text = r#"{"moods": [
            {"id": "work", "name": "Work", "look": {"style": "black", "glass_live": false, "edge": "top"}},
            {"id": "broken"}
        ], "active_mood": "work", "height": "tall"}"#;
        let s = salvage(text);
        assert_eq!(s.moods.len(), 1);
        assert_eq!(s.moods[0].name, "Work");
        let look = s.moods[0].look.clone().unwrap();
        assert_eq!(look.style, "black");
        assert_eq!(look.visibility, Settings::default().visibility);
        assert_eq!(s.height, Settings::default().height, "a bad scalar still falls back");
    }
}
