import type { ComponentType } from "react";
import Transit from "./Transit";
import { STOPS } from "../lib/mbta";
import Sticker, { BUILTIN } from "./Sticker";
import type { Settings } from "../settings";
import { AgendaCell, AgentCell, AskCell, ClaudeCell } from "./Agents";
import Apps from "./Apps";
import { DateCell, TimeCell } from "./Clock";
import Start from "./Start";
import System from "./System";
import Tray from "./Tray";
import Meeting from "./Meeting";
import NowPlaying from "./NowPlaying";
import ClaudeUsage from "./ClaudeUsage";
import { CpuCell, GpuCell, NetCell, RamCell, StorageCell, TopCell } from "./Stats";
import WeatherCell, { SunCell } from "./Weather";
import Projects from "./Projects";
import Clipboard from "./Clipboard";
import FocusCell from "./Focus";
import Capture from "./Capture";
import WorldClock from "./WorldClock";
import { BatteryCell, BrightnessCell, MicCell, VolumeCell } from "./Controls";
import { DownloadsCell, RecentCell, ScreenshotsCell } from "./Files";
import { NotesCell, RecycleCell } from "./Notes";
import DockerCell from "./Docker";
import PowerCell from "./Power";
import Countdown from "./Countdown";
import TodosCell from "./Todos";
import SitesCell from "./Sites";
import CalcCell from "./Calc";
import PingCell from "./Ping";
import SessionsCell from "./Sessions";
import GitHubCell from "./GitHub";
import Snip from "./Snip";
import ScreenTime from "./ScreenTime";
import Stopwatch from "./Stopwatch";
import Timeline from "./Timeline";
import { ZONES } from "../lib/zones";
import MoodCell from "./Mood";
import Spacer from "./Spacer";
import { CurrencyCell, HeadlinesCell, PhotoCell, ShortcutsCell, StocksCell } from "./Everyday";
import { DEFAULT_FEED, DEFAULT_SHORTCUTS, DEFAULT_SYMBOLS } from "../lib/feeds";
import { Keyboard, Search, ShowDesktop, TaskView } from "./Shell";
import MoonPhaseCell from "./MoonPhase";

export type Options = Record<string, unknown>;
export type WidgetProps = { settings: Settings; options: Options };

/** One option a widget offers in the panel (its gear). */
export type OptionField =
  | { key: string; label: string; type: "toggle" }
  | { key: string; label: string; type: "text"; placeholder?: string }
  | { key: string; label: string; type: "select"; choices: [string, string][] }
  /** An image the user picks; stored by the dock (stickers.rs), the option holds its file name. */
  | { key: string; label: string; type: "image" };

export type WidgetDef = {
  id: string;
  name: string;
  group: string;
  blurb: string;
  /** Missing = planned ("soon" in the picker). */
  component?: ComponentType<WidgetProps>;
  defaults?: Options;
  fields?: OptionField[];
  /** Can be added more than once (spacers, world clocks). */
  multi?: boolean;
};

export const GROUPS = ["Dock", "AI & agents", "Workflow", "System", "Media", "Time", "Weather", "Everyday", "Files & tools", "Dev & accounts"];

/** Fahrenheit where people use it (the US and a few others), Celsius everywhere else. */
const FAHRENHEIT = typeof navigator !== "undefined" && /-(US|LR|MM|BS|BZ|KY|PW)$/i.test(navigator.language);

export const WIDGETS: WidgetDef[] = [
  // Dock basics
  { id: "start", name: "Start", group: "Dock", blurb: "Start: search every app, settings and files, pinned and recent apps. Can open Windows' Start instead.", component: Start, defaults: { opens: "freedock" }, fields: [{ key: "opens", label: "Opens", type: "select", choices: [["freedock", "freedock Start"], ["windows", "Windows Start"]] }] },
  { id: "apps", name: "Apps", group: "Dock", blurb: "Pinned and open apps.", component: Apps },
  {
    id: "sticker", name: "Sticker", group: "Dock", multi: true, component: Sticker,
    blurb: "Something cute on the dock: an animated cat, ghost, plant, star or blob, or your own image or GIF. Click to pet it.",
    defaults: { kind: "cat", size: "medium", click: "pet", bare: true, image: "", link: "" },
    fields: [
      { key: "kind", label: "Character", type: "select", choices: [...BUILTIN, ["custom", "My own image or GIF"]] },
      { key: "image", label: "Image (for My own)", type: "image" },
      { key: "size", label: "Size", type: "select", choices: [["small", "Small"], ["medium", "Medium"], ["large", "Large"]] },
      { key: "click", label: "When clicked", type: "select", choices: [["pet", "Hearts and a hop"], ["bounce", "Hop"], ["link", "Open a link"], ["none", "Nothing"]] },
      { key: "link", label: "Link (for Open a link)", type: "text", placeholder: "https://…" },
      { key: "bare", label: "No cell background", type: "toggle" },
    ],
  },
  {
    id: "mood",
    name: "Mood switcher",
    group: "Dock",
    blurb: "Switch between saved dock layouts (Daily, Focus, Coding...). Scroll over it to step through.",
    component: MoodCell,
    defaults: { name: false },
    fields: [{ key: "name", label: "Show the mood's name", type: "toggle" }],
  },
  { id: "taskview", name: "Task View", group: "Dock", blurb: "Your open windows and desktops (Win+Tab).", component: TaskView },
  { id: "search", name: "Search", group: "Dock", blurb: "Windows search (Win+S).", component: Search },
  { id: "keyboard", name: "Keyboard language", group: "Dock", blurb: "The input language in use; click for the next. Hidden with only one layout.", component: Keyboard },
  { id: "desktop", name: "Show desktop", group: "Dock", blurb: "A thin strip that shows the desktop and brings the windows back. Put it last.", component: ShowDesktop },
  {
    id: "spacer", name: "Spacer", group: "Dock", blurb: "A gap between widgets, with a faint divider. Fill pushes everything after it to the far end of a full-width bar.", component: Spacer, multi: true,
    defaults: { size: "auto", look: "line" },
    fields: [
      { key: "size", label: "Size", type: "select", choices: [["auto", "Auto (fill a full-width bar, medium on a pill)"], ["small", "Small"], ["medium", "Medium"], ["large", "Large"], ["fill", "Fill the bar (full-width bar)"]] },
      { key: "look", label: "Look", type: "select", choices: [["line", "Line"], ["dot", "Dot"], ["none", "Empty"]] },
    ],
  },
  { id: "tray", name: "Background apps", group: "Dock", blurb: "Tray icons: Discord, OneDrive, and the rest.", component: Tray, defaults: { inline: "0" }, fields: [{ key: "inline", label: "On the dock", type: "select", choices: [["0", "None (chevron only)"], ["2", "2 icons"], ["4", "4 icons"], ["6", "6 icons"], ["8", "8 icons"]] }] },
  { id: "system", name: "Quick settings", group: "System", blurb: "Wi-Fi, Bluetooth, sound, brightness and battery in one flyout.", component: System },
  {
    id: "date", name: "Date", group: "Time", blurb: "Weekday and date.", component: DateCell,
    defaults: { weekday: true }, fields: [{ key: "weekday", label: "Show the weekday", type: "toggle" }],
  },
  {
    id: "time", name: "Time", group: "Time", blurb: "The time.", component: TimeCell,
    defaults: { seconds: false, face: "digital" },
    fields: [
      { key: "face", label: "Face", type: "select", choices: [["digital", "Digital"], ["analog", "Analog"], ["both", "Analog and digital"]] },
      { key: "seconds", label: "Show seconds", type: "toggle" },
    ],
  },

  // Planned: each becomes one file plus a `component` here.
  { id: "ask", name: "Quick Ask", group: "AI & agents", blurb: "One-off questions for Claude. Nothing is saved. It only reads and searches unless you turn on tools in Settings.", component: AskCell },
  { id: "claude", name: "New Claude session", group: "AI & agents", blurb: "Start Claude Code in a recent project.", component: ClaudeCell },
  { id: "agenda", name: "Agenda", group: "Workflow", blurb: "Your next event today (from your calendar links); click for the calendar.", component: AgendaCell },
  { id: "agent", name: "Agent (MCP)", group: "AI & agents", blurb: "A connected MCP server: what's next, add a to-do, search its notes (Settings > Agenda).", component: AgentCell },
  { id: "sessions", name: "Claude sessions", group: "AI & agents", blurb: "How many Claude Code sessions are working, and which finished or need an approval (from local transcripts).", component: SessionsCell },
  {
    id: "claude-usage", name: "Claude usage", group: "AI & agents", blurb: "Your 5-hour and weekly Claude limits; turns amber, then red, as you near them.", component: ClaudeUsage,
    defaults: { every: "15" },
    fields: [{ key: "every", label: "Check every", type: "select", choices: [["5", "5 min"], ["15", "15 min"], ["30", "30 min"], ["60", "hour"]] }],
  },
  {
    id: "meeting", name: "Next meeting", group: "Workflow", blurb: "Today's next meeting from your calendars, a countdown, and Join when it's close.", component: Meeting,
    defaults: { hideWhenFree: true, length: "60", pauseMedia: false },
    fields: [
      { key: "hideWhenFree", label: "Hide when nothing's left today", type: "toggle" },
      { key: "pauseMedia", label: "Pause music when a meeting starts", type: "toggle" },
      { key: "length", label: "Meetings last", type: "select", choices: [["30", "30 min"], ["45", "45 min"], ["60", "1 hour"], ["90", "90 min"]] },
    ],
  },
  {
    id: "timeline", name: "Day timeline", group: "Workflow", blurb: "The next few hours as a strip with today's meetings from your calendars and a now line (horizontal docks).", component: Timeline,
    defaults: { hours: "5", width: "220", length: "60" },
    fields: [
      { key: "hours", label: "Look ahead", type: "select", choices: [["3", "3 hours"], ["5", "5 hours"], ["8", "8 hours"], ["12", "12 hours"]] },
      { key: "width", label: "Width", type: "select", choices: [["160", "Narrow"], ["220", "Medium"], ["300", "Wide"]] },
      { key: "length", label: "Meetings last", type: "select", choices: [["30", "30 min"], ["60", "1 hour"], ["90", "90 min"]] },
    ],
  },
  { id: "todos", name: "To-dos", group: "Workflow", blurb: "How many to-dos are on today's list; click to tick them off, add more, and see what's coming up.", component: TodosCell },
  {
    id: "capture", name: "Quick to-do", group: "Workflow", blurb: "Type a to-do for today, tomorrow or next week (your local list, or your MCP server's).", component: Capture,
    defaults: { label: true }, fields: [{ key: "label", label: "Show the label", type: "toggle" }],
  },
  {
    id: "projects", name: "Projects", group: "Workflow", blurb: "Your repos in Programming, latest first: open in VS Code, a terminal, Claude Code, Explorer or GitHub.", component: Projects,
    defaults: { label: true }, fields: [{ key: "label", label: "Show the label", type: "toggle" }],
  },
  { id: "clipboard", name: "Clipboard history", group: "Workflow", blurb: "Your recent copies: search, copy back or ask Claude. Memory only; private copies (passwords) skipped.", component: Clipboard },
  { id: "snip", name: "Snip & ask", group: "Workflow", blurb: "Drag over part of the screen, then ask Claude about it in Quick Ask.", component: Snip },
  { id: "focus", name: "Focus timer", group: "Workflow", blurb: "Focus sessions, breaks and timers with a countdown on the dock; tells you when time's up.", component: FocusCell },
  {
    id: "cpu", name: "CPU", group: "System", blurb: "Processor load with a live graph; click for the System flyout.", component: CpuCell,
    defaults: { graph: true }, fields: [{ key: "graph", label: "Show the graph", type: "toggle" }],
  },
  {
    id: "ram", name: "Memory", group: "System", blurb: "RAM in use.", component: RamCell,
    defaults: { show: "pct" }, fields: [{ key: "show", label: "Show", type: "select", choices: [["pct", "Percent"], ["gb", "GB in use"]] }],
  },
  {
    id: "gpu", name: "GPU", group: "System", blurb: "Graphics card load (the way Task Manager counts it) or VRAM in use, with a graph.", component: GpuCell,
    defaults: { graph: true, show: "load", which: "discrete" },
    fields: [
      { key: "show", label: "Show", type: "select", choices: [["load", "Load"], ["vram", "VRAM in use"]] },
      { key: "which", label: "Card", type: "select", choices: [["discrete", "The main card"], ["busiest", "Whichever is busiest"]] },
      { key: "graph", label: "Show the graph", type: "toggle" },
    ],
  },
  { id: "top", name: "Top apps", group: "System", blurb: "The app using the most CPU right now; click for the top six.", component: TopCell },
  { id: "network", name: "Network", group: "System", blurb: "Download and upload speed.", component: NetCell },
  {
    id: "storage", name: "Storage", group: "System", blurb: "Free space on a drive.", component: StorageCell,
    defaults: { drive: "C:" }, fields: [{ key: "drive", label: "Drive", type: "text" }],
  },
  {
    id: "battery", name: "Battery", group: "System", blurb: "Charge, charging, and the time left on battery.", component: BatteryCell,
    defaults: { time: true, style: "bar" },
    fields: [
      { key: "style", label: "Style", type: "select", choices: [["bar", "Battery"], ["ring", "Ring"], ["dots", "Dots"]] },
      { key: "time", label: "Show time left", type: "toggle" },
    ],
  },
  {
    id: "ping", name: "Ping", group: "System", blurb: "Latency to a server as live bars, with jitter and loss in the tooltip; check your connection before a call.", component: PingCell,
    defaults: { target: "1.1.1.1", every: "5" },
    fields: [
      { key: "target", label: "Server (IPv4)", type: "text", placeholder: "1.1.1.1" },
      { key: "every", label: "Every", type: "select", choices: [["2", "2 seconds"], ["5", "5 seconds"], ["15", "15 seconds"]] },
    ],
  },
  { id: "screen-time", name: "Screen Time", group: "System", blurb: "How long you've been active today, by program, and the week (kept on this PC).", component: ScreenTime },
  { id: "power", name: "Power", group: "System", blurb: "Lock, sleep, sign out, restart or shut down (the last three ask for a second click).", component: PowerCell },
  {
    id: "now-playing", name: "Now Playing", group: "Media", blurb: "Spotify, YouTube, any player: cover, play/pause, skip; click for the full player.", component: NowPlaying,
    defaults: { hideWhenIdle: true, skip: true },
    fields: [
      { key: "hideWhenIdle", label: "Hide when nothing's playing", type: "toggle" },
      { key: "skip", label: "Show the skip button", type: "toggle" },
    ],
  },
  {
    id: "volume", name: "Volume", group: "Media", blurb: "Speaker volume; scroll over it to change, click to mute or open quick settings.", component: VolumeCell,
    defaults: { step: "2", click: "flyout" },
    fields: [
      { key: "step", label: "Scroll step", type: "select", choices: [["1", "1%"], ["2", "2%"], ["5", "5%"]] },
      { key: "click", label: "Click", type: "select", choices: [["flyout", "Opens quick settings"], ["mute", "Mutes"]] },
    ],
  },
  {
    id: "mic", name: "Mic", group: "Media", blurb: "One click mutes or unmutes your microphone (for calls too).", component: MicCell,
    defaults: { label: true }, fields: [{ key: "label", label: "Show the label", type: "toggle" }],
  },
  { id: "brightness", name: "Brightness", group: "Media", blurb: "Screen brightness; scroll over it to change (laptop screens).", component: BrightnessCell },
  {
    id: "world-clock", name: "World clock", group: "Time", blurb: "The time in another city; add it once per city.", multi: true, component: WorldClock,
    defaults: { zone: "Europe/London", label: "" },
    fields: [
      { key: "zone", label: "City", type: "select", choices: ZONES },
      { key: "label", label: "Name (optional)", type: "text", placeholder: "e.g. Mom" },
    ],
  },
  { id: "sun", name: "Sunrise & sunset", group: "Time", blurb: "The next sunrise or sunset in your city.", component: SunCell },
  {
    id: "countdown", name: "Countdown", group: "Time", blurb: "Days to an exam, a trip or a deadline (hours on the day); add one per date.", multi: true, component: Countdown,
    defaults: { label: "", when: "" },
    fields: [
      { key: "label", label: "Name", type: "text", placeholder: "e.g. Final exam, Trip to Lisbon" },
      { key: "when", label: "Date (and time)", type: "text", placeholder: "2026-11-19 or 2026-11-19 14:00" },
    ],
  },
  { id: "timer", name: "Stopwatch", group: "Time", blurb: "Click to start or pause; reset when paused. (For countdowns, use Focus timer.)", component: Stopwatch },
  {
    id: "weather", name: "Weather", group: "Weather", blurb: "Now, with today's high and low; click for hourly, the week and sunrise/sunset. City is in Settings.", component: WeatherCell,
    defaults: { unit: FAHRENHEIT ? "f" : "c", range: true, rainAlert: true },
    fields: [
      { key: "unit", label: "Units", type: "select", choices: [["f", "°F, mph"], ["c", "°C, km/h"]] },
      { key: "range", label: "Show today's high and low", type: "toggle" },
      { key: "rainAlert", label: "Umbrella when rain is likely in the next 3 hours", type: "toggle" },
    ],
  },
  {
    id: "stocks", name: "Stocks", group: "Everyday", blurb: "A watchlist of stocks, indexes, crypto and currencies with today's change; click for the list (Yahoo Finance, no account).", component: StocksCell,
    defaults: { symbols: DEFAULT_SYMBOLS, rotate: true, graph: true },
    fields: [
      { key: "symbols", label: "Symbols", type: "text", placeholder: "AAPL, ^GSPC, BTC-USD, EURUSD=X" },
      { key: "rotate", label: "Step through them (off: show the first)", type: "toggle" },
      { key: "graph", label: "Show today's graph", type: "toggle" },
    ],
  },
  {
    id: "currency", name: "Currency", group: "Everyday", blurb: "Today's exchange rate between two currencies; click to convert any amount (European Central Bank rates).", component: CurrencyCell,
    defaults: { from: "USD", to: "EUR" },
    fields: [
      { key: "from", label: "From", type: "text", placeholder: "USD" },
      { key: "to", label: "To", type: "text", placeholder: "EUR" },
    ],
  },
  {
    id: "news", name: "Headlines", group: "Everyday", blurb: "The latest headlines from any news site's RSS feed, one at a time; click for the list.", component: HeadlinesCell,
    defaults: { feed: DEFAULT_FEED, width: "normal" },
    fields: [
      { key: "feed", label: "Feed link (RSS or Atom)", type: "text", placeholder: "https://feeds.bbci.co.uk/news/rss.xml" },
      { key: "width", label: "Width", type: "select", choices: [["normal", "Normal"], ["wide", "Wide"]] },
    ],
  },
  {
    id: "photo", name: "Photo frame", group: "Everyday", blurb: "A photo from your Pictures (or any folder), changing every few minutes; scroll for another, click to open it. Stays on this PC.", component: PhotoCell, multi: true,
    defaults: { folder: "", every: "15", shape: "square" },
    fields: [
      { key: "folder", label: "Folder (empty: Pictures)", type: "text", placeholder: "C:\\Users\\you\\Pictures\\Trips" },
      { key: "every", label: "Change every", type: "select", choices: [["1", "Minute"], ["5", "5 minutes"], ["15", "15 minutes"], ["60", "Hour"], ["1440", "Day"]] },
      { key: "shape", label: "Shape", type: "select", choices: [["square", "Square"], ["wide", "Wide"]] },
    ],
  },
  {
    id: "shortcuts", name: "Shortcuts", group: "Everyday", blurb: "Your own buttons for folders, files, apps, web pages and Settings pages.", component: ShortcutsCell, multi: true,
    defaults: { items: DEFAULT_SHORTCUTS },
    fields: [{ key: "items", label: "Items (Name = folder, file or link; separate with ;)", type: "text", placeholder: "Docs = %USERPROFILE%\\Documents; Mail = https://outlook.com" }],
  },
  { id: "downloads", name: "Downloads", group: "Files & tools", blurb: "Your newest downloads with previews; click to open, or show in Explorer.", component: DownloadsCell },
  { id: "recent", name: "Recent files", group: "Files & tools", blurb: "The files you opened most recently, with previews; click to open.", component: RecentCell },
  { id: "screenshots", name: "Screenshots", group: "Files & tools", blurb: "Your newest screenshots as a grid; click to open.", component: ScreenshotsCell },
  { id: "recycle", name: "Recycle Bin", group: "Files & tools", blurb: "How much is in the bin; open it, or empty it (Windows asks first).", component: RecycleCell },
  {
    id: "notes", name: "Notes", group: "Files & tools", blurb: "A scratch pad that saves as you type (kept on this PC).", component: NotesCell,
    defaults: { preview: true }, fields: [{ key: "preview", label: "Show the first line", type: "toggle" }],
  },
  { id: "calc", name: "Calculator", group: "Files & tools", blurb: "A quick calculator: percentages (15% of 80), powers, sqrt, pi; the answer shows as you type.", component: CalcCell },
  {
    id: "transit", name: "Boston MBTA arrivals", group: "Workflow", component: Transit,
    blurb: "Boston only: next train or bus each way at your MBTA stop (Park Street by default); click for every line's next times.",
    defaults: { stop: "place-pktrm" },
    fields: [{ key: "stop", label: "Stop", type: "select", choices: STOPS }],
  },
  { id: "github", name: "GitHub", group: "Dev & accounts", blurb: "Review requests, your PRs and their checks, unread notifications (uses your gh CLI login).", component: GitHubCell },
  { id: "docker", name: "Docker", group: "Dev & accounts", blurb: "How many containers are running; start, stop or restart them (uses the docker CLI).", component: DockerCell },
  {
    id: "sites", name: "Site status", group: "Dev & accounts", blurb: "Whether your sites are up and how fast they answer; red when one is down.", component: SitesCell,
    defaults: { list: "" },
    fields: [{ key: "list", label: "Sites (comma-separated)", type: "text", placeholder: "example.com, myapp.vercel.app" }],
  },
  {
    id: "moon-phase",
    name: "Moon phase",
    group: "Everyday",
    blurb: "Tonight's moon, its phase and how much of it is lit.",
    component: MoonPhaseCell,
    defaults: { label: true },
    fields: [{ key: "label", label: "Show the label", type: "toggle" }],
  },
];

export const byId = (id: string) => WIDGETS.find((w) => w.id === id);
