#!/usr/bin/env node

import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { repositoryRoot } from "./artifact-paths.mjs";

const requiredIcons = [
  "icons/32x32.png",
  "icons/128x128.png",
  "icons/128x128@2x.png",
  "icons/icon.icns",
  "icons/icon.ico",
];

const applications = {
  ToskLight: path.join(repositoryRoot, "apps/light-desktop/src-tauri"),
  "Hardware Controls": path.join(repositoryRoot, "apps/light-hardware-controls/src-tauri"),
  "Viz Editor": path.join(repositoryRoot, "apps/viz-editor/src-tauri"),
};

for (const application of Object.values(applications)) {
  const config = JSON.parse(fs.readFileSync(path.join(application, "tauri.conf.json"), "utf8"));
  assert.deepEqual(config.bundle?.icon, requiredIcons, `${application} must declare the complete desktop icon set`);
  for (const icon of requiredIcons) {
    const iconPath = path.join(application, icon);
    assert.ok(fs.statSync(iconPath).size > 0, `${iconPath} must exist and not be empty`);
  }
}

// Every application an operator can see at once needs its own icon. The Viz Editor shipped with a
// copy of the Hardware Controls artwork until it was badged, so this compares all three.
const digest = (file) => crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
const names = Object.keys(applications);
for (let first = 0; first < names.length; first += 1) {
  for (let second = first + 1; second < names.length; second += 1) {
    assert.notEqual(
      digest(path.join(applications[names[first]], "icons/icon.png")),
      digest(path.join(applications[names[second]], "icons/icon.png")),
      `${names[first]} and ${names[second]} must use distinct icons`,
    );
  }
}

// The visualizer is not a Tauri application, so nothing generates an icon set for it. It draws its
// window and corner mark from the Viz Editor's icon, and its macOS bundle from the same .icns.
const visualizerIcon = fs.readFileSync(path.join(repositoryRoot, "apps/viz-renderer/src/png.rs"), "utf8");
assert.ok(
  visualizerIcon.includes('include_bytes!("../../viz-editor/src-tauri/icons/128x128.png")'),
  "the visualizer must show the badged Viz icon rather than the plain ToskLight one",
);
const visualizerBundler = fs.readFileSync(path.join(repositoryRoot, "tools/bundle-visualizer-macos.sh"), "utf8");
assert.ok(
  visualizerBundler.includes("apps/viz-editor/src-tauri/icons/icon.icns"),
  "the visualizer's macOS bundle must carry the Viz icon",
);

for (const renderer of ["tools/build_html_manual.py", "tools/manual/build_pdf.py"]) {
  const source = fs.readFileSync(path.join(repositoryRoot, renderer), "utf8");
  assert.ok(source.includes('ROOT / "apps" / "light-desktop"'), `${renderer} must use the current ToskLight application icon`);
  assert.ok(!source.includes('ROOT / "apps" / "control-ui"'), `${renderer} must not use the retired control-ui path`);
}

console.log("ToskLight desktop icon configuration is complete and application-specific.");
