# Contributing

Thanks for wanting to make freedock better. New widgets, themes, fixes and docs are all welcome.

1. Fork and clone, then `npm install` and `npm run tauri dev` (needs Node 20+, Rust stable and the
   WebView2 runtime that ships with Windows 10 and 11).
2. Read [AGENTS.md](AGENTS.md): it maps the code and has recipes for widgets, flyouts and settings.
   `npm run new:widget -- "Name"` scaffolds a widget.
3. Before you open a pull request, run `npm run check` and `cd src-tauri && cargo test`, and quit the
   dock once to check the Windows taskbar comes back.
4. Keep pull requests focused: one feature or fix each. Say what you tested.

Please keep to the rules in AGENTS.md: idle cost near zero, no keys or accounts for built-in
widgets, secrets only through `secrets.rs`, and never take focus from the user's window.

Using a coding agent is fine (that's what freedock is built for); please review what it wrote.
