import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// Mirrors src-tauri/src/settings.rs.
export type Style = "glass" | "tinted" | "midnight" | "black" | "white" | "clear";
export type WidgetSlot = { id: string; enabled: boolean; options: Record<string, unknown> | null };
/** What a mood may keep for itself (see Mood in settings.rs). */
export type Look = { style: Style; glass_live: boolean; edge: Settings["edge"]; visibility: Settings["visibility"] };
/** A saved dock layout. The active mood's widgets are `Settings.widgets` while it is active. */
export type Mood = { id: string; name: string; icon: string; widgets: WidgetSlot[]; look: Look | null };
export type Settings = {
  edge: "bottom" | "top" | "left" | "right";
  offset: number;
  shape: "pill" | "full";
  style: Style;
  height: number;
  margin: number;
  pill_width: number;
  radius: number;
  accent: string;
  glass_tint: number;
  glass_blur: number;
  glass_live: boolean;
  icon_size: number;
  magnify: number;
  hour12: boolean | null;
  /** How flyouts and previews appear. */
  popup_motion: "warp" | "fade" | "off";
  hotkey: string;
  ask_hotkey: string;
  ask_selection_hotkey: string;
  ask_tools: boolean;
  snip_hotkey: string;
  capture_hotkey: string;
  calc_hotkey: string;
  quick_tiles: string[];
  layouts: Record<string, string[]>;
  claude_launch: string;
  city: string;
  lat: number;
  lon: number;
  hide_fullscreen: boolean;
  visibility: "always" | "smart" | "autohide";
  show_delay: number;
  show_distance: number;
  hide_distance: number;
  hide_delay: number;
  handle: boolean;
  widgets: WidgetSlot[];
  moods: Mood[];
  active_mood: string;
  shared_look: Look | null;
  mood_hotkey: string;
  mood_number_keys: boolean;
  previews: boolean;
  win_opens_start: boolean;
  monitor: string;
  hide_other_taskbars: boolean;
  win_number_keys: boolean;
  web_search: string;
};

export const isVertical = (s: Settings) => s.edge === "left" || s.edge === "right";

export const getSettings = () => invoke<Settings>("get_settings");

/** Apply to the dock now; write to disk only when `persist` (the panel debounces that). */
export const saveSettings = (settings: Settings, persist: boolean) => invoke("save_settings", { settings, persist });

export const onSettings = (cb: (s: Settings) => void) => listen<Settings>("settings-changed", (e) => cb(e.payload));
