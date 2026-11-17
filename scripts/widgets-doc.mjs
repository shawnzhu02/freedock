#!/usr/bin/env node
// Writes docs/WIDGETS.md from src/widgets/registry.tsx (run after adding widgets: npm run docs:widgets).
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = readFileSync(join(root, "src/widgets/registry.tsx"), "utf8");
const groups = JSON.parse(src.match(/export const GROUPS = (\[[^\]]*\])/)[1]);
const list = src.slice(src.indexOf("export const WIDGETS"));
const str = (s) => s.replace(/\\(["'])/g, "$1");

// One entry per widget id, whatever order its fields are written in.
const starts = [...list.matchAll(/\bid:\s*"([^"]+)"/g)].map((m) => m.index);
const end = list.indexOf("\n];");
const rows = starts.map((at, i) => {
  const e = list.slice(at, starts[i + 1] ?? end);
  const field = (k) => e.match(new RegExp(String.raw`\b${k}:\s*"((?:[^"\\]|\\.)*)"`))?.[1] ?? "";
  return { id: field("id"), name: field("name"), group: field("group"), blurb: str(field("blurb")), soon: !/component:/.test(e) };
});

let out = `# Widgets

Every widget freedock ships with, by group (${rows.length} in all). Add them in Settings > Widgets;
the gear on a widget shows its options. Generated from \`src/widgets/registry.tsx\` by
\`npm run docs:widgets\`.

To make your own: \`npm run new:widget -- "Name"\` (see AGENTS.md).
`;
for (const g of groups) {
  const here = rows.filter((r) => r.group === g);
  if (!here.length) continue;
  out += `\n## ${g}\n\n| Widget | id | What it does |\n|---|---|---|\n`;
  for (const r of here) out += `| ${r.name}${r.soon ? " (planned)" : ""} | \`${r.id}\` | ${r.blurb.replace(/\|/g, "\\|")} |\n`;
}
writeFileSync(join(root, "docs/WIDGETS.md"), out);
console.log(`docs/WIDGETS.md: ${rows.length} widgets`);
