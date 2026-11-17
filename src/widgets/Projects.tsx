import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";

/** A folder-with-brackets glyph. */
export function ProjectGlyph({ size = 20 }: { size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" className="pglyph">
      <path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h4.2l1.8 2h9a1.5 1.5 0 0 1 1.5 1.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5z" />
      <path d="M10 11.5l-2 2 2 2M14 11.5l2 2-2 2" />
    </svg>
  );
}

/** Opens the Projects flyout: recent repos with VS Code, Terminal, Claude, Explorer and GitHub. */
export default function Projects({ settings, options }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  return (
    <button className="cell projects" title="Projects" onClick={(e) => openAt("projects", e, vertical)}>
      <ProjectGlyph />
      {!vertical && options.label !== false && <span>Projects</span>}
    </button>
  );
}
