#!/usr/bin/env node
// Write one semantic version into every manifest that carries the desk version.
// Used by the Forgejo release job; safe to run by hand.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
  console.error("usage: node tools/set-version.mjs <major.minor.patch>");
  process.exit(2);
}

const JSON_MANIFESTS = [
  "apps/control-ui/package.json",
  "apps/control-ui/src-tauri/tauri.conf.json",
  "apps/hardware-controls/package.json",
  "apps/hardware-controls/src-tauri/tauri.conf.json",
];

for (const relative of JSON_MANIFESTS) {
  const path = resolve(ROOT, relative);
  const source = readFileSync(path, "utf8");
  const updated = source.replace(/^(\s*"version":\s*)"[^"]*"/m, `$1"${version}"`);
  if (updated === source) {
    console.error(`error: no version field updated in ${relative}`);
    process.exit(1);
  }
  writeFileSync(path, updated);
  console.log(`${relative} -> ${version}`);
}

// Only the [workspace.package] version is owned here; member crates inherit it.
const cargoPath = resolve(ROOT, "Cargo.toml");
const cargo = readFileSync(cargoPath, "utf8");
const updatedCargo = cargo.replace(
  /(\[workspace\.package\][^[]*?\nversion = )"[^"]*"/,
  `$1"${version}"`,
);
if (updatedCargo === cargo) {
  console.error("error: no [workspace.package] version updated in Cargo.toml");
  process.exit(1);
}
writeFileSync(cargoPath, updatedCargo);
console.log(`Cargo.toml -> ${version}`);
