# Architecture

freedock is a [Tauri 2](https://tauri.app) app: a Rust process that owns the Windows integration and
a React + TypeScript UI drawn by WebView2.

```
            ┌──────────────────────── freedock.exe (Rust) ────────────────────────┐
 Windows ◄──┤ appbar · taskbar · visibility · apps · tray · thumbs · keyhook ...  │
            │        ▲ commands (invoke)            │ events (emit)               │
            └────────┼──────────────────────────────┼─────────────────────────────┘
                     │                              ▼
   WebView2:   dock window (Dock.tsx)   flyout window (Popup.tsx)   settings (Panel.tsx)
```

- The UI calls Rust with `invoke("command", args)`; every command is a `#[tauri::command]` listed in
  `generate_handler![...]` in `src-tauri/src/lib.rs`.
- Rust tells the UI about changes with events: `settings-changed`, `apps-changed`, `tray-changed`,
  `dock-visibility`, `popup-mode`, `popup-hidden`, `mood-changed`...
- Settings live in `%APPDATA%\freedock\settings.json` (`settings.rs`). The UI saves the whole object
  with `save_settings`; Rust applies it (`apply_settings`: appbar, hotkeys, keyboard hook) and
  broadcasts `settings-changed`, and every window re-renders.

## The dock window

`appbar.rs` registers the dock as a Windows AppBar, so with "Always show" maximized windows stop at
it. `visibility.rs` decides when it shows or tucks into its handle (Smart: tucks while a window is
maximized; Auto-hide; Always), from the pointer position and window state, at 50 Hz only while it
matters and a few times a second otherwise. The dock never takes focus (`WS_EX_NOACTIVATE`).

`Dock.tsx` renders `settings.widgets` in order. Each entry is a widget id from
`src/widgets/registry.tsx` plus its options. Cells are keyed by widget id and copy number, so switching
moods keeps the state of widgets two moods share.

## The flyout window

One window (label `ask`) shows whichever view is open: Quick Ask, Start, a widget's flyout, the app
menu, hover previews. `popup.rs` sizes and places it beside the dock (`size_for`, `position`), shows
it, and hides it when it loses focus. Views that fit their content call `popup_fit`. When it hides,
views unmount (`popup-hidden`) so nothing polls behind a closed flyout, and WebView2 is told to trim
memory (`webmem.rs`).

## The Windows taskbar

`taskbar.rs` saves the taskbar's auto-hide state to `taskbar.json` (marked dirty), turns auto-hide on
and hides the taskbar windows on the dock's display. A guard process (`--guard <pid>`) restores it if
the dock dies; a dirty file is restored at the next start; `freedock.exe --restore-taskbar` restores
it by hand.

## Data the widgets use

| Data | Source | Module |
|---|---|---|
| Open windows and pins | `EnumWindows`, WinEvent hooks, `pins.json` | `apps.rs` |
| Tray icons | the notification-area spy window | `tray.rs` |
| Weather, air quality | Open-Meteo (no key) | `src/lib/weather.ts`, `air.ts` |
| Calendar | private iCal links (rrule, time zones) | `agenda.rs` |
| To-dos | `todos.json`, or an MCP server | `agenda.rs`, `mcp.rs` |
| System stats, GPU | PDH / DXGI | `stats.rs`, `gpu.rs` |
| Media | Windows media sessions | `media.rs` |
| GitHub | the user's `gh` CLI | `github.rs` |
| Claude usage, sessions | Claude Code's local files | `usage.rs`, `sessions.rs` |

## Performance

- Pollers in the UI run only while the dock shows (`src/lib/active.ts`); animations pause while tucked.
- Rust sends app-list changes as events (title changes on a slower lane) and the UI skips re-renders
  when nothing changed. Icons are fetched once per program and cached.
- Live glass captures the screen behind the dock only when it changes, at 33 to 500 ms per frame
  depending on recent changes; wallpaper glass (the default) costs nothing.
- The settings window is created when opened and destroyed a minute after it closes.
- `scripts/perf.ps1` measures the dock's CPU and memory (all its processes) over a time window.
