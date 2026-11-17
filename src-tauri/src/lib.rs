//! freedock's backend: the Tauri app setup (windows, tray icon, hotkeys, background threads) and every
//! command the UI calls with `invoke`. Each feature's logic lives in its own module; the commands
//! here are thin wrappers. Adding a command: write it here (or in a module), then list it in
//! `generate_handler![...]` below. See AGENTS.md.

mod agenda;
mod appbar;
mod apps;
mod ask;
mod clipboard;
mod docker;
mod feeds;
mod files;
mod focus;
mod github;
mod glass;
mod gpu;
mod keyhook;
mod mcp;
mod media;
mod mixer;
mod notes;
mod panel;
mod ping;
mod popup;
mod projects;
mod qs;
mod quick;
mod screentime;
mod secrets;
mod sessions;
mod settings;
mod sites;
mod snip;
mod sta;
mod startmenu;
mod stats;
mod stickers;
mod taskbar;
mod thumbs;
mod tray;
mod usage;
mod visibility;
mod wallpaper;
mod webmem;
mod winsearch;

static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// Tell the UI something changed (from window procedures and hooks, which have no app handle).
pub fn app_handle() -> Option<tauri::AppHandle> {
    APP.get().cloned()
}

pub fn notify(event: &str) {
    if let Some(app) = APP.get() {
        let _ = app.emit(event, ());
    }
}

#[tauri::command]
fn wallpaper() -> wallpaper::Wallpaper {
    #[cfg(windows)]
    return wallpaper::win::current();
    #[cfg(not(windows))]
    wallpaper::Wallpaper {
        data: None,
        fit: "fill".into(),
    }
}

#[tauri::command]
fn geometry(app: tauri::AppHandle, window: Option<String>) -> Option<wallpaper::Geometry> {
    let dock = app.get_webview_window(window.as_deref().unwrap_or("dock"))?;
    #[cfg(windows)]
    return Some(wallpaper::win::geometry(dock.hwnd().ok()?.0 as isize));
    #[cfg(not(windows))]
    None
}

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, RunEvent};

#[tauri::command]
fn get_settings() -> settings::Settings {
    settings::load()
}

/// Every setting at its default, for the panel's "back to default" buttons.
#[tauri::command]
fn default_settings() -> settings::Settings {
    settings::Settings::default()
}

/// The panel changed something: apply it to the dock now, and write it to disk when `persist` is set
/// (the panel sends a preview on every slider move and a persist once the slider settles).
#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    settings: settings::Settings,
    persist: bool,
) -> Result<(), String> {
    apply_settings(&app, &settings);
    if persist {
        settings::save(&settings).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn apply_settings(app: &tauri::AppHandle, settings: &settings::Settings) {
    #[cfg(windows)]
    appbar::update_settings(settings.clone());
    hotkeys::apply(app, settings);
    keyhook::apply(app, settings.win_opens_start, settings.win_number_keys);
    let _ = app.emit("settings-changed", settings);
}

/// Win+N: the dock's Nth app, clicked (focus, minimize or launch).
fn app_number(app: &tauri::AppHandle, n: usize) {
    #[cfg(windows)]
    {
        let app = app.clone();
        std::thread::spawn(move || {
            if let Some(a) = apps::win::list().get(n.wrapping_sub(1)) {
                click_app(&app, &a.key);
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (app, n);
}

// ---- Start menu (startmenu.rs)

#[tauri::command]
async fn start_apps() -> Vec<startmenu::StartApp> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(startmenu::win::apps)
        .await
        .unwrap_or_default();
    #[cfg(not(windows))]
    vec![]
}

#[derive(serde::Serialize)]
struct StartInfo {
    pins: Vec<String>,
    recent: Vec<String>,
    user: String,
}

#[tauri::command]
fn start_state() -> StartInfo {
    let s = startmenu::load_state();
    StartInfo {
        pins: s.pins,
        recent: s.recent,
        user: std::env::var("USERNAME").unwrap_or_default(),
    }
}

/// Search the way Windows' Start does (startmenu::win::find): apps, indexed programs and files, and
/// Settings. `index: false` is the instant part; the UI asks for both and shows each as it comes.
#[tauri::command]
async fn start_find(q: String, index: Option<bool>) -> startmenu::Found {
    let index = index.unwrap_or(true);
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || startmenu::win::find(&q, index))
        .await
        .unwrap_or_default();
    #[cfg(not(windows))]
    startmenu::Found::default()
}

/// "Open file location": the file or folder selected in File Explorer.
#[tauri::command]
fn start_reveal(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.is_absolute() || !p.exists() {
        return Err("that file is gone".into());
    }
    popup::hide(&app);
    #[cfg(windows)]
    startmenu::win::reveal(&path);
    Ok(())
}

/// Open an app, Settings page, file or folder from Start (see `startmenu::allowed`). `admin` runs an
/// app or program as administrator (Ctrl+Shift+Enter, as in Windows' Start).
#[tauri::command]
fn start_open(app: tauri::AppHandle, target: String, admin: Option<bool>) -> Result<(), String> {
    if !startmenu::allowed(&target) {
        return Err("Start can't open that".into());
    }
    let admin = admin.unwrap_or(false) && !target.starts_with("ms-settings:");
    if target.starts_with(r"shell:AppsFolder\") || target.to_lowercase().ends_with(".exe") {
        if let Ok(mut s) = startmenu::load_for_change() {
            s.launched(&target);
            let _ = startmenu::save_state(&s);
        }
    }
    popup::hide(&app);
    #[cfg(windows)]
    startmenu::win::open(&target, admin);
    Ok(())
}

#[tauri::command]
fn start_pin(id: String, on: bool) -> Result<(), String> {
    if !id.starts_with(r"shell:AppsFolder\") {
        return Err("only apps can be pinned".into());
    }
    let mut s = startmenu::load_for_change()?;
    s.pin(&id, on);
    startmenu::save_state(&s)
}

#[tauri::command]
fn start_pins_order(ids: Vec<String>) -> Result<(), String> {
    let mut s = startmenu::load_for_change()?;
    let mut next: Vec<String> = ids.into_iter().filter(|i| s.pins.contains(i)).collect();
    next.extend(
        s.pins
            .iter()
            .filter(|p| !next.contains(p))
            .cloned()
            .collect::<Vec<_>>(),
    );
    s.pins = next;
    startmenu::save_state(&s)
}

/// "Search files for ..." (Explorer's search) and "Search the web for ..." (the user's search site).
#[tauri::command]
fn start_search(app: tauri::AppHandle, q: String, web: bool) -> Result<(), String> {
    popup::hide(&app);
    if web {
        let tpl = settings::load().web_search;
        let url = tpl.replace("{q}", &startmenu::url_encode(&q));
        if !url.starts_with("https://") {
            return Err("the web search link must start with https://".into());
        }
        return tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string());
    }
    #[cfg(windows)]
    startmenu::win::search_files(&q);
    Ok(())
}

/// Windows' own Start (the escape hatch in our Start's footer).
#[tauri::command]
fn start_windows(app: tauri::AppHandle) {
    popup::hide(&app);
    #[cfg(windows)]
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(120));
        apps::win::press_win();
    });
}

/// Switch to a mood (saved layout). `current` is the settings panel's latest state, when it asks, so
/// an edit it has not written yet isn't lost. No restart: the dock re-renders from the new settings.
#[tauri::command]
fn set_mood(
    app: tauri::AppHandle,
    id: String,
    current: Option<settings::Settings>,
) -> Result<(), String> {
    let mut s = current.unwrap_or_else(settings::load);
    if !settings::switch_mood(&mut s, &id) {
        return Err(format!("no mood called {id}"));
    }
    settings::save(&s).map_err(|e| e.to_string())?;
    apply_settings(&app, &s);
    let _ = app.emit("mood-changed", &s.active_mood);
    Ok(())
}

/// Switch `step` moods along (1 = next, -1 = previous), wrapping.
#[tauri::command]
fn cycle_mood(app: tauri::AppHandle, step: i32) -> Result<(), String> {
    let s = settings::load();
    match settings::mood_after(&s, step) {
        Some(id) => set_mood(app, id, None),
        None => Ok(()),
    }
}

/// Switch to the mood in place `n` (1-based), if there is one.
fn pick_mood(app: &tauri::AppHandle, n: usize) {
    if let Some(m) = settings::load().moods.get(n.wrapping_sub(1)) {
        let _ = set_mood(app.clone(), m.id.clone(), None);
    }
}

/// Open the agents' popup in a mode ("ask", "claude", "agent", ...), centred on `anchor` (dock px along its edge).
#[tauri::command]
fn popup_open(app: tauri::AppHandle, mode: String, anchor: Option<f64>, arg: Option<String>) {
    popup::toggle(&app, &mode, arg.unwrap_or_default(), anchor);
}

/// The flyout has painted the view it was opened with: show it (it stays cloaked until then).
#[tauri::command]
fn popup_ready(app: tauri::AppHandle) {
    popup::ready(&app);
}

/// One step of the flyout page's open path, written to popup.log (diagnostics).
#[tauri::command]
fn popup_trace(msg: String) {
    popup::trace(&msg);
}

/// A flyout asks to be this tall (logical px) to fit its content.
#[tauri::command]
fn popup_fit(app: tauri::AppHandle, height: f64) {
    popup::fit(&app, height);
}

#[tauri::command]
fn popup_close(app: tauri::AppHandle) {
    popup::hide(&app);
}

#[tauri::command]
fn popup_state() -> Option<popup::Mode> {
    popup::state()
}

/// Quick Ask: send a question (or follow-up) to the running Claude process.
#[tauri::command]
async fn ask_send(app: tauri::AppHandle, text: String, image: Option<bool>) -> Result<(), String> {
    // The snip goes with this message only; it was held in memory, never written anywhere.
    let img = if image == Some(true) {
        Some(snip::take().ok_or("The snip is gone; take it again")?)
    } else {
        None
    };
    tauri::async_runtime::spawn_blocking(move || ask::send(&app, &text, img))
        .await
        .map_err(|e| e.to_string())?
}

/// Snip & ask: open Windows' snipping overlay, wait (up to a minute) for the snip to land on the
/// clipboard, then open Quick Ask with it attached.
#[tauri::command]
fn snip_start(app: tauri::AppHandle) -> Result<(), String> {
    snip_begin(&app)
}

fn snip_begin(app: &tauri::AppHandle) -> Result<(), String> {
    let app = app.clone();
    #[cfg(windows)]
    {
        let seq = snip::win::sequence();
        snip::clear();
        popup::hide(&app);
        tauri_plugin_opener::open_url("ms-screenclip:", None::<&str>).map_err(|e| e.to_string())?;
        std::thread::spawn(move || {
            for _ in 0..240 {
                std::thread::sleep(std::time::Duration::from_millis(250));
                if snip::win::sequence() == seq || !snip::win::has_image() {
                    continue;
                }
                // Let the snipping tool finish putting its formats on the clipboard.
                std::thread::sleep(std::time::Duration::from_millis(150));
                let Some(b64) = snip::win::read_dib().and_then(|d| snip::from_dib(&d)) else {
                    return;
                };
                *snip::PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(b64);
                // Just a nudge: the page fetches the preview with snip_preview (events carry small payloads).
                let _ = app.emit_to("ask", "ask-image", ());
                let app2 = app.clone();
                let _ =
                    app.run_on_main_thread(move || popup::open(&app2, "ask", String::new(), None));
                return;
            }
        });
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("not supported".into())
}

/// The waiting snip as a data URL, for the preview.
#[tauri::command]
fn snip_preview() -> Option<String> {
    snip::PENDING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|b| format!("data:image/png;base64,{b}"))
}

#[tauri::command]
fn snip_clear() {
    snip::clear();
}

/// Quick Ask: end this conversation; the next question starts fresh.
#[tauri::command]
fn ask_reset() {
    ask::reset();
}

#[tauri::command]
fn claude_projects() -> Vec<ask::Project> {
    ask::projects(14)
}

#[tauri::command]
fn claude_launch(app: tauri::AppHandle, dir: String) -> Result<(), String> {
    let r = ask::launch(&settings::load().claude_launch, &dir);
    if r.is_ok() {
        popup::hide(&app);
    }
    r
}

// ---- Agenda: calendar links, local to-dos and an optional MCP server (agenda.rs, mcp.rs)

/// Today's merged view for the calendar, meeting, timeline and to-do widgets.
#[tauri::command]
async fn agenda_today() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(agenda::today)
        .await
        .map_err(|e| e.to_string())?
}

/// Add a to-do (to the MCP server when one is connected, else to the local list).
#[tauri::command]
async fn agenda_add_todo(name: String, for_date: Option<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || agenda::add_todo(&name, for_date))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn todo_toggle(id: String) -> Result<(), String> {
    agenda::todo_toggle(&id)
}

#[tauri::command]
fn todo_delete(id: String) -> Result<(), String> {
    agenda::todo_delete(&id)
}

#[derive(serde::Serialize)]
struct AgendaStatus {
    /// The connected MCP server's name.
    mcp: Option<String>,
    /// How many calendar links are saved (the links themselves never leave the backend).
    calendars: usize,
    /// https MCP servers Claude Code knows, by name, to offer in Settings.
    claude_servers: Vec<String>,
}

#[tauri::command]
fn agenda_status() -> AgendaStatus {
    AgendaStatus {
        mcp: mcp::connected(),
        calendars: agenda::ics_urls().len(),
        claude_servers: mcp::claude_servers(),
    }
}

/// Save calendar links (one per line; empty clears them). Returns how many were kept.
#[tauri::command]
fn calendars_set(links: String) -> Result<usize, String> {
    agenda::ics_set(&links)
}

/// Connect an MCP server: by name from Claude Code's config, or by its https link.
#[tauri::command]
fn mcp_connect(
    from_claude: Option<String>,
    url: Option<String>,
    name: Option<String>,
) -> Result<(), String> {
    match (from_claude, url) {
        (Some(n), _) => mcp::import(&n),
        (None, Some(u)) => mcp::set_url(&u, name.as_deref().unwrap_or("")),
        _ => Err("Pick a server or paste its link".into()),
    }
}

#[tauri::command]
fn mcp_disconnect() -> Result<(), String> {
    mcp::disconnect()
}

/// Call one of the agenda tools on the connected server (the Agent flyout uses now, add_todo and
/// search_memory).
#[tauri::command]
async fn mcp_call(tool: String, args: serde_json::Value) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || mcp::call(&tool, args))
        .await
        .map_err(|e| e.to_string())?
}

/// Open a web link from a popup (http and https only).
#[tauri::command]
fn open_link(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("only web links open".into());
    }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// Stock, index and crypto quotes for the Stocks widget (feeds.rs).
#[tauri::command]
async fn stocks_quotes(list: String) -> Vec<feeds::Quote> {
    tauri::async_runtime::spawn_blocking(move || feeds::quotes(&list))
        .await
        .unwrap_or_default()
}

/// Today's exchange rates from `base` (Currency widget).
#[tauri::command]
async fn fx_rates(base: String) -> Result<feeds::Rates, String> {
    tauri::async_runtime::spawn_blocking(move || feeds::rates(&base))
        .await
        .map_err(|e| e.to_string())?
}

/// The latest headlines of an RSS or Atom feed (Headlines widget).
#[tauri::command]
async fn news_headlines(url: String) -> Result<Vec<feeds::Headline>, String> {
    tauri::async_runtime::spawn_blocking(move || feeds::headlines(&url))
        .await
        .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
struct Photo {
    path: String,
    url: String,
}

/// The `n`th photo of a folder ("" = Pictures) as a thumbnail `size` px across (Photo frame).
#[tauri::command]
async fn photo_frame(folder: String, n: u64, size: i32) -> Option<Photo> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = feeds::photo(&folder, n)?;
        #[cfg(windows)]
        let url = apps::win::thumbnail(&path, size.clamp(48, 512))?;
        #[cfg(not(windows))]
        let url = String::new();
        Some(Photo { path, url })
    })
    .await
    .ok()
    .flatten()
}

/// The Shortcuts widget's items, with %VARIABLES% filled in.
#[tauri::command]
fn shortcuts_list(spec: String) -> Vec<feeds::Shortcut> {
    feeds::parse_shortcuts(&spec, |k| std::env::var(k).ok())
}

/// Open one shortcut: a web link in the browser, else what Start may open (startmenu::allowed).
#[tauri::command]
fn shortcut_open(target: String) -> Result<(), String> {
    if target.starts_with("https://") || target.starts_with("http://") {
        return tauri_plugin_opener::open_url(target, None::<&str>).map_err(|e| e.to_string());
    }
    if !startmenu::allowed(&target) {
        return Err("That shortcut points to something that doesn't exist.".into());
    }
    #[cfg(windows)]
    startmenu::win::open(&target, false);
    Ok(())
}

/// Move mode: the dock shows an outline and a Done cell, and any drag on it moves it.
#[tauri::command]
fn edit_mode(app: tauri::AppHandle, on: bool) {
    set_edit_mode(&app, on);
}

fn set_edit_mode(app: &tauri::AppHandle, on: bool) {
    #[cfg(windows)]
    visibility::win::EDIT.store(on, std::sync::atomic::Ordering::SeqCst);
    let _ = app.emit("edit-mode", on);
}

/// The pointer went down on the dock in move mode (or with Alt held): follow it until release, then
/// land on the nearest edge and save.
#[tauri::command]
fn drag_begin(app: tauri::AppHandle) {
    #[cfg(windows)]
    appbar::drag(move |edge, offset, monitor| {
        let mut s = settings::load();
        s.edge = edge;
        s.offset = offset;
        if let Some(m) = monitor {
            s.monitor = m;
        }
        if let Err(e) = save_settings(app.clone(), s, true) {
            eprintln!("saving the dock's new place: {e}");
        }
    });
}

#[derive(serde::Serialize)]
struct MonitorInfo {
    id: String,
    label: String,
    primary: bool,
}

/// The monitors, for the "Show dock on" setting.
#[tauri::command]
fn monitors_list() -> Vec<MonitorInfo> {
    #[cfg(windows)]
    return appbar::win::monitors()
        .into_iter()
        .enumerate()
        .map(|(i, (id, r, primary))| MonitorInfo {
            label: format!(
                "Display {} ({}x{}){}",
                i + 1,
                r.width(),
                r.height(),
                if primary { ", main" } else { "" }
            ),
            id,
            primary,
        })
        .collect();
    #[cfg(not(windows))]
    vec![]
}

#[tauri::command]
fn open_settings(app: tauri::AppHandle, at: Option<String>) {
    if let Some(at) = at {
        *SETTINGS_GOTO.lock().unwrap_or_else(|e| e.into_inner()) = Some(at);
        let _ = app.emit_to(panel::LABEL, "settings-goto", ());
    }
    panel::open(&app);
}

/// Where the next Settings opening should land ("General#weather"), set by `open_settings` and taken
/// once by the panel (on load, or on the `settings-goto` event when it's already loaded).
static SETTINGS_GOTO: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[tauri::command]
fn settings_goto() -> Option<String> {
    SETTINGS_GOTO
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
}

#[tauri::command]
fn close_settings(app: tauri::AppHandle) {
    panel::hide(&app);
}

#[tauri::command]
fn reload_dock(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("dock") {
        let _ = w.eval("location.reload()");
    }
}

/// Quitting puts the taskbar back (RunEvent::Exit runs shut_down).
#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn autostart_get(app: tauri::AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn autostart_set(app: tauri::AppHandle, on: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    if on { al.enable() } else { al.disable() }.map_err(|e| e.to_string())
}

/// The global shortcuts: one opens the settings panel. Re-registered whenever the settings change.
mod hotkeys {
    use std::sync::Mutex;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

    #[derive(Clone, Copy, PartialEq, Debug)]
    pub enum Action {
        Settings,
        Ask,
        AskSelection,
        Snip,
        Capture,
        Calc,
        /// 0 = next mood, 1-9 = the mood in that place.
        Mood(u8),
    }

    /// What is registered now: (key text, shortcut id, action).
    static CURRENT: Mutex<Vec<(String, u32, Action)>> = Mutex::new(Vec::new());

    pub fn apply(app: &tauri::AppHandle, s: &crate::settings::Settings) {
        let mut want = vec![
            (s.hotkey.clone(), Action::Settings),
            (s.ask_hotkey.clone(), Action::Ask),
            (s.ask_selection_hotkey.clone(), Action::AskSelection),
            (s.snip_hotkey.clone(), Action::Snip),
            (s.capture_hotkey.clone(), Action::Capture),
            (s.calc_hotkey.clone(), Action::Calc),
            (s.mood_hotkey.clone(), Action::Mood(0)),
        ];
        // Number keys: the mood key's modifiers plus 1-9 ("Ctrl+Alt+Shift+M" gives "Ctrl+Alt+Shift+1"...).
        if s.mood_number_keys {
            if let Some((mods, _)) = s.mood_hotkey.rsplit_once('+') {
                for n in 1..=9u8 {
                    want.push((format!("{mods}+{n}"), Action::Mood(n)));
                }
            }
        }
        let mut cur = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
        let same = cur.len() == want.len()
            && cur
                .iter()
                .zip(&want)
                .all(|((k, _, a), (wk, wa))| k == wk && a == wa);
        if same {
            return;
        }
        let gs = app.global_shortcut();
        for (k, _, _) in cur.drain(..) {
            let _ = gs.unregister(k.as_str());
        }
        for (k, action) in want {
            if k.trim().is_empty() {
                continue;
            }
            let Ok(sc) = k.parse::<Shortcut>() else {
                eprintln!("hotkey {k}: not a shortcut");
                continue;
            };
            match gs.register(sc) {
                Ok(()) => cur.push((k, sc.id(), action)),
                Err(e) => eprintln!("hotkey {k}: {e}"),
            }
        }
    }

    pub fn fire(app: &tauri::AppHandle, id: u32) {
        let action = CURRENT
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|(_, i, _)| *i == id)
            .map(|(_, _, a)| *a);
        match action {
            Some(Action::Settings) => crate::panel::toggle(app),
            Some(Action::Ask) => crate::popup::toggle(app, "ask", String::new(), None),
            Some(Action::AskSelection) => {
                // Copying the selection waits for the hotkey's keys to come up: off the hotkey thread.
                let app = app.clone();
                std::thread::spawn(move || {
                    #[cfg(windows)]
                    let text = crate::ask::selection();
                    #[cfg(not(windows))]
                    let text = String::new();
                    let app2 = app.clone();
                    let _ = app
                        .run_on_main_thread(move || crate::popup::open(&app2, "ask", text, None));
                });
            }
            Some(Action::Snip) => {
                let _ = crate::snip_begin(app);
            }
            Some(Action::Capture) => crate::popup::toggle(app, "capture", String::new(), None),
            Some(Action::Calc) => crate::popup::toggle(app, "calc", String::new(), None),
            Some(Action::Mood(0)) => {
                let _ = crate::cycle_mood(app.clone(), 1);
            }
            Some(Action::Mood(n)) => crate::pick_mood(app, n as usize),
            None => {}
        }
    }
}

/// The UI's tiles need this many logical px: the pill resizes to hug them.
#[tauri::command]
fn dock_fit(width: u32) {
    #[cfg(windows)]
    appbar::fit_content(width);
}

/// Windows' own shortcuts behind the small shell buttons: "desktop" (Win+D, show the desktop and back),
/// "taskview" (Win+Tab), "search" (Win+S) and "language" (Win+Space, next keyboard layout).
#[tauri::command]
fn shell_key(which: String) {
    #[cfg(windows)]
    {
        let vk: u16 = match which.as_str() {
            "desktop" => 0x44,
            "taskview" => 0x09,
            "search" => 0x53,
            "language" => 0x20,
            _ => return,
        };
        apps::win::press_win_with(vk);
    }
    #[cfg(not(windows))]
    let _ = which;
}

/// The keyboard language in use and how many layouts are installed.
#[tauri::command]
fn keyboard_layout() -> (String, usize) {
    #[cfg(windows)]
    return apps::win::keyboard();
    #[cfg(not(windows))]
    (String::new(), 0)
}

/// The Start button tile: the same as pressing the Windows key.
#[tauri::command]
fn start_menu() {
    #[cfg(windows)]
    apps::win::start_toggle();
}

/// The latest live-glass grab: a 24-byte header, then RGBA (see glass.rs). Empty when there is none.
#[tauri::command]
fn glass_frame(window: Option<String>) -> tauri::ipc::Response {
    tauri::ipc::Response::new(
        glass::latest(window.as_deref().unwrap_or("dock")).unwrap_or_default(),
    )
}

/// Debug aid: `freedock --snap` asks the running dock to save its glass layer as a PNG in %TEMP%
/// (live glass keeps the dock out of ordinary screenshots).
#[tauri::command]
fn glass_snap_save(data: String, window: Option<String>) -> Result<String, String> {
    use base64::Engine;
    let b64 = data
        .split_once(',')
        .map(|(_, b)| b)
        .ok_or("not a data URL")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| e.to_string())?;
    let name = match window.as_deref() {
        None | Some("dock") => "freedock-glass.png".to_string(),
        Some(w) if w.chars().all(|c| c.is_ascii_alphanumeric()) => {
            format!("freedock-glass-{w}.png")
        }
        Some(_) => return Err("bad window label".into()),
    };
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

/// Whether the dock is showing (or tucked into its handle).
#[tauri::command]
fn dock_visible() -> bool {
    #[cfg(windows)]
    return visibility::win::shown();
    #[cfg(not(windows))]
    true
}

/// Background apps' tray icons (the taskbar is hidden, so the dock shows them).
#[tauri::command]
fn tray_list() -> Vec<tray::TrayIcon> {
    #[cfg(windows)]
    return tray::win::list();
    #[cfg(not(windows))]
    vec![]
}

/// How many tray icons there are (the dock's chevron only needs the number, not every image).
#[tauri::command]
fn tray_count() -> usize {
    #[cfg(windows)]
    return tray::win::count();
    #[cfg(not(windows))]
    0
}

#[tauri::command]
fn tray_action(id: String, action: String) {
    // The app may open its own menu, which takes focus from the flyout: keep the flyout up meanwhile.
    popup::expect_app_menu();
    #[cfg(windows)]
    tray::win::act(id, action);
}

/// Windows' own flyouts: "quick" (Wi-Fi, volume, battery) or "notifications" (with the calendar).
#[tauri::command]
fn shell_flyout(which: String) {
    #[cfg(windows)]
    match which.as_str() {
        // Win+A does nothing while the taskbar is hidden; the action-center URI still opens Quick Settings.
        "quick" => {
            let _ = tauri_plugin_opener::open_url(
                "ms-actioncenter:controlcenter/&showFooter=true",
                None::<&str>,
            );
        }
        "notifications" => apps::win::press_win_with(b'N' as u16),
        _ => {}
    }
}

// ---- Screen Time

#[tauri::command]
fn screentime_watch() {
    screentime::watch();
}

/// The last `days` days, newest first, with icons for the top programs.
#[derive(serde::Serialize)]
struct StDay {
    date: String,
    total: u64,
    apps: Vec<(screentime::AppTime, Option<String>)>,
}

#[tauri::command]
async fn screentime_get(days: usize, icons: bool) -> Vec<StDay> {
    tauri::async_runtime::spawn_blocking(move || {
        screentime::get(days.min(14))
            .into_iter()
            .enumerate()
            .map(|(i, d)| StDay {
                date: d.date,
                total: d.total,
                apps: d
                    .apps
                    .into_iter()
                    .take(if i == 0 { 10 } else { 3 })
                    .map(|a| {
                        #[cfg(windows)]
                        let icon = if icons && i == 0 {
                            apps::win::icon(&a.exe)
                        } else {
                            None
                        };
                        #[cfg(not(windows))]
                        let icon = None;
                        (a, icon)
                    })
                    .collect(),
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

// ---- GitHub (through the user's gh CLI)

#[tauri::command]
async fn github_summary(fresh: bool) -> github::Summary {
    tauri::async_runtime::spawn_blocking(move || github::summary(fresh))
        .await
        .unwrap_or_default()
}

// ---- Docker containers (through the docker CLI)

#[tauri::command]
async fn docker_list() -> docker::List {
    tauri::async_runtime::spawn_blocking(docker::list)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn docker_act(id: String, action: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || docker::act(&id, &action))
        .await
        .map_err(|e| e.to_string())?
}

// ---- Site status

#[tauri::command]
async fn sites_check(list: String, fresh: bool) -> Vec<sites::Site> {
    tauri::async_runtime::spawn_blocking(move || sites::check(&list, fresh))
        .await
        .unwrap_or_default()
}

// ---- Claude Code sessions (from local transcripts)

#[tauri::command]
async fn claude_sessions() -> Vec<sessions::Session> {
    tauri::async_runtime::spawn_blocking(|| {
        sessions::list(std::time::Duration::from_secs(3 * 3600))
    })
    .await
    .unwrap_or_default()
}

// ---- Ping

#[tauri::command]
async fn ping_once(target: String) -> Result<Option<u32>, String> {
    tauri::async_runtime::spawn_blocking(move || ping::once(&target, 1500))
        .await
        .map_err(|e| e.to_string())?
}

// ---- Notes and Recycle Bin

#[tauri::command]
fn notes_get() -> Result<String, String> {
    notes::get()
}

#[tauri::command]
fn notes_set(text: String) -> Result<(), String> {
    notes::set(&text)
}

#[tauri::command]
async fn recycle_bin() -> Option<notes::Bin> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(notes::bin)
        .await
        .ok()
        .flatten();
    #[cfg(not(windows))]
    None
}

/// Shows Windows' own confirmation; nothing is deleted unless the user says yes there.
#[tauri::command]
async fn recycle_empty() -> Result<(), String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(notes::empty_bin)
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    Err("not supported".into())
}

// ---- Downloads and Screenshots

#[tauri::command]
async fn files_list(kind: String) -> Vec<files::Item> {
    tauri::async_runtime::spawn_blocking(move || files::list(&kind, 12))
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn files_thumb(kind: String, path: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || files::thumb(&kind, &path))
        .await
        .ok()
        .flatten()
}

/// Open a listed file, show it in Explorer ("reveal"), or open the folder ("folder").
#[tauri::command]
async fn files_open(kind: String, path: String, how: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || files::open(&kind, &path, &how))
        .await
        .map_err(|e| e.to_string())?
}

// ---- Focus sessions and timers

#[tauri::command]
fn focus_get() -> focus::Focus {
    focus::get()
}

/// Start a focus session, a break or a plain timer of `minutes`.
#[tauri::command]
fn focus_start(kind: String, minutes: f64, label: Option<String>) -> Result<focus::Focus, String> {
    focus::start(&kind, minutes, label.unwrap_or_default())
}

/// Pause, or resume when paused.
#[tauri::command]
fn focus_pause() -> focus::Focus {
    focus::pause()
}

#[tauri::command]
fn focus_stop() -> focus::Focus {
    focus::stop()
}

// ---- Clipboard history (memory only)

#[tauri::command]
fn clip_watch(on: bool) {
    clipboard::watch(on);
}

#[tauri::command]
fn clip_list() -> Vec<clipboard::Clip> {
    clipboard::list()
}

/// Put a history item back on the clipboard.
#[tauri::command]
fn clip_copy(id: u64) -> Result<(), String> {
    let text = clipboard::get(id).ok_or("That item is gone")?;
    #[cfg(windows)]
    ask::write_clipboard_text(&text);
    Ok(())
}

/// Ask Claude about a history item: Quick Ask opens with it as the selection.
#[tauri::command]
fn clip_ask(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let text = clipboard::get(id).ok_or("That item is gone")?;
    popup::open(&app, "ask", text, None);
    Ok(())
}

#[tauri::command]
fn clip_delete(id: Option<u64>) {
    match id {
        Some(id) => clipboard::delete(id),
        None => clipboard::clear(),
    }
}

// ---- Projects

#[tauri::command]
async fn projects_list() -> Vec<projects::Project> {
    tauri::async_runtime::spawn_blocking(projects::list)
        .await
        .unwrap_or_default()
}

/// Open a project (one from projects_list) in VS Code, a terminal, Claude Code, Explorer or GitHub.
#[tauri::command]
async fn project_open(path: String, how: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || projects::open(&path, &how))
        .await
        .map_err(|e| e.to_string())?
}

// ---- System stats

// ---- Stickers (the Sticker widget's own images)

#[tauri::command]
async fn sticker_save(data: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || stickers::save(&data))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn sticker_get(name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || stickers::get(&name))
        .await
        .map_err(|e| e.to_string())?
}

// ---- Quick settings: sound page and the rest of Windows' tiles

#[tauri::command]
async fn quick_devices() -> Vec<mixer::Device> {
    tauri::async_runtime::spawn_blocking(mixer::devices)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn quick_set_device(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || mixer::set_default(&id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn quick_apps() -> Vec<mixer::AppVolume> {
    tauri::async_runtime::spawn_blocking(mixer::apps)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn quick_set_app(key: String, volume: Option<u8>, muted: Option<bool>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || mixer::set_app(&key, volume, muted))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn quick_extra() -> qs::Extra {
    tauri::async_runtime::spawn_blocking(qs::get)
        .await
        .unwrap_or_default()
}

/// One of the extra tiles: "airplane", "hotspot", "night_light" (on/off), "project" (mode), or a
/// tool to start/stop ("captions", "magnifier", "narrator", "keyboard").
#[tauri::command]
async fn quick_set_extra(
    what: String,
    on: Option<bool>,
    mode: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        {
            let on = on.unwrap_or(false);
            match what.as_str() {
                "airplane" => qs::set_airplane(on),
                "hotspot" => qs::set_hotspot(on),
                "night_light" => qs::set_night_light(on),
                "project" => qs::set_project(mode.as_deref().unwrap_or("")),
                _ => qs::toggle_tool(&what),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (what, on, mode);
            Err("not supported".to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// GPU load and VRAM per adapter (the discrete card first).
#[tauri::command]
async fn gpu_stats() -> Vec<gpu::Gpu> {
    tauri::async_runtime::spawn_blocking(gpu::sample)
        .await
        .unwrap_or_default()
}

/// CPU, memory, network and storage; `apps` adds the busiest apps (costlier, for the flyout).
#[tauri::command]
async fn system_stats(apps: Option<bool>) -> stats::Stats {
    let apps = apps.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || stats::get(apps))
        .await
        .unwrap_or_default()
}

// ---- Claude usage

/// The 5-hour and weekly limits, at most `max_age` seconds old (a tiny probe refreshes it).
#[tauri::command]
async fn claude_usage(max_age: i64) -> Result<usage::Usage, String> {
    tauri::async_runtime::spawn_blocking(move || usage::get(max_age))
        .await
        .map_err(|e| e.to_string())?
}

// ---- Now Playing

#[tauri::command]
async fn media_now() -> Option<media::NowPlaying> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(media::win::now)
        .await
        .ok()
        .flatten();
    #[cfg(not(windows))]
    None
}

#[tauri::command]
async fn media_art() -> Option<String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(media::win::art)
        .await
        .ok()
        .flatten();
    #[cfg(not(windows))]
    None
}

#[tauri::command]
async fn media_control(action: String) -> Result<(), String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || media::win::control(&action))
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    Err("not supported".into())
}

#[tauri::command]
async fn media_seek(seconds: f64) -> Result<(), String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || media::win::seek(seconds))
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    Err("not supported".into())
}

// ---- Quick settings flyout

/// Polled by the dock every few seconds: runs off the main thread so it never stalls the UI.
#[tauri::command]
async fn quick_audio() -> Option<quick::Audio> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(quick::win::audio)
        .await
        .ok()
        .flatten();
    #[cfg(not(windows))]
    None
}

/// "ethernet", "wifi" or "none".
#[tauri::command]
async fn net_link() -> &'static str {
    tauri::async_runtime::spawn_blocking(stats::link)
        .await
        .unwrap_or("none")
}

#[tauri::command]
fn quick_dark() -> Option<bool> {
    #[cfg(windows)]
    return quick::win::dark_mode();
    #[cfg(not(windows))]
    None
}

#[tauri::command]
async fn quick_set_dark(dark: bool) -> Result<(), String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || quick::win::set_dark_mode(dark))
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    Err("not supported".into())
}

/// Polled by the dock every few seconds: runs off the main thread so it never stalls the UI.
#[tauri::command]
async fn quick_mic() -> Option<quick::Audio> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(quick::win::mic)
        .await
        .ok()
        .flatten();
    #[cfg(not(windows))]
    None
}

#[tauri::command]
fn quick_set_mic(muted: bool) -> Result<(), String> {
    #[cfg(windows)]
    return quick::win::set_mic_muted(muted);
    #[cfg(not(windows))]
    Err("not supported".into())
}

#[tauri::command]
fn quick_set_audio(volume: Option<u8>, muted: Option<bool>) -> Result<(), String> {
    #[cfg(windows)]
    return quick::win::set_volume(volume, muted);
    #[cfg(not(windows))]
    Err("not supported".into())
}

#[tauri::command]
async fn quick_networks(scan: bool) -> Vec<quick::Network> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || quick::win::networks(scan))
        .await
        .unwrap_or_default();
    #[cfg(not(windows))]
    vec![]
}

#[tauri::command]
async fn quick_connect(ssid: Option<String>) -> Result<(), String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || quick::win::connect(ssid.as_deref()))
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    Err("not supported".into())
}

#[tauri::command]
async fn quick_radios() -> quick::Radios {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(quick::win::radios)
        .await
        .unwrap_or_default();
    #[cfg(not(windows))]
    quick::Radios::default()
}

#[tauri::command]
async fn quick_set_radio(kind: String, on: bool) -> Result<(), String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || quick::win::set_radio(&kind, on))
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    Err("not supported".into())
}

#[tauri::command]
async fn quick_brightness() -> Option<u8> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(quick::win::brightness)
        .await
        .ok()
        .flatten();
    #[cfg(not(windows))]
    None
}

#[tauri::command]
async fn quick_set_brightness(level: u8) -> Result<(), String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || quick::win::set_brightness(level))
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    Err("not supported".into())
}

/// Windows settings pages the flyouts link to. Only these fixed URIs, nothing from the page.
/// Lock, sleep, sign out, restart or shut down. The flyout asks for a second click before the last
/// three; restart and shut down use Windows' own shutdown.exe (by full path) so apps get the usual
/// chance to save.
#[tauri::command]
fn power_action(app: tauri::AppHandle, action: String) -> Result<(), String> {
    popup::hide(&app);
    #[cfg(windows)]
    {
        let shutdown = |args: &[&str]| -> Result<(), String> {
            let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
            std::process::Command::new(format!(r"{root}\System32\shutdown.exe"))
                .args(args)
                .spawn()
                .map(|_| ())
                .map_err(|e| e.to_string())
        };
        match action.as_str() {
            "lock" => unsafe { windows::Win32::System::Shutdown::LockWorkStation() }
                .map_err(|e| e.to_string()),
            "sleep" => {
                // Let the flyout disappear before the machine sleeps.
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    unsafe { windows::Win32::System::Power::SetSuspendState(false, false, false) };
                });
                Ok(())
            }
            "signout" => shutdown(&["/l"]),
            "restart" => shutdown(&["/r", "/t", "0"]),
            "shutdown" => shutdown(&["/s", "/t", "0"]),
            _ => Err("unknown action".into()),
        }
    }
    #[cfg(not(windows))]
    Err(format!("{action}: Windows only"))
}

#[tauri::command]
fn open_windows(page: String) -> Result<(), String> {
    let uri = match page.as_str() {
        "quick" => "ms-actioncenter:controlcenter/&showFooter=true",
        "wifi" => "ms-settings:network-wifi",
        "bluetooth" => "ms-settings:bluetooth",
        "sound" => "ms-settings:sound",
        "display" => "ms-settings:display",
        "battery" => "ms-settings:batterysaver",
        "settings" => "ms-settings:",
        "storage" => "ms-settings:storagesense",
        "hotspot" => "ms-settings:network-mobilehotspot",
        "nightlight" => "ms-settings:nightlight",
        "accessibility" => "ms-settings:easeofaccess",
        "captions" => "ms-settings:easeofaccess-livecaptions",
        "recycle" => {
            std::process::Command::new("explorer.exe")
                .arg("shell:RecycleBinFolder")
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        "taskmgr" => {
            std::process::Command::new("taskmgr.exe")
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        "notifications" => {
            #[cfg(windows)]
            apps::win::press_win_with(b'N' as u16);
            return Ok(());
        }
        _ => return Err("unknown page".into()),
    };
    tauri_plugin_opener::open_url(uri, None::<&str>).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
struct Power {
    /// 0-100, or None on a desktop without a battery.
    percent: Option<u8>,
    charging: bool,
    plugged: bool,
    /// Windows' estimate of the time left on battery, in seconds.
    secs_left: Option<u32>,
}

#[tauri::command]
fn power() -> Power {
    #[cfg(windows)]
    {
        use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        let mut s = SYSTEM_POWER_STATUS::default();
        if unsafe { GetSystemPowerStatus(&mut s) }.is_ok() {
            return Power {
                percent: (s.BatteryLifePercent <= 100 && s.BatteryFlag & 128 == 0)
                    .then_some(s.BatteryLifePercent),
                charging: s.BatteryFlag & 8 != 0,
                plugged: s.ACLineStatus == 1,
                secs_left: (s.BatteryLifeTime != u32::MAX && s.ACLineStatus != 1)
                    .then_some(s.BatteryLifeTime),
            };
        }
    }
    Power {
        percent: None,
        charging: false,
        plugged: true,
        secs_left: None,
    }
}

/// Pins and open windows (icons come separately, from `app_icon`). Off the main thread: it opens
/// every app's process and must never stall the UI.
#[tauri::command]
async fn apps_list() -> Vec<apps::App> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(apps::win::list)
        .await
        .unwrap_or_default();
    #[cfg(not(windows))]
    vec![]
}

/// One app's icon as a PNG data URL (cached here too); the UI asks once per program.
#[tauri::command]
async fn app_icon(path: String) -> Option<String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || apps::win::icon(&path))
        .await
        .ok()
        .flatten();
    #[cfg(not(windows))]
    None
}

/// A click on an app: minimize, focus the next window, or launch.
// The app commands below list windows and read pins (processes, shortcut targets through the shell
// lane), which can block; they run off the UI thread so the dock never freezes behind them.

#[tauri::command]
async fn app_click(app_handle: tauri::AppHandle, key: String) {
    let _ = tauri::async_runtime::spawn_blocking(move || click_app(&app_handle, &key)).await;
}

fn click_app(app_handle: &tauri::AppHandle, key: &str) {
    popup::preview_end(app_handle);
    #[cfg(windows)]
    if let Some(app) = apps::win::list().into_iter().find(|a| a.key == key) {
        match apps::click(&app) {
            apps::Click::Minimize(h) => apps::win::minimize(h),
            apps::Click::Focus(h) => apps::win::focus(h),
            apps::Click::Launch => apps::win::launch(&app),
        }
    }
}

/// Pin or unpin an app on the dock (freedock's own pins; the Windows taskbar is left alone).
#[tauri::command]
async fn app_pin(key: String, on: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || pin_app(key, on))
        .await
        .map_err(|e| e.to_string())?
}

fn pin_app(key: String, on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let apps = apps::win::list();
        let app = apps.iter().find(|a| a.key == key);
        let path = app
            .and_then(|a| a.lnk.clone().or(a.exe.clone()))
            .unwrap_or_default();
        if on && path.is_empty() {
            return Err("this app can't be pinned".into());
        }
        let pins = apps::win::pins();
        let paths: Vec<String> = pins.iter().map(|p| p.0.clone()).collect();
        let targets: Vec<Option<String>> = pins.iter().map(|p| p.1.clone()).collect();
        apps::win::save_pins(&apps::pins_edit(&paths, &targets, &path, &key, on))?;
    }
    let _ = (key, on);
    Ok(())
}

/// The pinned apps in a new order (app keys as arranged on the dock).
#[tauri::command]
async fn pins_order(keys: Vec<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || order_pins(keys))
        .await
        .map_err(|e| e.to_string())?
}

fn order_pins(keys: Vec<String>) -> Result<(), String> {
    #[cfg(windows)]
    {
        let pins = apps::win::pins();
        let paths: Vec<String> = pins.iter().map(|p| p.0.clone()).collect();
        let pin_keys: Vec<String> = pins
            .iter()
            .map(|p| p.1.clone().unwrap_or_else(|| p.0.clone()).to_lowercase())
            .collect();
        apps::win::save_pins(&apps::pins_reorder(&paths, &pin_keys, &keys))?;
    }
    let _ = keys;
    Ok(())
}

/// Hover previews for an app (after the pointer rests on its icon).
#[tauri::command]
async fn preview_open(app: tauri::AppHandle, key: String, anchor: Option<f64>) {
    #[cfg(windows)]
    {
        let n = tauri::async_runtime::spawn_blocking({
            let key = key.clone();
            move || {
                apps::win::list()
                    .into_iter()
                    .find(|a| a.key == key)
                    .map(|a| a.windows.len())
                    .unwrap_or(0)
            }
        })
        .await
        .unwrap_or(0);
        popup::preview(&app, key, n, anchor);
    }
    #[cfg(not(windows))]
    let _ = (app, key, anchor);
}

#[tauri::command]
fn preview_end(app: tauri::AppHandle) {
    popup::preview_end(&app);
}

/// The preview page laid out its boxes (physical px in the flyout): draw each window's thumbnail.
#[tauri::command]
fn thumbs_place(app: tauri::AppHandle, items: Vec<(i64, i32, i32, i32, i32)>, fade: Option<bool>) {
    // A late layout (the preview already closed or replaced) must not put thumbnails back.
    #[cfg(windows)]
    if !popup::state().is_some_and(|m| m.mode == "thumbs") {
        return;
    }
    #[cfg(windows)]
    if let Some(w) = app
        .get_webview_window("ask")
        .filter(|w| w.is_visible().unwrap_or(false))
        .and_then(|w| w.hwnd().ok())
    {
        let boxes: Vec<(i64, (i32, i32, i32, i32))> = items
            .into_iter()
            .map(|(h, x, y, bw, bh)| (h, (x, y, bw, bh)))
            .collect();
        thumbs::win::place(w.0 as isize, &boxes, fade.unwrap_or(false));
    }
    #[cfg(not(windows))]
    let _ = (app, items, fade);
}

/// One window from the app menu: bring it forward or close it.
#[tauri::command]
fn app_window(hwnd: i64, action: String) {
    #[cfg(windows)]
    match action.as_str() {
        "focus" => apps::win::focus(hwnd),
        "close" => apps::win::close(hwnd),
        _ => {}
    }
    let _ = (hwnd, action);
}

/// Files dropped on an app's icon: open them with that app.
#[tauri::command]
async fn app_open_files(key: String, paths: Vec<String>) {
    #[cfg(windows)]
    let _ = tauri::async_runtime::spawn_blocking(move || {
        if let Some(app) = apps::win::list().into_iter().find(|a| a.key == key) {
            apps::win::open_with(&app, &paths);
        }
    })
    .await;
    #[cfg(not(windows))]
    let _ = (key, paths);
}

/// A middle-click: always a new window (launch again).
#[tauri::command]
async fn app_launch(key: String) {
    #[cfg(windows)]
    let _ = tauri::async_runtime::spawn_blocking(move || {
        if let Some(app) = apps::win::list().into_iter().find(|a| a.key == key) {
            apps::win::launch(&app);
        }
    })
    .await;
    #[cfg(not(windows))]
    let _ = key;
}

/// Everything that must happen on the way out, whatever the way out is.
fn shut_down() {
    #[cfg(windows)]
    appbar::unregister();
    taskbar::restore();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // `freedock --restore-taskbar`: put the taskbar back without starting the dock (a manual safety exit).
    if std::env::args().any(|a| a == "--restore-taskbar") {
        taskbar::restore();
        return;
    }
    // `freedock --guard <pid>`: started by the dock; restores the taskbar when the dock exits.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--guard-launch") {
        if let Some(pid) = args.get(i + 1).and_then(|p| p.parse().ok()) {
            taskbar::guard_launch(pid);
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--guard") {
        if let Some(pid) = args.get(i + 1).and_then(|p| p.parse().ok()) {
            taskbar::guard(pid);
        }
        return;
    }
    // A panic still puts the taskbar back (release builds abort right after this hook).
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        shut_down();
        default_hook(info);
    }));

    let app = tauri::Builder::default()
        // A second copy would read our auto-hidden taskbar as "how the user had it" and keep it hidden forever.
        // Launching it again opens the settings panel instead.
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if args.iter().any(|a| a == "--snap") {
                let _ = app.emit("glass-snap", ());
            } else {
                panel::open(app)
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, ev| {
                    if ev.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        hotkeys::fire(app, shortcut.id());
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            popup_open,
            monitors_list,
            stocks_quotes,
            fx_rates,
            news_headlines,
            photo_frame,
            shortcuts_list,
            shortcut_open,
            start_apps,
            start_find,
            start_reveal,
            start_state,
            start_open,
            start_pin,
            start_pins_order,
            start_search,
            start_windows,
            app_open_files,
            shell_key,
            keyboard_layout,
            preview_open,
            preview_end,
            thumbs_place,
            app_pin,
            pins_order,
            app_window,
            set_mood,
            cycle_mood,
            popup_fit,
            popup_ready,
            popup_trace,
            popup_close,
            popup_state,
            ask_send,
            ask_reset,
            claude_projects,
            claude_launch,
            agenda_today,
            agenda_add_todo,
            todo_toggle,
            todo_delete,
            agenda_status,
            calendars_set,
            mcp_connect,
            mcp_disconnect,
            mcp_call,
            open_link,
            glass_frame,
            glass_snap_save,
            edit_mode,
            drag_begin,
            dock_visible,
            tray_list,
            tray_count,
            tray_action,
            shell_flyout,
            power,
            claude_usage,
            system_stats,
            gpu_stats,
            files_list,
            files_thumb,
            files_open,
            github_summary,
            power_action,
            docker_list,
            docker_act,
            sites_check,
            ping_once,
            claude_sessions,
            screentime_watch,
            screentime_get,
            snip_start,
            snip_preview,
            snip_clear,
            notes_get,
            notes_set,
            recycle_bin,
            recycle_empty,
            focus_get,
            focus_start,
            focus_pause,
            focus_stop,
            clip_watch,
            clip_list,
            clip_copy,
            clip_ask,
            clip_delete,
            projects_list,
            project_open,
            media_now,
            media_art,
            media_control,
            media_seek,
            quick_audio,
            net_link,
            quick_dark,
            quick_set_dark,
            quick_mic,
            quick_set_mic,
            quick_set_audio,
            quick_networks,
            quick_connect,
            quick_radios,
            quick_set_radio,
            quick_brightness,
            quick_set_brightness,
            quick_devices,
            quick_set_device,
            quick_apps,
            quick_set_app,
            quick_extra,
            quick_set_extra,
            sticker_save,
            sticker_get,
            open_windows,
            get_settings,
            default_settings,
            save_settings,
            open_settings,
            settings_goto,
            close_settings,
            reload_dock,
            quit,
            autostart_get,
            autostart_set,
            apps_list,
            app_icon,
            app_click,
            app_launch,
            wallpaper,
            geometry,
            dock_fit,
            start_menu
        ])
        .setup(|app| {
            let _ = APP.set(app.handle().clone());
            taskbar::take_over();
            taskbar::spawn_guard();
            let dock = app
                .get_webview_window("dock")
                .expect("dock window in tauri.conf.json");
            #[cfg(windows)]
            appbar::register(dock.hwnd()?.0 as isize, settings::load());
            dock.show()?;
            let boot = settings::load();
            hotkeys::apply(app.handle(), &boot);
            keyhook::apply(app.handle(), boot.win_opens_start, boot.win_number_keys);
            // Built now, hidden: Windows can stall building a webview inside a click or hotkey callback,
            // and a ready panel opens instantly.
            popup::prepare(app.handle());
            // Screen Time keeps counting whenever its widget is in the dock, even before the dock draws it.
            if settings::load()
                .widgets
                .iter()
                .any(|w| w.id == "screen-time" && w.enabled)
            {
                screentime::watch();
            }
            let _ = focus::ON_DONE.set(Box::new(|| {
                if let Some(app) = app_handle() {
                    popup::open(&app, "focus", String::new(), None);
                }
            }));
            #[cfg(windows)]
            {
                tray::win::start();
                glass::win::start(app.handle().clone());
                visibility::win::start(app.handle().clone());
                apps::win::watch();
                // The hook only marks the list stale; this sends one "apps-changed" per burst of window events.
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    use std::sync::atomic::Ordering::SeqCst;
                    let mut titles_at = std::time::Instant::now();
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(250));
                        let titles = titles_at.elapsed() >= std::time::Duration::from_secs(2)
                            && apps::win::TITLES_CHANGED.swap(false, SeqCst);
                        if apps::win::CHANGED.swap(false, SeqCst) || titles {
                            apps::win::TITLES_CHANGED.store(false, SeqCst);
                            titles_at = std::time::Instant::now();
                            let _ = handle.emit("apps-changed", ());
                        }
                    }
                });
            }

            let quit = MenuItem::with_id(app, "quit", "Quit freedock", true, None::<&str>)?;
            let reload = MenuItem::with_id(app, "reload", "Reload dock", true, None::<&str>)?;
            let prefs = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let mover = MenuItem::with_id(app, "move", "Move dock", true, None::<&str>)?;
            let mood = MenuItem::with_id(app, "mood", "Next mood", true, None::<&str>)?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().expect("app icon"))
                .tooltip("freedock")
                .menu(&Menu::with_items(
                    app,
                    &[&prefs, &mood, &mover, &reload, &quit],
                )?)
                // Left-click toggles the settings panel; right-click shows the menu.
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, e| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = e
                    {
                        panel::toggle(tray.app_handle());
                    }
                })
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "quit" => app.exit(0),
                    "settings" => panel::open(app),
                    "move" => set_edit_mode(app, true),
                    "mood" => {
                        let _ = cycle_mood(app.clone(), 1);
                    }
                    "reload" => {
                        if let Some(w) = app.get_webview_window("dock") {
                            let _ = w.eval("location.reload()");
                        }
                    }
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building freedock");

    app.run(|_, event| {
        if let RunEvent::Exit = event {
            shut_down();
        }
    });
}
