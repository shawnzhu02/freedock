import type { WidgetProps } from "./registry";
import { useNow } from "./useNow";
import { countdownText, daysUntil, parseWhen } from "../lib/countdown";

/** Countdown: days (or hours, on the day) to a date the user sets in the widget's options. */
export default function Countdown({ settings, options }: WidgetProps) {
  const now = useNow();
  const vertical = settings.edge === "left" || settings.edge === "right";
  const when = String(options.when ?? "");
  const label = String(options.label ?? "").trim() || "Countdown";
  const target = parseWhen(when);
  const hasTime = /\d:\d/.test(when);
  const text = target ? countdownText(target, now, hasTime) : "Set a date";
  const soon = !!target && daysUntil(target, now) <= 1 && text !== "Done";
  const tip = target
    ? `${label}: ${target.toLocaleString(undefined, hasTime ? { dateStyle: "full", timeStyle: "short" } : { dateStyle: "full" })}`
    : "Countdown: set the date in Settings > Widgets (gear)";
  return (
    <div className={`cell countdown${soon ? " soon" : ""}${text === "Done" ? " done" : ""}`} title={tip}>
      {vertical ? (
        <b className="cdv">{text.replace("Tomorrow", "1d").replace("Today", "0d").replace("Set a date", "–")}</b>
      ) : (
        <span className="cdtext">
          <small>{label}</small>
          <b>{text}</b>
        </span>
      )}
    </div>
  );
}
