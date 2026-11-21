import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openAt } from "./Agents";
import type { WidgetProps } from "./registry";
import { pollActive } from "../lib/active";

export type Container = { id: string; name: string; image: string; state: string; status: string; ports: string };
export type DockerList = { containers: Container[]; error: string | null };

/** The container list, polled while the dock is visible. */
export function useDocker(everyMs: number, key = 0) {
  const [d, setD] = useState<DockerList | null>(null);
  useEffect(() => {
    const get = () => invoke<DockerList>("docker_list").then(setD).catch((e) => setD({ containers: [], error: String(e) }));
    get();
    const stop = pollActive(everyMs, () => get());
    return () => stop();
  }, [everyMs, key]);
  return [d, setD] as const;
}

/** A container glyph (our own drawing): a box with stacked slats. */
export function BoxGlyph({ size = 20 }: { size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" className="pglyph">
      <path d="M3.5 8.5 12 4l8.5 4.5v7L12 20l-8.5-4.5z" />
      <path d="M3.5 8.5 12 13l8.5-4.5M12 13v7" />
    </svg>
  );
}

/** Docker: how many containers are running; the flyout lists them with start/stop/restart. */
export default function DockerCell({ settings }: WidgetProps) {
  const vertical = settings.edge === "left" || settings.edge === "right";
  const [d] = useDocker(60_000);
  const running = d?.containers.filter((c) => c.state === "running").length ?? 0;
  const tip = !d ? "Docker" : d.error ? `Docker: ${d.error}` : `Docker: ${running} running of ${d.containers.length}`;
  return (
    <button className={`cell stat ctl dockerc${d?.error ? " off" : ""}`} title={tip} onClick={(e) => openAt("docker", e, vertical)}>
      <BoxGlyph size={18} />
      {!vertical && <b className="cval">{d && !d.error ? running : "–"}</b>}
    </button>
  );
}
