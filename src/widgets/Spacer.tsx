import type { WidgetProps } from "./registry";

/** Space between widgets, with an optional faint divider so it reads as a deliberate gap.
 *  Sizes are Auto, fixed (Small 12 px, Medium 32 px, Large 64 px) or Fill, which takes all the free room on
 *  a full-width bar (on a pill, which hugs its widgets, Fill acts as Large). In move mode a dashed
 *  outline shows where it is. */
export default function Spacer({ settings, options }: WidgetProps) {
  // Auto (the default, and what older spacers get): fill on a full-width bar, a medium gap on a pill.
  const pick = String(options.size ?? "auto");
  const size = pick === "auto" ? (settings.shape === "full" ? "fill" : "medium") : pick;
  const look = String(options.look ?? "line");
  return (
    <span className={`spacer sz-${size} look-${look}`} aria-hidden="true">
      <i />
    </span>
  );
}
