# Widgets

Every widget freedock ships with, by group (61 in all). Add them in Settings > Widgets;
the gear on a widget shows its options. Generated from `src/widgets/registry.tsx` by
`npm run docs:widgets`.

To make your own: `npm run new:widget -- "Name"` (see AGENTS.md).

## Dock

| Widget | id | What it does |
|---|---|---|
| Start | `start` | Start: search every app, settings and files, pinned and recent apps. Can open Windows' Start instead. |
| Apps | `apps` | Pinned and open apps. |
| Sticker | `sticker` | Something cute on the dock: an animated cat, ghost, plant, star or blob, or your own image or GIF. Click to pet it. |
| Mood switcher | `mood` | Switch between saved dock layouts (Daily, Focus, Coding...). Scroll over it to step through. |
| Task View | `taskview` | Your open windows and desktops (Win+Tab). |
| Search | `search` | Windows search (Win+S). |
| Keyboard language | `keyboard` | The input language in use; click for the next. Hidden with only one layout. |
| Show desktop | `desktop` | A thin strip that shows the desktop and brings the windows back. Put it last. |
| Spacer | `spacer` | A gap between widgets, with a faint divider. Fill pushes everything after it to the far end of a full-width bar. |
| Background apps | `tray` | Tray icons: Discord, OneDrive, and the rest. |

## AI & agents

| Widget | id | What it does |
|---|---|---|
| Quick Ask | `ask` | One-off questions for Claude. Nothing is saved. It only reads and searches unless you turn on tools in Settings. |
| New Claude session | `claude` | Start Claude Code in a recent project. |
| Agent (MCP) | `agent` | A connected MCP server: what's next, add a to-do, search its notes (Settings > Agenda). |
| Claude sessions | `sessions` | How many Claude Code sessions are working, and which finished or need an approval (from local transcripts). |
| Claude usage | `claude-usage` | Your 5-hour and weekly Claude limits; turns amber, then red, as you near them. |

## Workflow

| Widget | id | What it does |
|---|---|---|
| Agenda | `agenda` | Your next event today (from your calendar links); click for the calendar. |
| Next meeting | `meeting` | Today's next meeting from your calendars, a countdown, and Join when it's close. |
| Day timeline | `timeline` | The next few hours as a strip with today's meetings from your calendars and a now line (horizontal docks). |
| To-dos | `todos` | How many to-dos are on today's list; click to tick them off, add more, and see what's coming up. |
| Quick to-do | `capture` | Type a to-do for today, tomorrow or next week (your local list, or your MCP server's). |
| Projects | `projects` | Your repos in Programming, latest first: open in VS Code, a terminal, Claude Code, Explorer or GitHub. |
| Clipboard history | `clipboard` | Your recent copies: search, copy back or ask Claude. Memory only; private copies (passwords) skipped. |
| Snip & ask | `snip` | Drag over part of the screen, then ask Claude about it in Quick Ask. |
| Focus timer | `focus` | Focus sessions, breaks and timers with a countdown on the dock; tells you when time's up. |
| Boston MBTA arrivals | `transit` | Boston only: next train or bus each way at your MBTA stop (Park Street by default); click for every line's next times. |

## System

| Widget | id | What it does |
|---|---|---|
| Quick settings | `system` | Wi-Fi, Bluetooth, sound, brightness and battery in one flyout. |
| CPU | `cpu` | Processor load with a live graph; click for the System flyout. |
| Memory | `ram` | RAM in use. |
| GPU | `gpu` | Graphics card load (the way Task Manager counts it) or VRAM in use, with a graph. |
| Top apps | `top` | The app using the most CPU right now; click for the top six. |
| Network | `network` | Download and upload speed. |
| Storage | `storage` | Free space on a drive. |
| Battery | `battery` | Charge, charging, and the time left on battery. |
| Ping | `ping` | Latency to a server as live bars, with jitter and loss in the tooltip; check your connection before a call. |
| Screen Time | `screen-time` | How long you've been active today, by program, and the week (kept on this PC). |
| Power | `power` | Lock, sleep, sign out, restart or shut down (the last three ask for a second click). |

## Media

| Widget | id | What it does |
|---|---|---|
| Now Playing | `now-playing` | Spotify, YouTube, any player: cover, play/pause, skip; click for the full player. |
| Volume | `volume` | Speaker volume; scroll over it to change, click to mute or open quick settings. |
| Mic | `mic` | One click mutes or unmutes your microphone (for calls too). |
| Brightness | `brightness` | Screen brightness; scroll over it to change (laptop screens). |

## Time

| Widget | id | What it does |
|---|---|---|
| Date | `date` | Weekday and date. |
| Time | `time` | The time. |
| World clock | `world-clock` | The time in another city; add it once per city. |
| Sunrise & sunset | `sun` | The next sunrise or sunset in your city. |
| Countdown | `countdown` | Days to an exam, a trip or a deadline (hours on the day); add one per date. |
| Stopwatch | `timer` | Click to start or pause; reset when paused. (For countdowns, use Focus timer.) |

## Weather

| Widget | id | What it does |
|---|---|---|
| Weather | `weather` | Now, with today's high and low; click for hourly, the week and sunrise/sunset. City is in Settings. |

## Everyday

| Widget | id | What it does |
|---|---|---|
| Stocks | `stocks` | A watchlist of stocks, indexes, crypto and currencies with today's change; click for the list (Yahoo Finance, no account). |
| Currency | `currency` | Today's exchange rate between two currencies; click to convert any amount (European Central Bank rates). |
| Headlines | `news` | The latest headlines from any news site's RSS feed, one at a time; click for the list. |
| Photo frame | `photo` | A photo from your Pictures (or any folder), changing every few minutes; scroll for another, click to open it. Stays on this PC. |
| Shortcuts | `shortcuts` | Your own buttons for folders, files, apps, web pages and Settings pages. |

## Files & tools

| Widget | id | What it does |
|---|---|---|
| Downloads | `downloads` | Your newest downloads with previews; click to open, or show in Explorer. |
| Recent files | `recent` | The files you opened most recently, with previews; click to open. |
| Screenshots | `screenshots` | Your newest screenshots as a grid; click to open. |
| Recycle Bin | `recycle` | How much is in the bin; open it, or empty it (Windows asks first). |
| Notes | `notes` | A scratch pad that saves as you type (kept on this PC). |
| Calculator | `calc` | A quick calculator: percentages (15% of 80), powers, sqrt, pi; the answer shows as you type. |

## Dev & accounts

| Widget | id | What it does |
|---|---|---|
| GitHub | `github` | Review requests, your PRs and their checks, unread notifications (uses your gh CLI login). |
| Docker | `docker` | How many containers are running; start, stop or restart them (uses the docker CLI). |
| Site status | `sites` | Whether your sites are up and how fast they answer; red when one is down. |
