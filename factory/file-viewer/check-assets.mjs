// A missing Worker or WASM file silently loses a format, so the image's build fails instead:
// every asset that web-full's manifest and the asset packs declare must be in `root`.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const root = process.argv[2];
const manifests = [
  join(root, "flyfish-viewer-assets.json"),
  ...process.argv.slice(3),
];
const walk = (value, visit) => {
  if (Array.isArray(value)) value.forEach((item) => walk(item, visit));
  else if (value && typeof value === "object") {
    visit(value);
    Object.values(value).forEach((item) => walk(item, visit));
  }
};
let declared = 0;
const missing = [];
for (const manifest of manifests) {
  walk(JSON.parse(readFileSync(manifest, "utf8")), (entry) => {
    if (typeof entry.defaultPath !== "string") return;
    declared += 1;
    if (!existsSync(join(root, entry.defaultPath))) missing.push(entry.defaultPath);
  });
}
if (declared === 0 || missing.length > 0) {
  console.error(`declared ${declared}, missing: ${missing.join(" ")}`);
  process.exit(1);
}
console.log(`all ${declared} declared assets are present`);
