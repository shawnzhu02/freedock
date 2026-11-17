// Search helpers for the Start menu (src/popup/StartView.tsx). The same ranking as startmenu.rs.

/** How well `name` answers `q` (lower is better; null = no match): the whole name, its start, a
 *  word's start, its initials ("vsc" for Visual Studio Code), anywhere in it. */
export function rankName(name: string, q: string): number | null {
  const n = name.toLowerCase();
  const t = q.trim().toLowerCase();
  if (!t) return null;
  const words = n.split(/[^\p{L}\p{N}]+/u).filter(Boolean);
  const initials = words.map((w) => w[0]).join("");
  if (n === t) return 0;
  if (n.startsWith(t)) return 1;
  if (words.some((w) => w.startsWith(t))) return 2;
  if (t.length >= 2 && initials.startsWith(t)) return 3;
  if (n.includes(t)) return 4;
  return null;
}

/** Settings pages Start can find: name, extra search words, and the page. */
export const SETTINGS_PAGES: { name: string; words: string; uri: string }[] = [
  { name: "Display", words: "screen resolution scale monitor brightness", uri: "ms-settings:display" },
  { name: "Night light", words: "blue light warm", uri: "ms-settings:nightlight" },
  { name: "Sound", words: "audio volume speakers microphone output input", uri: "ms-settings:sound" },
  { name: "Notifications", words: "do not disturb focus alerts", uri: "ms-settings:notifications" },
  { name: "Power & battery", words: "sleep battery saver energy", uri: "ms-settings:powersleep" },
  { name: "Storage", words: "disk space cleanup", uri: "ms-settings:storagesense" },
  { name: "Multitasking", words: "snap windows alt tab", uri: "ms-settings:multitasking" },
  { name: "Clipboard", words: "history copy paste", uri: "ms-settings:clipboard" },
  { name: "About this PC", words: "system info specs rename pc", uri: "ms-settings:about" },
  { name: "Bluetooth & devices", words: "pair headphones mouse keyboard", uri: "ms-settings:bluetooth" },
  { name: "Printers & scanners", words: "printer print scan", uri: "ms-settings:printers" },
  { name: "Mouse", words: "pointer cursor scroll", uri: "ms-settings:mousetouchpad" },
  { name: "Touchpad", words: "gestures trackpad", uri: "ms-settings:devices-touchpad" },
  { name: "Wi-Fi", words: "wireless network internet", uri: "ms-settings:network-wifi" },
  { name: "Network & internet", words: "ethernet vpn proxy", uri: "ms-settings:network-status" },
  { name: "VPN", words: "network", uri: "ms-settings:network-vpn" },
  { name: "Mobile hotspot", words: "share internet tethering", uri: "ms-settings:network-mobilehotspot" },
  { name: "Airplane mode", words: "flight", uri: "ms-settings:network-airplanemode" },
  { name: "Background", words: "wallpaper personalize", uri: "ms-settings:personalization-background" },
  { name: "Colors", words: "dark mode light mode accent theme", uri: "ms-settings:colors" },
  { name: "Themes", words: "personalize", uri: "ms-settings:themes" },
  { name: "Lock screen", words: "personalize", uri: "ms-settings:lockscreen" },
  { name: "Fonts", words: "typeface", uri: "ms-settings:fonts" },
  { name: "Installed apps", words: "uninstall programs remove", uri: "ms-settings:appsfeatures" },
  { name: "Default apps", words: "browser open with", uri: "ms-settings:defaultapps" },
  { name: "Startup apps", words: "start with windows boot", uri: "ms-settings:startupapps" },
  { name: "Your info", words: "account profile picture", uri: "ms-settings:yourinfo" },
  { name: "Sign-in options", words: "password pin windows hello fingerprint face", uri: "ms-settings:signinoptions" },
  { name: "Date & time", words: "clock time zone", uri: "ms-settings:dateandtime" },
  { name: "Language & region", words: "keyboard input locale", uri: "ms-settings:regionlanguage" },
  { name: "Typing", words: "keyboard autocorrect", uri: "ms-settings:typing" },
  { name: "Gaming", words: "game mode xbox game bar", uri: "ms-settings:gaming-gamemode" },
  { name: "Accessibility", words: "ease of access narrator magnifier contrast", uri: "ms-settings:easeofaccess" },
  { name: "Privacy & security", words: "permissions camera location microphone", uri: "ms-settings:privacy" },
  { name: "Windows Security", words: "antivirus defender firewall", uri: "ms-settings:windowsdefender" },
  { name: "Windows Update", words: "updates upgrade patch", uri: "ms-settings:windowsupdate" },
  { name: "Recovery", words: "reset restore", uri: "ms-settings:recovery" },
  { name: "Settings", words: "control panel options", uri: "ms-settings:" },
];
