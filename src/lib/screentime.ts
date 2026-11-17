export type AppTime = { exe: string; name: string; secs: number };
export type StDay = { date: string; total: number; apps: [AppTime, string | null][] };

/** 11580 -> "3h 13m", 2700 -> "45m", 20 -> "<1m". */
export function dur(secs: number): string {
  if (secs < 60) return "<1m";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h ? `${h}h ${m}m` : `${m}m`;
}

const NAMES: Record<string, string> = {
  firefox: "Firefox",
  code: "VS Code",
  windowsterminal: "Terminal",
  msedge: "Edge",
  chrome: "Chrome",
  explorer: "File Explorer",
  notepad: "Notepad",
  claude: "Claude",
  discord: "Discord",
  spotify: "Spotify",
  slack: "Slack",
  winword: "Word",
  excel: "Excel",
  powerpnt: "PowerPoint",
  robloxstudiobeta: "Roblox Studio",
  mintty: "MSYS2",
};

/** A friendlier program name: known ones by hand, else the file name as is. */
export const appName = (stem: string) => NAMES[stem.toLowerCase()] ?? stem;

/** "2026-11-16" -> "Mon" (local). */
export function weekday(date: string): string {
  const [y, m, d] = date.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString("en-US", { weekday: "short" });
}
