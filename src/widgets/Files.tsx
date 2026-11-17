import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";

type Kind = "downloads" | "screenshots" | "recent";

function Glyph({ kind }: { kind: Kind }) {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" className="pglyph">
      {kind === "recent" ? (
        <>
          <path d="M7 3.5h6.5L18 8v4M7 3.5a1 1 0 0 0-1 1v15a1 1 0 0 0 1 1h4.5" />
          <circle cx="16.5" cy="16.5" r="4" />
          <path d="M16.5 14.5v2l1.4 1.2" />
        </>
      ) : kind === "downloads" ? (
        <>
          <path d="M12 4v10.5M7.5 10.5 12 15l4.5-4.5" />
          <path d="M5 16.5v1.5A2 2 0 0 0 7 20h10a2 2 0 0 0 2-2v-1.5" />
        </>
      ) : (
        <>
          <path d="M4 8V6a2 2 0 0 1 2-2h2M16 4h2a2 2 0 0 1 2 2v2M20 16v2a2 2 0 0 1-2 2h-2M8 20H6a2 2 0 0 1-2-2v-2" />
          <circle cx="12" cy="12" r="3" />
        </>
      )}
    </svg>
  );
}

const make = (kind: Kind, title: string) =>
  function FilesCell({ settings }: WidgetProps) {
    const vertical = settings.edge === "left" || settings.edge === "right";
    return (
      <button className="cell projects" title={title} onClick={(e) => openAt(kind, e, vertical)}>
        <Glyph kind={kind} />
      </button>
    );
  };

export const DownloadsCell = make("downloads", "Latest downloads");
export const ScreenshotsCell = make("screenshots", "Latest screenshots");
export const RecentCell = make("recent", "Recently opened files");
