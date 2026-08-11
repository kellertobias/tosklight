#!/usr/bin/env node
// Stamp the workspace version into the assembled landing page and build its screenshot
// gallery from the reviewed Storybook marketing screenshots.

import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { artifactPaths } from "./artifact-paths.mjs";
import {
	normalizePublicPerformanceStatus,
	renderCompactPerformanceSummary,
	renderPerformancePage,
} from "./performance-publication.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SCREENSHOTS = resolve(
  process.env.LIGHT_MARKETING_SCREENSHOTS_DIR ??
    resolve(ROOT, "docs/marketing/assets/screenshots"),
);
const MARKETING_MANIFEST = resolve(ROOT, "docs/marketing/screenshot-manifest.json");
const DEMO_DIRECTORY = resolve(artifactPaths.visual, "product-demo");
const PERFORMANCE_STATUS_FILE = process.env.LIGHT_PERFORMANCE_STATUS_FILE;

// The manifest order is the reviewed marketing-gallery order. Its files are
// deterministic Storybook captures, separate from the Help/manual screenshot set.
const marketingManifest = JSON.parse(readFileSync(MARKETING_MANIFEST, "utf8"));
if (marketingManifest.version !== 1 || !Array.isArray(marketingManifest.entries)) {
  throw new Error(`Invalid marketing screenshot manifest: ${MARKETING_MANIFEST}`);
}
const GALLERY = marketingManifest.entries;

const REPOSITORY = "kellertobias/tosklight";

// Release asset URLs are deterministic: releases/download/<tag>/<file>. Every file name
// here is one the release workflow's staging step chooses explicitly, so this list and
// .github/workflows/release.yml must be changed together.
const PLATFORMS = [
  {
    title: "macOS",
    note: "Apple Silicon (M1 and later). Intel Macs are not supported.",
    assets: [
      {
        kind: "Desk, Headless, PreViz, and Media bundle",
        file: () => "tosklight-bundle-macos_arm64.zip",
      },
    ],
  },
  {
    title: "Windows",
    note: "64-bit. The installer is unsigned — SmartScreen will ask you to confirm.",
    assets: [
      {
        kind: "Desk, Headless, PreViz, and Media bundle",
        file: () => "tosklight-bundle-windows_amd64.zip",
      },
    ],
  },
  {
    title: "Linux (x86_64)",
    note: "64-bit x86. AppImage runs anywhere; the .deb suits Debian and Ubuntu.",
    assets: [
      {
        kind: "Desk, Headless, PreViz, and Media bundle",
        file: () => "tosklight-bundle-linux_amd64.zip",
      },
    ],
  },
  {
    title: "Raspberry Pi",
    note:
      "Pi 4 and Pi 5 on 64-bit Raspberry Pi OS. Statically linked, so it runs on any " +
      "release. Headless and Media only — run the desk from a browser on another machine.",
    assets: [
      {
        kind: "Headless and Media bundle",
        file: () => "tosklight-bundle-linux_arm64.zip",
      },
    ],
  },
];
const PORTABLE_ASSETS = [
  {
    kind: "Default demo show (portable show file)",
    file: () => "assets-demo-show.show",
  },
  {
    kind: "Operator handbook (PDF)",
    file: () => "assets-handbook.pdf",
  },
];

const target = process.argv[2];
if (!target) {
  console.error("usage: node tools/render-landing-page.mjs <index.html>");
  process.exit(2);
}
const siteRoot = dirname(target);

const cargo = readFileSync(resolve(ROOT, "Cargo.toml"), "utf8");
const version =
  process.env.LIGHT_RELEASE_VERSION ??
  /\[workspace\.package\][^[]*?\nversion = "([^"]*)"/.exec(cargo)?.[1];
if (!version) {
  console.error("error: could not read [workspace.package] version from Cargo.toml");
  process.exit(1);
}
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/u.test(version)) {
  console.error(`error: release version is not valid SemVer: ${version}`);
  process.exit(1);
}

const escapeHtml = (value) =>
  value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

mkdirSync(resolve(siteRoot, "screenshots"), { recursive: true });
const figures = GALLERY.map(({ file, title, caption }) => {
  const source = resolve(SCREENSHOTS, file);
  const name = file.replace(/\//g, "-");
  copyFileSync(source, resolve(siteRoot, "screenshots", name));
  return (
    `<figure class="shot">` +
    `<img src="screenshots/${escapeHtml(name)}" alt="${escapeHtml(title)} — ${escapeHtml(caption)}" loading="lazy" decoding="async">` +
    `<figcaption><strong>${escapeHtml(title)}</strong> ${escapeHtml(caption)}</figcaption>` +
    `</figure>`
  );
}).join("\n        ");

const tag = `v${version}`;
const releaseUrl = `https://github.com/${REPOSITORY}/releases/tag/${tag}`;
const downloadUrl = (file) =>
  `https://github.com/${REPOSITORY}/releases/download/${tag}/${file}`;

const downloadCard = ({ title, note, assets }) => {
  const rows = assets
    .map(({ kind, file }) => {
      const name = file(version);
      return (
        `<li><div class="download-meta">` +
        `<span class="download-kind">${escapeHtml(kind)}</span>` +
        `<code>${escapeHtml(name)}</code></div>` +
        `<a class="download-button" href="${escapeHtml(downloadUrl(name))}" download>Download</a></li>`
      );
    })
    .join("");
  return (
    `<div class="platform"><h3>${escapeHtml(title)}</h3>` +
    `<p class="platform-note">${escapeHtml(note)}</p>` +
    `<ul class="download-list">${rows}</ul></div>`
  );
};
const downloads = [
  downloadCard({
    title: "Portable show",
    note: "The reviewed completed demo show that a new desk opens as Default Stage Show.",
    assets: PORTABLE_ASSETS,
  }),
  ...PLATFORMS.map(downloadCard),
].join("\n        ");

let performanceCandidate;
if (PERFORMANCE_STATUS_FILE && existsSync(PERFORMANCE_STATUS_FILE)) {
  try {
    performanceCandidate = JSON.parse(readFileSync(PERFORMANCE_STATUS_FILE, "utf8"));
  } catch {
    // The public site must remain deployable when an infrastructure failure produced
    // an invalid status artifact. The explicit unknown state preserves that distinction.
  }
}
const performance = normalizePublicPerformanceStatus(performanceCandidate, {
  version,
  releaseUrl,
});
mkdirSync(resolve(siteRoot, "performance"), { recursive: true });
writeFileSync(
  resolve(siteRoot, "performance", "status.json"),
  `${JSON.stringify(performance, null, 2)}\n`,
);
writeFileSync(
  resolve(siteRoot, "performance", "index.html"),
  renderPerformancePage(performance),
);
const performanceMarkup = renderCompactPerformanceSummary(performance);

const demoSources = [
  {
    source: resolve(DEMO_DIRECTORY, "tosklight-product-demo-h265.mp4"),
    file: "tosklight-product-demo-h265.mp4",
    type: "video/mp4",
  },
  {
    source: resolve(DEMO_DIRECTORY, "tosklight-product-demo.webm"),
    file: "tosklight-product-demo.webm",
    type: "video/webm",
  },
].filter(({ source }) => existsSync(source));

if (process.env.LIGHT_REQUIRE_DEMO_VIDEO === "1" && !demoSources.length) {
  console.error(`error: product-demo video artifact is missing below ${DEMO_DIRECTORY}`);
  process.exit(1);
}

let demo = `<p class="section-lede">The generated product demo is not available in this local Pages build.</p>`;
if (demoSources.length) {
  mkdirSync(resolve(siteRoot, "media"), { recursive: true });
  for (const { source, file } of demoSources) {
    copyFileSync(source, resolve(siteRoot, "media", file));
  }
  const sources = demoSources
    .map(({ file, type }) => `<source src="media/${file}" type="${type}">`)
    .join("");
  demo =
    `<video class="product-demo" controls preload="metadata">${sources}` +
    `Your browser does not support the generated product-demo video.</video>`;
}

let page = readFileSync(target, "utf8");
for (const [placeholder, replacement] of [
  ["__VERSION__", version],
  ["__GALLERY__", figures],
  ["__DEMO__", demo],
  ["__DOWNLOADS__", downloads],
  ["__RELEASE_URL__", releaseUrl],
  ["__PERFORMANCE__", performanceMarkup],
]) {
  if (!page.includes(placeholder)) {
    console.error(`error: ${target} has no ${placeholder} placeholder`);
    process.exit(1);
  }
  page = page.replaceAll(placeholder, replacement);
}
writeFileSync(target, page);
const assetCount =
  PORTABLE_ASSETS.length + PLATFORMS.reduce((total, { assets }) => total + assets.length, 0);
console.log(
  `Stamped ${target} with version ${version}, ${GALLERY.length} screenshots, ` +
    `${assetCount} download links for ${tag}`,
);
