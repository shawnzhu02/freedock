import { useEffect } from "react";
import { useKept } from "../lib/kept";
import { invoke } from "@tauri-apps/api/core";

type Win = { hwnd: number; title: string; active: boolean; minimized: boolean };
type App = { key: string; name: string; pinned: boolean; windows: Win[] };

/** An app's right-click menu: its windows (click to switch, x to close), New window, Pin or Unpin,
 *  and Close window(s). Closing asks the window to close, so apps can still ask to save. */
export default function AppMenu({ appKey, opened }: { appKey: string; opened: number }) {
  const [app, setApp] = useKept<App | null | undefined>(`appmenu.${appKey}`, undefined);
  const load = () =>
    invoke<App[]>("apps_list")
      .then((l) => setApp(l.find((a) => a.key === appKey) ?? null))
      .catch(() => setApp(null));
  useEffect(() => {
    load();
  }, [appKey, opened]);
  if (app === undefined) return <div className="view" />;
  if (app === null) return <div className="view"><p className="empty center">That app has closed.</p></div>;
  const done = () => invoke("popup_close");
  const closeAll = () => {
    app.windows.forEach((w) => invoke("app_window", { hwnd: w.hwnd, action: "close" }));
    done();
  };
  return (
    <div className="view appmenu">
      <header>
        <b>{app.name}</b>
        <span className="sp" />
      </header>
      {app.windows.length > 0 && (
        <div className="amwins" role="list" aria-label="Windows">
          {app.windows.map((w) => (
            <div className={`amwin${w.active ? " on" : ""}`} role="listitem" key={w.hwnd}>
              <button
                className="amtitle"
                title={w.title}
                onClick={() => {
                  invoke("app_window", { hwnd: w.hwnd, action: "focus" });
                  done();
                }}
              >
                {w.title || app.name}
                {w.minimized && <small> (minimized)</small>}
              </button>
              <button
                className="x"
                title="Close this window"
                aria-label={`Close ${w.title || app.name}`}
                onClick={() => {
                  invoke("app_window", { hwnd: w.hwnd, action: "close" });
                  setTimeout(load, 400);
                }}
              >
                ×
              </button>
            </div>
          ))}
        </div>
      )}
      <div className="amacts">
        <button onClick={() => invoke("app_launch", { key: app.key }).then(done)}>New window</button>
        <button onClick={() => invoke("app_pin", { key: app.key, on: !app.pinned }).then(done).catch(() => {})}>
          {app.pinned ? "Unpin from dock" : "Pin to dock"}
        </button>
        {app.windows.length > 0 && (
          <button onClick={closeAll}>{app.windows.length > 1 ? `Close all ${app.windows.length} windows` : "Close window"}</button>
        )}
      </div>
    </div>
  );
}
