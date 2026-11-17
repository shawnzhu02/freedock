// Drag-to-reorder with pointer events. HTML5 drag-and-drop doesn't reach the page in these
// windows (Tauri's file-drop handler takes the drag events), so this follows the pointer itself:
// press on an item (not on a button or field inside it), move over another item of the same
// list, and they swap places live; letting go saves.
import { useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { moveItem } from "./layout";

export function useReorder<T>(group: string, items: T[], onChange: (next: T[]) => void, onDone: (final: T[]) => void) {
  const [dragging, setDragging] = useState<number | null>(null);
  const live = useRef(items);
  live.current = items;

  const start = (i: number) => (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    const el = e.target as HTMLElement;
    if (el.closest("button, input, select, textarea, a") && !el.closest("[data-grip]")) return;
    e.preventDefault();
    let cur = i;
    let moved = false;
    setDragging(i);
    document.body.classList.add("reordering");
    const move = (ev: PointerEvent) => {
      const hit = document.elementFromPoint(ev.clientX, ev.clientY)?.closest(`[data-reorder="${group}"]`) as HTMLElement | null;
      if (!hit) return;
      const j = Number(hit.dataset.index);
      if (Number.isNaN(j) || j === cur) return;
      const next = moveItem(live.current, cur, j);
      live.current = next;
      cur = j;
      moved = true;
      setDragging(j);
      onChange(next);
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      document.body.classList.remove("reordering");
      setDragging(null);
      if (moved) {
        onDone(live.current);
        // The click that ends a drag isn't a click on the item (a mood chip would switch moods).
        const eat = (c: MouseEvent) => {
          c.stopPropagation();
          c.preventDefault();
        };
        window.addEventListener("click", eat, { capture: true, once: true });
        setTimeout(() => window.removeEventListener("click", eat, { capture: true }), 0);
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  };

  /** Spread onto each item: <div {...item(i)}>. */
  const item = (i: number) => ({ "data-reorder": group, "data-index": i, onPointerDown: start(i) });
  return { dragging, item };
}
