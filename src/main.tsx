import React from "react";
import ReactDOM from "react-dom/client";

// One page, three windows: the dock, the settings panel (#settings) and the agents' popup (#ask).
// Each loads only its own stylesheet.
const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

async function mount() {
  if (location.hash === "#ask") {
    await import("./popup/popup.css");
    const { default: Popup } = await import("./popup/Popup");
    root.render(
      <React.StrictMode>
        <Popup />
      </React.StrictMode>,
    );
    return;
  }
  if (location.hash === "#settings") {
    await import("./settings/panel.css");
    const { default: Panel } = await import("./settings/Panel");
    root.render(
      <React.StrictMode>
        <Panel />
      </React.StrictMode>,
    );
    return;
  }
  await import("./dock.css");
  const { default: Dock } = await import("./Dock");
  root.render(
    <React.StrictMode>
      <Dock />
    </React.StrictMode>,
  );
}

mount();
