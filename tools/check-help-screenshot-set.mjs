#!/usr/bin/env node
// The help screenshots are produced by two captures in two jobs: Storybook renders the ones a
// story can show, and the live desk renders the ones only a running desk can. Neither job can
// tell whether the set is complete, because neither sees the other's output. This does, once
// both have been assembled, and it is the only place that knows the set must be whole.
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const screenshots = path.join(root, "docs/help/assets/screenshots");
const manifestPath = path.join(root, "docs/help/screenshot-manifest.json");

const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const declared = manifest.entries.map((entry) => entry.file).sort();

function pngsUnder(directory, prefix = "") {
  if (!fs.existsSync(directory)) return [];
  return fs
    .readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory())
        return pngsUnder(path.join(directory, entry.name), relative);
      return entry.name.endsWith(".png") ? [relative] : [];
    })
    .sort();
}

const present = pngsUnder(screenshots);
const missing = declared.filter((file) => !present.includes(file));
const undeclared = present.filter((file) => !declared.includes(file));

// An empty file is the failure this is really guarding against: an upload that raced, or a
// capture that wrote nothing. It ships as a broken image rather than a missing one, which is
// harder to notice.
const empty = present.filter(
  (file) => fs.statSync(path.join(screenshots, file)).size === 0,
);

if (missing.length || undeclared.length || empty.length) {
  for (const file of missing)
    console.error(`error: ${file} is declared but was not captured`);
  for (const file of undeclared)
    console.error(`error: ${file} was captured but is not in the manifest`);
  for (const file of empty) console.error(`error: ${file} is empty`);
  process.exit(1);
}

console.log(
  `Help screenshot set is complete: ${present.length} of ${declared.length} declared files.`,
);
