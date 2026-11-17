// Projects list helpers. Pure, so `npm test` covers them.

export type Project = { name: string; path: string; branch: string; touched: number; claude: number | null; web: string | null };

/** Compact age: "now", "12m", "3h", "2d", "5w", "8mo". Seconds in, Unix. */
export function since(at: number, now: number): string {
  const s = Math.max(0, now - at);
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  if (s < 14 * 86400) return `${Math.floor(s / 86400)}d`;
  if (s < 60 * 86400) return `${Math.floor(s / (7 * 86400))}w`;
  return `${Math.floor(s / (30 * 86400))}mo`;
}

/** Filter by every typed word appearing in the name or branch, keeping the recency order. */
export function filter(list: Project[], q: string): Project[] {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return list;
  return list.filter((p) => {
    const hay = `${p.name} ${p.branch}`.toLowerCase();
    return words.every((w) => hay.includes(w));
  });
}
