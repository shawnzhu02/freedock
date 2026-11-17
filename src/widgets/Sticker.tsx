// A cute thing on the dock: one of our own animated characters, or the user's own image or GIF.
// Clicking it pets it (a heart floats up), bounces it, or opens a link the user set.
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { WidgetProps } from "./registry";

export const BUILTIN: [string, string][] = [
  ["cat", "Cat (blinks)"],
  ["ghost", "Ghost (floats)"],
  ["plant", "Plant (sways)"],
  ["star", "Star (twinkles)"],
  ["blob", "Blob (squishes)"],
];

/** Our own little characters; the motion is CSS (dock.css, .stk-*). */
export function Character({ kind, size }: { kind: string; size: number }) {
  const common = { width: size, height: size, viewBox: "0 0 48 48", "aria-hidden": true } as const;
  switch (kind) {
    case "ghost":
      return (
        <svg {...common} className="stk-ghost">
          <path d="M10 44V22a14 14 0 0 1 28 0v22l-4.7-3.5L28.7 44 24 40.5 19.3 44l-4.6-3.5z" fill="#f2f0ff" />
          <ellipse cx="19" cy="23" rx="2.6" ry="3.4" fill="#2b2540" />
          <ellipse cx="29" cy="23" rx="2.6" ry="3.4" fill="#2b2540" />
          <ellipse cx="15.5" cy="28.5" rx="2.4" ry="1.4" fill="#ffb3c7" />
          <ellipse cx="32.5" cy="28.5" rx="2.4" ry="1.4" fill="#ffb3c7" />
          <path d="M22 29q2 2 4 0" stroke="#2b2540" strokeWidth="1.6" fill="none" strokeLinecap="round" />
        </svg>
      );
    case "plant":
      return (
        <svg {...common}>
          <g className="stk-leaves">
            <path d="M24 30C24 20 18 14 10 13c0 8 6 14 14 17z" fill="#5ccf7a" />
            <path d="M24 30c0-12 7-19 15-20-1 9-7 17-15 20z" fill="#3fb866" />
            <path d="M24 31V18" stroke="#2f9a52" strokeWidth="2" strokeLinecap="round" />
          </g>
          <path d="M14 30h20l-3 14H17z" fill="#e8875a" />
          <rect x="12.5" y="28" width="23" height="5" rx="2" fill="#f19c6f" />
          <circle cx="20.5" cy="37" r="1.3" fill="#5a2e1c" />
          <circle cx="27.5" cy="37" r="1.3" fill="#5a2e1c" />
          <path d="M22.5 39.5q1.5 1.3 3 0" stroke="#5a2e1c" strokeWidth="1.2" fill="none" strokeLinecap="round" />
        </svg>
      );
    case "star":
      return (
        <svg {...common} className="stk-star">
          <path d="M24 5l5.6 11.6 12.7 1.8-9.2 8.9 2.2 12.6L24 34l-11.3 5.9 2.2-12.6-9.2-8.9 12.7-1.8z" fill="#ffd34d" stroke="#f2b51d" strokeWidth="1.5" strokeLinejoin="round" />
          <circle cx="20" cy="22" r="1.8" fill="#4a3500" />
          <circle cx="28" cy="22" r="1.8" fill="#4a3500" />
          <path d="M21.5 26.5q2.5 2.2 5 0" stroke="#4a3500" strokeWidth="1.5" fill="none" strokeLinecap="round" />
          <path className="stk-sparkle" d="M40 6l1 3 3 1-3 1-1 3-1-3-3-1 3-1z" fill="#fff6c9" />
        </svg>
      );
    case "blob":
      return (
        <svg {...common} className="stk-blob">
          <path d="M8 34c0-12 7-22 16-22s16 10 16 22c0 6-7 8-16 8S8 40 8 34z" fill="#8fd3ff" />
          <ellipse cx="18" cy="20" rx="4" ry="2.5" fill="#d9f2ff" opacity=".7" />
          <circle cx="19" cy="29" r="2.2" fill="#123048" />
          <circle cx="29" cy="29" r="2.2" fill="#123048" />
          <path d="M22 33q2 2 4 0" stroke="#123048" strokeWidth="1.6" fill="none" strokeLinecap="round" />
        </svg>
      );
    default:
      return (
        <svg {...common}>
          <g className="stk-cat">
            <path d="M9 20L11 6l9 8M39 20L37 6l-9 8" fill="#ffb867" />
            <path d="M12.5 10.5l1 6 4-2.5M35.5 10.5l-1 6-4-2.5" fill="#ffd9a8" />
            <ellipse cx="24" cy="27" rx="16" ry="14" fill="#ffb867" />
            <g className="stk-eyes">
              <ellipse cx="18" cy="26" rx="2.3" ry="3" fill="#3a2410" />
              <ellipse cx="30" cy="26" rx="2.3" ry="3" fill="#3a2410" />
            </g>
            <path d="M22.5 31h3l-1.5 1.6z" fill="#e8607d" />
            <path d="M24 32.6q-2 2.4-4 .6M24 32.6q2 2.4 4 .6" stroke="#3a2410" strokeWidth="1.2" fill="none" strokeLinecap="round" />
            <path d="M7 29h7M7 32l7-1M41 29h-7M41 32l-7-1" stroke="#7a4c22" strokeWidth=".9" strokeLinecap="round" />
          </g>
        </svg>
      );
  }
}

const SIZES: Record<string, number> = { small: 26, medium: 34, large: 42 };

export default function Sticker({ options }: WidgetProps) {
  const kind = String(options.kind ?? "cat");
  const image = typeof options.image === "string" ? options.image : "";
  const size = SIZES[String(options.size ?? "medium")] ?? 34;
  const click = String(options.click ?? "pet");
  const [src, setSrc] = useState<string | null>(null);
  const [err, setErr] = useState("");
  const [hearts, setHearts] = useState<number[]>([]);
  const [bounce, setBounce] = useState(0);

  useEffect(() => {
    setSrc(null);
    setErr("");
    if (kind !== "custom" || !image) return;
    invoke<string>("sticker_get", { name: image }).then(setSrc).catch((e) => setErr(String(e)));
  }, [kind, image]);

  const onClick = () => {
    if (click === "link") {
      const url = String(options.link ?? "").trim();
      if (/^https?:\/\//i.test(url)) invoke("open_link", { url });
      return;
    }
    if (click === "bounce") {
      setBounce((b) => b + 1);
      return;
    }
    if (click === "pet") {
      const id = Date.now() + Math.random();
      setHearts((h) => [...h.slice(-4), id]);
      setBounce((b) => b + 1);
      setTimeout(() => setHearts((h) => h.filter((x) => x !== id)), 1100);
    }
  };

  const art =
    kind === "custom" ? (
      src ? (
        <img src={src} alt="" style={{ height: size, maxWidth: size * 2.5 }} draggable={false} />
      ) : (
        <span className="stk-empty" title={err || "Pick an image in Settings > Widgets > Sticker"}>
          ?
        </span>
      )
    ) : (
      <Character kind={kind} size={size} />
    );

  return (
    <button
      className={`cell sticker${options.bare === false ? "" : " bare"}`}
      title={click === "link" && options.link ? String(options.link) : click === "pet" ? "Pet me" : ""}
      onClick={onClick}
    >
      <span key={bounce} className={bounce ? "stk-hop" : ""}>
        {art}
      </span>
      {hearts.map((h) => (
        <i key={h} className="stk-heart" aria-hidden="true">
          ♥
        </i>
      ))}
    </button>
  );
}
