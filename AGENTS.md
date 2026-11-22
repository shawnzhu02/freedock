# AGENTS.md: working on freedock

This file is for coding agents (Claude Code, Codex, Cursor, Copilot...) and for people. freedock is
meant to be changed: if a user asks you to add a widget, restyle the dock or change how something
behaves, this is the map.

## What it is

A dock that replaces the Windows taskbar while it runs. **Tauri 2**: a Rust backend
(`src-tauri/src/*.rs`) and a React + TypeScript UI (`src/`) rendered by WebView2. Three windows, each
the same web app on a different `#hash`:

| Window | Label | Built in | Shows |
|---|---|---|---|
| The dock | `dock` | `lib.rs` setup | `src/Dock.tsx`: one row of cells (widgets) |
| Flyout | `ask` | `popup.rs` | `src/popup/Popup.tsx`: one view at a time, by *mode* |
| Settings | `settings` | `panel.rs` (built when opened) | `src/settings/Panel.tsx` |

Full map: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Widget catalog: [docs/WIDGETS.md](docs/WIDGETS.md).

## Commands

```sh
npm install
npm run tauri dev          # run with hot reload (quit from the tray icon)
npm run check              # TypeScript + JS tests
cd src-tauri && cargo test # Rust tests
npm run tauri build        # installer + exe in src-tauri/target/release
npm run new:widget -- "Steam friends" --group "Dev & accounts"   # scaffold a widget
```

Run `npm run check` and `cargo test` before you finish. Both must pass.

## See your change without running the dock

`npm run tauri dev` starts a real dock, which hides the user's taskbar while it runs. To look at UI
changes, use the preview tools instead. They build the UI against a fake backend
(`scripts/preview/mock.js`), render it in headless Microsoft Edge, and never touch the running dock:

```sh
npm run preview:states -- --only weather,moon-phase   # hover sweep: contact sheet of every cell, dark and light, bottom and left edges
npm run preview:flyouts -- --only quick,media         # opens each flyout the way Rust does and reports flicker
node scripts/preview/showcase.mjs                     # the README screenshots
```

Each prints the paths of the images it wrote; open them to check your work. When a widget calls a new
command, add a sample answer to `data` in `mock.js`, or the preview shows the widget empty. Unknown
commands return `null`.

## Debug a bug

- **User data** is in `%APPDATA%\freedock\`: `settings.json`, `pins.json`, `todos.json`, `start.json`
  (launch counts), `taskbar.json` (the saved taskbar state) and `popup.log`.
- **`popup.log`** traces every flyout: open, the page saying it is ready, reveal, close and safety-net
  closes. Read it first when a flyout doesn't appear, flickers or won't close.
- **Self-test:** create an empty file named `selftest` in that folder while the dock runs. It opens
  several flyouts one after another and logs whether each came up, had focus and was uncloaked.
- **Rust errors** go to stderr, which shows in the `npm run tauri dev` terminal. Commands return
  `Result<_, String>`, and the UI gets the message as a rejected `invoke`.
- **Taskbar stuck hidden:** `freedock.exe --restore-taskbar` (a dev build is at
  `src-tauri/target/debug/freedock.exe`).
- **Reproduce first.** If you can, add a failing test next to the code (`src/lib/*.test.ts` for UI
  logic, `#[cfg(test)] mod tests` in the Rust module), fix it, and keep the test.

## Recipes

### Add a widget
1. `npm run new:widget -- "Name"` creates `src/widgets/Name.tsx` and registers it in
   `src/widgets/registry.tsx` (id, name, group, blurb, component, option defaults and fields).
2. Fill in the component. It gets `{ settings, options }` and renders one `<button className="cell">`.
3. Options: add to `defaults` and `fields` (`toggle`, `text`, `select`, `image`); Settings > Widgets
   shows them under the widget's gear. Read them from `options`.
4. Styles go in `src/dock.css`. Use the dock's CSS variables (`--accent`, `--icon`, `--radius`).

### Call Windows or the network from a widget
Write a `#[tauri::command]` in a Rust module (new file → add `mod name;` in `lib.rs`), register it in
the `generate_handler![...]` list in `lib.rs`, and call it with `invoke("name", { args })`. Anything
that blocks (files, COM, HTTP) goes in an `async fn` with `tauri::async_runtime::spawn_blocking`.
Windows-only code sits behind `#[cfg(windows)]` with a `#[cfg(not(windows))]` fallback.

### Add a flyout (a panel that opens above the dock)
1. Size: add `"mode" => (width, height)` in `size_for` in `src-tauri/src/popup.rs`.
2. View: create `src/popup/ModeView.tsx` and render it in `Popup.tsx` (`{m === "mode" && <ModeView opened={opened} />}`),
   add the mode to the `Mode` type, and to `FIT` if its height should follow its content.
3. Open it from a cell: `openAt("mode", e, vertical)` (from `src/widgets/Agents.tsx`).
4. Long flyouts: wrap sections in `useSections` (`src/popup/Sections.tsx`) and users get Customize
   (drag to reorder, hide) for free.

### Add a setting
1. Field + doc comment + default in `Settings` (`src-tauri/src/settings.rs`). Old settings files stay
   valid because missing keys take the default.
2. The same field in `src/settings.ts`.
3. A row in `src/settings/Panel.tsx` (`<Row label=... k={["field"]}>` gives it a reset button).
4. React to it in Rust in `apply_settings` (`lib.rs`) if the backend needs to know.

### Add a Quick Settings tile
Tiles live in `src/popup/Quick.tsx` (the `ALL_TILES` list) with their backends in `quick.rs` / `qs.rs`.

### Add a theme
Dock styles are `style-<name>` classes in `src/dock.css` plus an entry in the Look tab
(`src/settings/Panel.tsx`) and the `Style` type in `src/settings.ts`.

## Rules (please keep them)

- **Speed.** freedock should cost almost nothing while idle. Poll only while the dock is showing
  (`pollActive(ms, f)` from `src/lib/active.ts`, which also catches up when the dock reappears), share pollers between cells, prefer events
  over polling in Rust, and skip re-renders when data didn't change. Measure with
  `scripts/perf.ps1` before and after a change that runs all the time.
- **The taskbar must always come back.** `taskbar.rs` saves the taskbar's state before hiding it and a
  guard process restores it if the dock dies. Don't bypass `taskbar::restore()`; test quitting.
- **Privacy.** Local first. Use key-free public APIs (Open-Meteo, not keyed weather APIs). Secrets
  (tokens, private calendar links) go through `secrets.rs` (DPAPI), never into `settings.json`,
  logs or error messages. Nothing is sent anywhere the user didn't set up.
- **Never steal focus.** The dock window never activates; flyouts take focus only when the user
  opened them on purpose. Hover previews show without focus.
- **Async answers can arrive late or out of order.** A slow `invoke` started for an old prop (another
  app, an edited list) must not overwrite the new one: guard with an `alive` flag or a latest-key ref.
  Every `onKeyDown` that acts on Enter or arrows starts with `if (composing(e)) return;`
  (`src/lib/ime.ts`), so IME users can pick candidates.
- **Safety.** Commands that open things validate what they open (see `startmenu::allowed`,
  `open_link`). Power actions ask for a second click.
- Code comments say *why*, for the next person. Each Rust module starts with a `//!` header saying
  what it does and how to extend it.

## Where things are

| Thing | Rust | UI |
|---|---|---|
| Show / hide, auto-hide | `visibility.rs` | `Dock.tsx` (`tucked` class) |
| Reserving screen space, monitors | `appbar.rs` | |
| Hiding the Windows taskbar | `taskbar.rs` | |
| Apps, pins, previews, app menu | `apps.rs`, `thumbs.rs`, `sta.rs` (shell calls on one STA thread) | `widgets/Apps.tsx`, `popup/AppMenu.tsx`, `popup/Thumbs.tsx` |
| Start menu and its search | `startmenu.rs`, `winsearch.rs` (Windows Search index, Settings index, launch counts), `keyhook.rs` | `popup/StartView.tsx`, `lib/start.ts` |
| Moods (saved layouts) | `settings.rs` (`Mood`, `switch_mood`) | `lib/moods.ts`, `widgets/Mood.tsx`, Moods bar in `Panel.tsx` |
| Calendar, to-dos, MCP agenda | `agenda.rs`, `mcp.rs`, `secrets.rs` | `lib/agenda.ts`, `lib/today.ts` |
| Glass | `glass.rs`, `wallpaper.rs` | `widgets/Glass.tsx`, `widgets/GlassCard.tsx` |
| Tray icons | `tray.rs` | `widgets/Tray.tsx` |
| Quick Ask (Claude) | `ask.rs` | `popup/Popup.tsx` (`Ask`) |
