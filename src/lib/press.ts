// Props that make a clickable row reachable by keyboard: Tab focuses it, Enter or Space runs it.
// Keys pressed on a real button inside the row are left to that button.
import type { KeyboardEvent } from "react";

export function press(run: () => void) {
  return {
    role: "button",
    tabIndex: 0,
    onClick: run,
    onKeyDown: (e: KeyboardEvent<HTMLElement>) => {
      if (e.target !== e.currentTarget || (e.key !== "Enter" && e.key !== " ")) return;
      e.preventDefault();
      run();
    },
  };
}
