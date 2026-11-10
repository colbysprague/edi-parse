// Print the demo's mock filesystem as JSON: { "<path>": "<contents>", ... }.
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";

const root = process.argv[2];
const mounts = [
  ["x12", "examples/x12"],
  ["samples", "data/samples"],
  ["invalid", "data/invalid"],
];

function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = join(dir, e.name);
    if (e.isDirectory()) return walk(p);
    return /\.(edi|x12)$/.test(e.name) ? [p] : [];
  });
}

const fs = {};
for (const [name, src] of mounts) {
  const base = join(root, src);
  for (const file of walk(base).sort()) {
    fs[`${name}/${relative(base, file)}`] = readFileSync(file, "utf8");
  }
}
process.stdout.write(JSON.stringify(fs));
