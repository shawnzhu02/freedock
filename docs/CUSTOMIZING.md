# Make it yours

Most of freedock changes from **Settings** (right-click the dock, the tray icon, or Ctrl+Alt+D):

- **Look:** style (glass, tinted, midnight, black, white, clear), pill or full-width bar, size,
  roundness, accent color, glass tint and blur, live glass.
- **Widgets:** add, remove and drag to reorder. The gear on a widget shows its options.
- **Moods:** saved layouts (Daily, Focus, Coding...) at the top of the Widgets tab. Each mood has its
  own widgets and can have its own look. Switch with the Mood widget, Ctrl+Alt+Shift+M or the tray
  menu.
- **General:** show and hide behavior (how close the pointer must come, delays), hotkeys, the
  Windows key, the monitor, the calendar links and the MCP server.
- Flyouts with a **Customize** button let you drag their sections into your order and hide some.

## Beyond settings: ask a coding agent

freedock is written to be changed by coding agents. Clone the repo, open it in Claude Code, Codex,
Cursor or Copilot, and ask in plain words. The agent reads `AGENTS.md` and knows where everything is.

```sh
git clone https://github.com/shawnzhu02/freedock
cd freedock
npm install
npm run tauri dev
```

Things you could ask for:

- "Add a widget that shows how many of my Steam friends are online."
- "Make the clock show the date under the time, small."
- "Add a Quick Settings tile that turns my Philips Hue lights on and off."
- "Add a 'Pomodoro' mood with only the apps, the focus timer and the clock, in the black style."
- "When I hover the weather cell, show the next 3 hours."
- "Add a widget with my company's on-call schedule from this URL."
- "Make the dock bounce an app's icon when it finishes loading."
- "Add a neon theme: dark glass with a pink glow."

When it works, `npm run tauri build` makes your own installer in `src-tauri/target/release/bundle`.
If you make something others would like, open a pull request.
