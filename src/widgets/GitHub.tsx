import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { badge, checkState, useGitHub } from "../lib/github";

/** A pull-request glyph (our own drawing). */
export function PrGlyph({ size = 20 }: { size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" className="pglyph">
      <circle cx="6.5" cy="5.5" r="2.2" />
      <circle cx="6.5" cy="18.5" r="2.2" />
      <circle cx="17.5" cy="18.5" r="2.2" />
      <path d="M6.5 7.7v8.6M17.5 16.3V10a3 3 0 0 0-3-3h-4M12.5 4.8 10.3 7l2.2 2.2" />
    </svg>
  );
}

/** GitHub: a count of what wants the user (unread + review requests); red when one of the user's PRs is failing checks. */
export default function GitHubCell({ settings }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  const g = useGitHub();
  const n = badge(g);
  const failing = !!g && g.prs.some((p) => checkState(p.checks) === "fail");
  const tip = !g
    ? "GitHub"
    : g.error
      ? `GitHub: ${g.error}`
      : `GitHub: ${g.unread} unread, ${g.reviews.length} review request${g.reviews.length === 1 ? "" : "s"}, ${g.prs.length} open PR${g.prs.length === 1 ? "" : "s"}${failing ? " (checks failing)" : ""}`;
  return (
    <button className={`cell projects gh${failing ? " failing" : ""}`} title={tip} onClick={(e) => openAt("github", e, vertical)}>
      <PrGlyph />
      {n > 0 && <span className="ghbadge">{n > 99 ? "99+" : n}</span>}
    </button>
  );
}
