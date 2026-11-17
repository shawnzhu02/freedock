import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isActive } from "./active.ts";

export type Pr = { title: string; repo: string; url: string; draft: boolean; review: string; checks: string; updated: string };
export type Note = { title: string; repo: string; reason: string; kind: string; url: string; updated: string };
export type GitHub = { login: string; unread: number; notes: Note[]; prs: Pr[]; reviews: Pr[]; error: string | null };

/** "ok" | "fail" | "wait" | "" for a checks rollup state. */
export function checkState(s: string): "ok" | "fail" | "wait" | "" {
  if (s === "SUCCESS") return "ok";
  if (s === "FAILURE" || s === "ERROR") return "fail";
  if (s === "PENDING" || s === "EXPECTED") return "wait";
  return "";
}

/** How many things want the user: unread notifications plus PRs waiting on the user's review. */
export function badge(g: GitHub | null): number {
  return g && !g.error ? g.unread + g.reviews.length : 0;
}

/** "review_requested" -> "review requested". */
export const reasonLabel = (r: string) => r.replace(/_/g, " ");

export function useGitHub(every = 5 * 60_000, opened = 0) {
  const [g, setG] = useState<GitHub | null>(null);
  useEffect(() => {
    const get = (fresh: boolean) => invoke<GitHub>("github_summary", { fresh }).then(setG).catch(() => {});
    get(opened > 0);
    const t = window.setInterval(() => isActive() && get(false), every);
    return () => clearInterval(t);
  }, [every, opened]);
  return g;
}
