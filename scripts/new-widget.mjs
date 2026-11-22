#!/usr/bin/env node
// Scaffold a new dock widget:  npm run new:widget -- "Steam friends"  [--group "Dev & accounts"]
//
// Creates src/widgets/<Name>.tsx from a commented template and registers it in
// src/widgets/registry.tsx, so it shows up in Settings > Widgets > Add right away (run `npm run tauri
// dev` and add it). Then edit the new file: docs/WIDGETS.md and AGENTS.md explain the pieces.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const gi = args.indexOf("--group");
const group = gi >= 0 ? args.splice(gi, 2)[1] : "Workflow";
const title = args.join(" ").trim();
if (!title) {
  console.error('Usage: npm run new:widget -- "Widget name" [--group "System"]');
  process.exit(1);
}

const words = title.split(/[^A-Za-z0-9]+/).filter(Boolean);
const id = words.map((w) => w.toLowerCase()).join("-");
// A name must be a valid identifier: "1Password vault" becomes Widget1PasswordVault.
const name = words.map((w) => w[0].toUpperCase() + w.slice(1)).join("").replace(/^(?=\d)/, "Widget");
if (!name) {
  console.error("Use a name with at least one letter or digit.");
  process.exit(1);
}
const component = `${name}Cell`;
const file = join(root, "src", "widgets", `${name}.tsx`);
const registry = join(root, "src", "widgets", "registry.tsx");

const reg = readFileSync(registry, "utf8");
if (existsSync(file)) {
  console.error(`src/widgets/${name}.tsx already exists.`);
  process.exit(1);
}
if (reg.includes(`id: "${id}"`)) {
  console.error(`A widget with id "${id}" is already registered.`);
  process.exit(1);
}
// Another widget's file can export a component with this name already (News.tsx has HeadlinesCell);
// a second import of it would not compile.
if (new RegExp(`\\b${component}\\b`).test(reg)) {
  console.error(`registry.tsx already uses the name ${component}. Pick a different widget name.`);
  process.exit(1);
}
const groups = [...reg.matchAll(/export const GROUPS = \[([^\]]*)\]/g)][0]?.[1] ?? "";
if (!groups.includes(`"${group}"`)) {
  console.error(`Unknown group "${group}". Pick one of: ${groups}`);
  process.exit(1);
}

writeFileSync(
  file,
  `import { useEffect, useState } from "react";
import { pollActive } from "../lib/active";
import type { WidgetProps } from "./registry";

/** ${title}: TODO describe what the cell shows and what a click does.
 *
 *  How a widget works (see docs/WIDGETS.md):
 *  - It is a React component that renders one \`<button className="cell">\` (or a \`<div>\`) on the dock.
 *  - \`settings\` is the whole dock settings object; \`options\` are this widget's own options (their
 *    defaults and fields are in its registry entry, and Settings > Widgets shows them under the gear).
 *  - Keep it light: \`pollActive(ms, f)\` runs f only while the dock is showing, and catches up when it\n *    comes back into view.
 *  - Anything that needs Windows or the network without CORS goes in a Rust command
 *    (src-tauri/src/*.rs, registered in lib.rs) that you call with \`invoke("name", { args })\`.
 *  - To open a flyout on click, add a popup mode (see "Add a flyout" in AGENTS.md) and call
 *    \`openAt("mode", e, vertical)\` from "./Agents".
 */
export default function ${component}({ settings, options }: WidgetProps) {
  const [value, setValue] = useState<string>("…");

  useEffect(() => {
    const load = () => {
      // TODO fetch or compute what the cell shows.
      setValue(new Date().toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }));
    };
    load();
    return pollActive(60_000, load);
  }, []);

  const vertical = settings.edge === "left" || settings.edge === "right";
  return (
    <button className="cell" title="${title}" aria-label="${title}" onClick={() => {}}>
      {!vertical && options.label !== false && <small>${title}</small>}
      <b>{value}</b>
    </button>
  );
}
`,
);

const entry = `  {
    id: "${id}",
    name: "${title}",
    group: "${group}",
    blurb: "TODO: one line for the widget picker.",
    component: ${component},
    defaults: { label: true },
    fields: [{ key: "label", label: "Show the label", type: "toggle" }],
  },
`;
const lines = reg.split("\n");
const lastImport = lines.reduce((at, l, i) => (l.startsWith("import ") ? i : at), 0);
lines.splice(lastImport + 1, 0, `import ${component} from "./${name}";`);
let next = lines.join("\n");
const start = next.indexOf("export const WIDGETS: WidgetDef[] = [");
const end = next.indexOf("\n];", start);
next = next.slice(0, end + 1) + entry + next.slice(end + 1);
writeFileSync(registry, next);

console.log(`Created src/widgets/${name}.tsx and registered "${id}" in src/widgets/registry.tsx.`);
console.log("Next: npm run tauri dev, then Settings > Widgets > Add > " + title + ".");
