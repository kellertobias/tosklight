#!/usr/bin/env node
// Stamp the workspace version into the assembled landing page and build its screenshot
// gallery from the help screenshots that `npm run test:help-screenshots` regenerates.

import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SCREENSHOTS = resolve(ROOT, "docs/help/assets/screenshots");

// Curated tour of the desk, in the order an operator meets these surfaces. Paths are
// relative to docs/help/assets/screenshots and are always the current generated files.
const GALLERY = [
  { file: "default-desk-overview.png", title: "Desk overview", caption: "Fixture selection, group shortcuts, 3D stage preview, and the live programmer." },
  { file: "fixture-sheet-programmer.png", title: "Fixture sheet", caption: "Per-fixture channel values with the programmer active above them." },
  { file: "cuelist-playback.png", title: "Cue list and playback", caption: "Cue list detail with playback faders and group masters." },
  { file: "software-keypad.png", title: "Software keypad", caption: "The command line and keypad, mirroring the attached hardware surface." },
  { file: "panes/stage.png", title: "Stage view", caption: "3D stage geometry with live output rendered onto the rig." },
  { file: "panes/groups.png", title: "Groups", caption: "Ordered group membership, preserved for value spreading." },
  { file: "workflows/show-patch.png", title: "Patch", caption: "Patching fixtures into universes and addresses." },
  { file: "panes/presets.png", title: "Presets", caption: "Stored looks recalled straight from the pool." },
];

const REPOSITORY = "kellertobias/tosklight";

// Release asset URLs are deterministic: releases/download/<tag>/<file>. Every file name
// here is one the release workflow's staging step chooses explicitly, so this list and
// .github/workflows/release.yml must be changed together.
const PLATFORMS = [
  {
    title: "macOS",
    note: "Apple Silicon (M1 and later). Intel Macs are not supported.",
    assets: [
      { kind: "Desktop application", file: (v) => `tosklight-${v}-macos-arm64.zip` },
      { kind: "Standalone server", file: (v) => `light-server-${v}-macos-arm64.zip` },
    ],
  },
  {
    title: "Windows",
    note: "64-bit. The installer is unsigned — SmartScreen will ask you to confirm.",
    assets: [
      { kind: "Desktop installer", file: (v) => `tosklight-${v}-windows-amd64-setup.exe` },
      { kind: "Standalone server", file: (v) => `light-server-${v}-windows-amd64.zip` },
    ],
  },
  {
    title: "Linux (x86_64)",
    note: "64-bit x86. AppImage runs anywhere; the .deb suits Debian and Ubuntu.",
    assets: [
      { kind: "Desktop application (AppImage)", file: (v) => `tosklight-${v}-linux-amd64.AppImage` },
      { kind: "Desktop package (.deb)", file: (v) => `tosklight-${v}-linux-amd64.deb` },
      { kind: "Standalone server", file: (v) => `light-server-${v}-linux-amd64.zip` },
    ],
  },
  {
    title: "Raspberry Pi",
    note:
      "Pi 4 and Pi 5 on 64-bit Raspberry Pi OS. Statically linked, so it runs on any " +
      "release. Server only — run the desk from a browser on another machine.",
    assets: [
      { kind: "Standalone server (ARM64)", file: (v) => `light-server-${v}-linux-arm64.zip` },
    ],
  },
];

const target = process.argv[2];
if (!target) {
  console.error("usage: node tools/render-landing-page.mjs <index.html>");
  process.exit(2);
}
const siteRoot = dirname(target);

const cargo = readFileSync(resolve(ROOT, "Cargo.toml"), "utf8");
const version = /\[workspace\.package\][^[]*?\nversion = "([^"]*)"/.exec(cargo)?.[1];
if (!version) {
  console.error("error: could not read [workspace.package] version from Cargo.toml");
  process.exit(1);
}

const escape = (value) =>
  value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

mkdirSync(resolve(siteRoot, "screenshots"), { recursive: true });
const figures = GALLERY.map(({ file, title, caption }) => {
  const source = resolve(SCREENSHOTS, file);
  const name = file.replace(/\//g, "-");
  copyFileSync(source, resolve(siteRoot, "screenshots", name));
  return (
    `<figure class="shot">` +
    `<img src="screenshots/${escape(name)}" alt="${escape(title)} — ${escape(caption)}" loading="lazy" decoding="async">` +
    `<figcaption><strong>${escape(title)}</strong> ${escape(caption)}</figcaption>` +
    `</figure>`
  );
}).join("\n        ");

const tag = `v${version}`;
const releaseUrl = `https://github.com/${REPOSITORY}/releases/tag/${tag}`;
const downloadUrl = (file) =>
  `https://github.com/${REPOSITORY}/releases/download/${tag}/${file}`;

const downloads = PLATFORMS.map(({ title, note, assets }) => {
  const rows = assets
    .map(({ kind, file }) => {
      const name = file(version);
      return (
        `<li><div class="download-meta">` +
        `<span class="download-kind">${escape(kind)}</span>` +
        `<code>${escape(name)}</code></div>` +
        `<a class="download-button" href="${escape(downloadUrl(name))}" download>Download</a></li>`
      );
    })
    .join("");
  return (
    `<div class="platform"><h3>${escape(title)}</h3>` +
    `<p class="platform-note">${escape(note)}</p>` +
    `<ul class="download-list">${rows}</ul></div>`
  );
}).join("\n        ");

let page = readFileSync(target, "utf8");
for (const [placeholder, replacement] of [
  ["__VERSION__", version],
  ["__GALLERY__", figures],
  ["__DOWNLOADS__", downloads],
  ["__RELEASE_URL__", releaseUrl],
]) {
  if (!page.includes(placeholder)) {
    console.error(`error: ${target} has no ${placeholder} placeholder`);
    process.exit(1);
  }
  page = page.replaceAll(placeholder, replacement);
}
writeFileSync(target, page);
const assetCount = PLATFORMS.reduce((total, { assets }) => total + assets.length, 0);
console.log(
  `Stamped ${target} with version ${version}, ${GALLERY.length} screenshots, ` +
    `${assetCount} download links for ${tag}`,
);
