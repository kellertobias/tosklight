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
  "ToskLight Control": path.join(repositoryRoot, "apps/light-desktop/src-tauri"),
  "Hardware Controls": path.join(repositoryRoot, "apps/light-hardware-controls/src-tauri"),
  "ToskLight Architect": path.join(repositoryRoot, "apps/viz-editor/src-tauri"),
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
assert.equal(
  digest(path.join(applications["ToskLight Control"], "icons/icon.svg")),
  digest(path.join(repositoryRoot, "assets/branding/ToskLight Control.svg")),
  "Control's in-application vector icon must be the approved Control artwork",
);
assert.equal(
  digest(path.join(applications["ToskLight Architect"], "icons/icon.svg")),
  digest(path.join(repositoryRoot, "assets/branding/ToskLight Architect.svg")),
  "Architect's in-application vector icon must be the approved Architect artwork",
);
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
  "the visualizer's macOS bundle must carry the Architect icon",
);

const pixelReferences = [
  ["apps/media/index.html", "assets/branding/ToskLight Pixel.svg"],
  ["apps/media/src/operator/MediaServerSurface.tsx", "assets/branding/ToskLight Pixel.svg"],
  ["crates/media/adapters/runtime/src/presentation.rs", "assets/branding/ToskLight Pixel.png"],
  ["crates/media/adapters/runtime/src/standby.rs", "assets/branding/ToskLight Pixel.png"],
  ["tools/bundle-media-macos.sh", "assets/branding/ToskLight Pixel.png"],
];
for (const [file, reference] of pixelReferences) {
  const source = fs.readFileSync(path.join(repositoryRoot, file), "utf8");
  assert.ok(source.includes(reference), `${file} must use the approved Pixel application icon`);
}

const landingRenderer = fs.readFileSync(
  path.join(repositoryRoot, "tools/render-landing-page.mjs"),
  "utf8",
);
for (const application of ["Control", "Architect", "Pixel"]) {
  assert.ok(
    landingRenderer.includes(`assets/branding/ToskLight ${application}.png`),
    `the public product card must use the approved ${application} icon`,
  );
}

const manualConfig = JSON.parse(
  fs.readFileSync(path.join(repositoryRoot, "docs/help/manual.config.json"), "utf8"),
);
assert.equal(
  manualConfig.logo,
  "../../apps/light-desktop/src-tauri/icons/icon.png",
  "the configured HTML and PDF manual must use the current ToskLight application icon",
);

console.log("ToskLight desktop icon configuration is complete and application-specific.");
