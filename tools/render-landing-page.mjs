#!/usr/bin/env node
// Stamp the workspace version into the assembled landing page and build its screenshot
// gallery from reviewed marketing captures and explicitly documented static stills.

import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { artifactPaths } from "./artifact-paths.mjs";
import {
	normalizePublicPerformanceStatus,
	renderPerformancePage,
} from "./performance-publication.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SCREENSHOTS = resolve(
  process.env.LIGHT_MARKETING_SCREENSHOTS_DIR ??
    resolve(ROOT, "docs/marketing/assets/screenshots"),
);
const MARKETING_MANIFEST = resolve(ROOT, "docs/marketing/screenshot-manifest.json");
const PRODUCT_ICONS = [
  ["assets/branding/ToskLight Control.png", "tosklight-control.png"],
  ["assets/branding/ToskLight Architect.png", "tosklight-architect.png"],
  ["assets/branding/ToskLight Pixel.png", "tosklight-pixel.png"],
];
const DEMO_DIRECTORY = resolve(artifactPaths.visual, "product-demo");
const PERFORMANCE_STATUS_FILE = process.env.LIGHT_PERFORMANCE_STATUS_FILE;

// The manifest order is the reviewed marketing-gallery order. Most files are deterministic
// Storybook captures; native-only surfaces may name a reviewed static source explicitly.
const marketingManifest = JSON.parse(readFileSync(MARKETING_MANIFEST, "utf8"));
if (marketingManifest.version !== 1 || !Array.isArray(marketingManifest.entries)) {
  throw new Error(`Invalid marketing screenshot manifest: ${MARKETING_MANIFEST}`);
}
const GALLERY = marketingManifest.entries;

const PRODUCT_GALLERIES = {
	control: ["application-overview.png", "tracked-programming.png", "dynamics.png"],
	architect: ["architect-renderer-ultra.png", "architect-cad.png"],
	pixel: ["pixel-eight-layer-composite.png", "media-server-visualizers.png", "media-server-playback.png"],
};
const DETAIL_GALLERIES = {
	control: ["control-desk.png", "control-programmer.png", "control-groups.png", "control-cues.png", "control-preload-go.png", "control-stage.png", "control-fixture-library.png", "control-output-engine.png"],
	pixel: PRODUCT_GALLERIES.pixel,
	architect: PRODUCT_GALLERIES.architect,
};

const galleryByFile = new Map(GALLERY.map((entry) => [entry.file, entry]));
for (const [product, files] of Object.entries(PRODUCT_GALLERIES)) {
	for (const file of files) {
		if (!galleryByFile.has(file)) {
			throw new Error(`Marketing gallery for ${product} references missing ${file}`);
		}
	}
}

const REPOSITORY = "kellertobias/tosklight";

// Release asset URLs are deterministic: releases/download/<tag>/<file>. Every file name
// here is one the release workflow's staging step chooses explicitly, so this list and
// .github/workflows/release.yml must be changed together.
const PLATFORMS = [
  {
    id: "macos",
    title: "macOS (Apple Silicon)",
    note: "Apple Silicon (M1 and later). Intel Macs are not supported.",
    file: "tosklight-bundle-macos_arm64.zip",
  },
  {
    id: "windows",
    title: "Windows",
    note: "64-bit. The installer is unsigned — SmartScreen will ask you to confirm.",
    file: "tosklight-bundle-windows_amd64.zip",
  },
  {
    id: "linux",
    title: "Linux (x86_64)",
    note: "64-bit x86. AppImage runs anywhere; the .deb suits Debian and Ubuntu.",
    file: "tosklight-bundle-linux_amd64.zip",
  },
  {
    id: "raspberry-pi",
    title: "Raspberry Pi",
    note:
      "Pi 4 and Pi 5 on 64-bit Raspberry Pi OS. Statically linked, so it runs on any " +
      "release. Headless and Media only — run the desk from a browser on another machine.",
    file: "tosklight-bundle-linux_arm64.zip",
  },
];
const PORTABLE_ASSETS = [
  {
    kind: "Demo show",
    note: "Open a complete example production and explore the suite with a real starting point.",
    file: () => "assets-demo-show.show",
  },
  {
    kind: "Operator handbook",
    note: "Keep the operator handbook ready for planning, setup, and show time.",
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
const renderFigure = ({ file, sourceFile, title, caption, wide }) => {
  const source = sourceFile ? resolve(ROOT, sourceFile) : resolve(SCREENSHOTS, file);
  const name = file.replace(/\//g, "-");
  copyFileSync(source, resolve(siteRoot, "screenshots", name));
  return (
    `<figure class="shot${wide ? " shot-wide" : ""}">` +
    `<button class="shot-image" type="button" aria-label="Enlarge ${escapeHtml(title)}">` +
			`<img src="screenshots/${escapeHtml(name)}" alt="${escapeHtml(title)}: ${escapeHtml(caption)}" loading="lazy" decoding="async"></button>` +
    `<figcaption><strong>${escapeHtml(title)}</strong> ${escapeHtml(caption)}</figcaption>` +
    `</figure>`
  );
};

// Copy the complete reviewed set so published README and deep links remain stable, while the
// landing page itself stays focused on a small, intentional selection for each product.
for (const entry of GALLERY) renderFigure(entry);
const productGallery = (product) =>
	PRODUCT_GALLERIES[product]
		.map((file) => renderFigure(galleryByFile.get(file)))
		.join("\n        ");

mkdirSync(resolve(siteRoot, "product-icons"), { recursive: true });
for (const [source, file] of PRODUCT_ICONS) {
  copyFileSync(resolve(ROOT, source), resolve(siteRoot, "product-icons", file));
}

const tag = `v${version}`;
const releaseUrl = `https://github.com/${REPOSITORY}/releases/tag/${tag}`;
const downloadUrl = (file) =>
  `https://github.com/${REPOSITORY}/releases/download/${tag}/${file}`;

const platformOptions = PLATFORMS.map(
	({ id, title, file, note }) =>
		`<option value="${escapeHtml(id)}" data-label="${escapeHtml(title)}" data-url="${escapeHtml(downloadUrl(file))}" data-file="${escapeHtml(file)}" data-note="${escapeHtml(note)}">${escapeHtml(title)}</option>`,
).join("");
const initialPlatform = PLATFORMS[0];
const additionalDownloads = PORTABLE_ASSETS.map(({ kind, note, file }) => {
	const name = file(version);
	return `<article class="additional-download"><div><h3>${escapeHtml(kind)}</h3><p>${escapeHtml(note)}</p></div><a class="download-button" href="${escapeHtml(downloadUrl(name))}" download>Download</a></article>`;
}).join("");

mkdirSync(resolve(siteRoot, "downloads"), { recursive: true });
writeFileSync(
	resolve(siteRoot, "downloads", "index.html"),
	`<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta name="theme-color" content="#07090d" />
    <meta name="description" content="Download current ToskLight test builds for macOS, Windows, Linux, and Raspberry Pi." />
    <title>Download ToskLight</title>
    <link rel="icon" href="../icon.png" type="image/png" />
    <link rel="stylesheet" href="../site.css" />
  </head>
  <body class="downloads-page">
    <a class="skip-link" href="#downloads">Skip to downloads</a>
    <nav class="topbar shell" aria-label="Downloads navigation">
      <a class="wordmark" href="../"><img src="../icon.png" alt="" /><span>ToskLight</span></a>
      <div class="nav-links">
        <a href="../#applications">Applications</a>
        <a href="../performance/">Development</a>
        <a href="../manual/">Manual</a>
        <a class="nav-cta" href="${escapeHtml(releaseUrl)}">Release notes</a>
      </div>
    </nav>
    <main>
      <header class="download-hero shell">
        <p class="eyebrow">Current test release · v${escapeHtml(version)}</p>
        <h1>Download ToskLight.</h1>
        <p>Choose the bundle for the computer running your show. The suite is still moving toward its first production release, so check each product's status before using a build on site.</p>
      </header>
      <section class="download-status shell" aria-label="Product release status">
        <article><img src="../product-icons/tosklight-control.png" alt="" /><div><span class="status status-development"><i></i>Currently in development</span><strong>ToskLight Control</strong></div></article>
        <article><img src="../product-icons/tosklight-pixel.png" alt="" /><div><span class="status status-candidate"><i></i>Release candidate</span><strong>ToskLight Pixel</strong></div></article>
        <article><img src="../product-icons/tosklight-architect.png" alt="" /><div><span class="status status-planned"><i></i>Published with Pixel</span><strong>ToskLight Architect</strong></div></article>
      </section>
      <section class="download-content shell" id="downloads">
        <div class="download-heading">
          <div><p class="eyebrow">Your build</p><h2>Download for your platform.</h2></div>
          <p>Every desktop bundle contains Control, Pixel, Architect, and the standalone server where supported. Raspberry Pi packages contain the headless and media applications for browser-connected operation.</p>
        </div>
        <div class="platform-picker">
          <label for="platform-select">Download for</label>
          <select id="platform-select">${platformOptions}</select>
          <p id="platform-note">${escapeHtml(initialPlatform.note)}</p>
          <a class="button button-primary platform-download" id="platform-download" href="${escapeHtml(downloadUrl(initialPlatform.file))}" download>Download for ${escapeHtml(initialPlatform.title)}</a>
          <p class="platform-suite"><strong>The complete ToskLight suite.</strong> Control, Pixel, Architect, and the standalone server where supported are included in one download.</p>
        </div>
        <section class="additional-downloads" aria-labelledby="additional-downloads-title">
          <div><p class="eyebrow">Useful alongside the suite</p><h2 id="additional-downloads-title">Start with a show and a handbook.</h2></div>
          <div class="additional-download-grid">${additionalDownloads}</div>
        </section>
        <aside class="install-notes" aria-label="Installation notes">
          <div class="unsigned-warning"><strong>Warning: unsigned test build</strong><p>Extra installation steps are required on macOS and Windows. Signed builds are planned for the production release.</p><a class="download-button" href="../manual/#page-00-quick-start-01-installation-and-first-start-md">Installation steps</a></div>
          <div><strong>Need the details?</strong><p>Read the <a href="../manual/#page-00-quick-start-01-installation-and-first-start-md">installation guide</a>, review <a href="${escapeHtml(releaseUrl)}">release notes</a>, or inspect the measured evidence on <a href="../performance/">Development</a>.</p></div>
        </aside>
      </section>
    </main>
    <footer><div class="shell download-footer"><p>Free to use, modify, and operate under the <a href="../license/">ToskLight Community License</a>. · <a href="../third-party-licenses.html">Third-party licenses</a> · <a href="../imprint/">Imprint &amp; Privacy</a></p><a href="../">← Back to ToskLight</a></div></footer>
    <script src="../site.js"></script>
  </body>
</html>`,
);

const productPages = [
	{
		path: "control",
		name: "ToskLight Control",
		icon: "tosklight-control.png",
		category: "Open source lighting control desk",
		title: "ToskLight Control",
		intro:
			"Full tracking, preset and group referencing, a built-in visualizer and GDTF/MVR support, driving DMX over network protocols from your computer.",
		benefits: [
			["Build a show that survives change.", "Groups and presets keep a look connected. Re-focus a light or correct a house white once, and the cues that use it stay current."],
			["Program at your own pace.", "Point at fixtures and shape a look directly, then take on the desk-style command line, tracking, and deeper programming when you are ready."],
			["Know what is on stage.", "Follow each fixture from the programmer through playback to the final output, and check the stage picture before the doors open."],
		],
		images: DETAIL_GALLERIES.control,
		manual: "../manual/#page-10-desk-index-md",
		sections: [
			["Why it exists", "Professional concepts should survive a smaller budget.", "The industry-standard consoles are excellent and worth every penny. If you can afford one, buy one. The difficulty sits below that budget: smaller productions fall back on open or low-cost software built on entirely different concepts, so the workflow does not carry over. You learn a model that stops applying the day you decide to take this seriously.", "ToskLight bridges that gap: professional concepts, free to use, running on macOS, Windows and Linux. It follows the conventions established by industry-standard desks, including full tracking and proper referencing of groups and presets, so the hours you put in here carry over to the craft you want to practise. The entry point is different: point at things to get a show running, then grow into the command line and the deeper structure as you need them."],
			["The desk", "Everything an operator touches, on one surface.", "The Fixture Sheet lists every patched fixture with its live dimmer, colour, position, beam and focus values, and marks whether each one comes from the Programmer, playback or the default. It answers the operator's real question: what is this light doing, and who told it to?", "Built-in views cover the work most shows need, while Desktops let you arrange your own. The lower section switches between the Programmer and freely configurable playbacks, reshaping itself when a hardware surface is connected."],
			["The programmer", "A programmer that works the way desks work.", "Intensity, colour, position, beam, shapers, focus, control and media sit in familiar attribute banks. Everything is reachable through the touch surface or typed on the command line, which becomes the faster path once it is in your fingers.", "Values stay in the Programmer until they are recorded. Blind, preview and highlight let you build the next look while the current one remains live."],
			["Referencing", "Cues point at groups and presets, not at numbers.", "Groups collect the fixtures you actually talk about during a show and presets hold the looks those groups take. Cues reference those objects instead of storing a fixed value on every fixture.", "Change the house warm preset once and every cue that used it follows. Repatch a group after a fixture fails and the cues keep working."],
			["Tracking", "Full tracking, so a cue only stores what it changes.", "Each cue stores only what it changes and everything else tracks through from the cues before it. A cue list reads as a sequence of intentions rather than a series of complete stage states.", "That saves the afternoon when a lamp is missing from a look: add it once and it stays in every cue that follows, instead of being pasted into each of them."],
			["Visualizer", "A stage view built in, not bolted on.", "A 3D view of the rig is built in. Because every fixture has a real position, those positions drive selection: grab what is on stage left instead of remembering fixture numbers.", "Confirm a look, point the moving lights, and see where beams land without standing in the venue or keeping a second copy of the rig in sync."],
			["Output", "DMX over the network, on a fixed render deadline.", "Logical show universes map to Art-Net or sACN destinations as explicit routes, so a show file stays independent of the network layout it runs on tonight.", "Frames go out on a fixed schedule rather than whenever the engine finishes one, keeping the rate consistent whether the desk is idle or busy."],
		],
		coming: [["Timecode and timeline editor", "In development · public beta"], ["Plugin support", "Being built"], ["Position / PSN support", "Being built"]],
	},
	{
		path: "pixel",
		name: "ToskLight Pixel",
		icon: "tosklight-pixel.png",
		category: "Open source live media server",
		title: "ToskLight Pixel",
		intro:
			"ToskLight Pixel brings video, images, text, generated visuals, and effects into the same live decision-making space as the rest of the show.",
		benefits: [
			["Eight layers, one output.", "Build a rich visual picture from up to eight independently controlled layers, including masks and master shapes that keep the composition intentional."],
			["A library made for live work.", "Bring in videos and transparent images, add flexible text such as time and text files, and use live sound-driven visualizers. Pixel prepares imported media for efficient playback."],
			["Direct control or network control.", "Use familiar desk-style live controls when you want to operate Pixel on its own, or connect it to any Art-Net or sACN desk. CITP/MSEX keeps media choices and previews close to the console."],
		],
		images: DETAIL_GALLERIES.pixel,
		manual: "../manual/#page-30-tosklight-media-index-md",
		sections: [
			["Layers", "Image and video layers, up to eight at once.", "Build a complete visual output from up to eight independently controlled image and video layers. Shape their position, opacity, masks, frames, and effects while keeping the complete composition visible.", "The layers are designed for a live show: each one can be controlled directly in Pixel or reached from the lighting desk over sACN and Art-Net."],
			["Text", "Text that belongs in the picture.", "A layer can hold editable text created in the application. Use it for a title, a schedule, a timer, a clock, or any other static or dynamic message the show needs.", "Text stays part of the same layered composition as video and images, so it can be placed and shaped as deliberately as every other visual."],
			["Visualizers", "Sound becomes part of the visual system.", "Pixel includes proof-of-concept visualizers driven by live audio input. They can be freely configured to suit the room, the music, and the rest of the composition.", "The visualizer catalogue will grow as the application develops. Today it provides a configurable starting point rather than claiming a finished library."],
			["Output", "Map one picture across the real room.", "Map Pixel's output across multiple projection surfaces and DMX-controlled fixtures. Each fixture can be reached remotely from the desk, so a show can send the same fixture on another universe and Pixel merges its media colour information into the result.", "Use Art-Net, sACN, CITP, and MSEX to connect Pixel to the rest of the show and make media choices available where the operator needs them."],
		],
		coming: [["Output mapping", "Being tested. Single output through the application window is the well-tested path today."]],
	},
	{
		path: "architect",
		name: "ToskLight Architect",
		icon: "tosklight-architect.png",
		category: "Open source visualizer and show CAD",
		title: "ToskLight Architect",
		intro:
			"ToskLight Architect gives the production a shared plan: draw the venue, place the rig, print the paperwork, and see the final picture before the first fixture is flown.",
		benefits: [
			["Plan and print with confidence.", "Create a measured CAD plan for fixtures, scenery, media surfaces, and installed geometry, then print the information the crew needs."],
			["Visualise the actual rig.", "Use detailed fixture models, beams, focus, colour, haze, and venue geometry to make creative choices before the audience arrives."],
			["Stay open on site.", "Visualise real Art-Net or sACN from ToskLight or another compatible desk, so the plan remains useful across the production."],
		],
		images: PRODUCT_GALLERIES.architect,
		manual: "../manual/#page-20-tosklight-previz-index-md",
		sections: [
			["Interchange", "Bring the rig in. Take the rig out.", "Import and export MVR so a production can move its rig between planning tools without rebuilding it by hand. Architect also supports GDTF fixture files, including their modes and physical information.", "GDTF fixtures are mapped to the default 3D models shipped with Architect, or they can carry their own images and 3D models when the fixture package provides them."],
			["CAD", "Look at the plan from the angle the job needs.", "Use the CAD view to inspect the room and rig from different angles, select the relevant section, and print clear paperwork for the production.", "Print whole plans or cutaway sections, so a crew member can take exactly the view they need for the part of the venue they are working on."],
			["Moving scenes", "Make the scene move with the fixture.", "Set a fixture, represented as a 3D point, as the master for other fixtures or scene elements. When that master is controlled through PSN or DMX, the connected elements follow its movement.", "This makes it possible to build moving scenes and keep surrounding objects in the visualizer connected to the same control that moves the real fixture."],
			["Lasers", "Simulate the path, not just the beam.", "Architect can simulate lasers from a scan path produced by a JavaScript function supplied by the user. The function receives DMX input and returns the path that the renderer draws.", "That gives the designer control over the laser's shape, brightness, scan speed, and position, and leaves room to build a laser effect that matches the specific production."],
			["Early prototypes", "See the right trigger, with more to come.", "Architect includes early prototypes for physical elements such as falling curtains and pyro elements such as flame jets and sparklers. They are intentionally rudimentary today.", "They are useful for confirming that the intended DMX channel was triggered, not yet for final visual realism. These elements may become more detailed as the application grows."],
		],
		coming: [["Media server integration", "Being tested"], ["Paperwork rendering", "Being improved. A real Architect-generated printout will be added here when it is ready to capture."]],
	},
];

for (const product of productPages) {
	const gallery = product.images
		.map((file) => {
			const entry = galleryByFile.get(file);
			const name = file.replace(/\//g, "-");
			return `<figure class="shot"><img src="../screenshots/${escapeHtml(name)}" alt="${escapeHtml(entry.title)}" loading="lazy"><figcaption><strong>${escapeHtml(entry.title)}</strong> ${escapeHtml(entry.caption)}</figcaption></figure>`;
		})
		.join("");
	const benefits = product.benefits
		.map(([title, copy]) => `<article><h2>${escapeHtml(title)}</h2><p>${escapeHtml(copy)}</p></article>`)
		.join("");
	const sections = product.sections
		? `<section class="product-detail-story shell">${product.sections.map(([label, title, first, second], index) => { const entry = galleryByFile.get(product.images[index % product.images.length]); const image = product.images[index % product.images.length].replace(/\//g, "-"); return `<article><div class="product-detail-copy"><p class="eyebrow">${escapeHtml(label)}</p><h2>${escapeHtml(title)}</h2></div><div class="product-detail-text"><p>${escapeHtml(first)}</p><p>${escapeHtml(second)}</p></div><figure class="shot"><img src="../screenshots/${escapeHtml(image)}" alt="${escapeHtml(entry.title)}" loading="lazy"><figcaption><strong>${escapeHtml(entry.title)}</strong> ${escapeHtml(entry.caption)}</figcaption></figure></article>`; }).join("")}</section>`
		: "";
	const coming = product.coming ? `<section class="product-detail-coming shell"><p class="eyebrow">What is coming</p><h2>Still moving forward.</h2><div>${product.coming.map(([title, status]) => `<article><strong>${escapeHtml(title)}</strong><p>${escapeHtml(status)}</p></article>`).join("")}</div></section>` : "";
	mkdirSync(resolve(siteRoot, product.path), { recursive: true });
	writeFileSync(
		resolve(siteRoot, product.path, "index.html"),
		`<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><meta name="theme-color" content="#07090d"><title>${escapeHtml(product.name)}</title><link rel="icon" href="../icon.png" type="image/png"><link rel="stylesheet" href="../site.css"></head><body class="product-detail-page product-detail-${escapeHtml(product.path)}"><nav class="topbar shell" aria-label="Primary navigation"><a class="wordmark" href="../"><img src="../icon.png" alt=""><span>ToskLight</span></a><div class="nav-links"><a class="is-active" href="../#applications">Applications</a><a href="../performance/">Development</a><a href="../manual/">Manual</a><a class="nav-cta" href="../downloads/">Downloads</a></div></nav><main><header class="product-detail-hero shell"><p class="product-category">${escapeHtml(product.category)}</p><div class="product-detail-title-row"><h1>${escapeHtml(product.title)}</h1><img src="../product-icons/${escapeHtml(product.icon)}" alt="${escapeHtml(product.name)} icon"></div><p>${escapeHtml(product.intro)}</p><div class="actions"><a class="button button-primary" href="../downloads/">Download</a><a class="button" href="${escapeHtml(product.manual)}">Read the manual</a></div></header><section class="product-detail-benefits shell">${benefits}</section>${sections}${coming}</main><footer><div class="shell download-footer"><p><a href="../license/">ToskLight Community License</a> · <a href="../third-party-licenses.html">Third-party licenses</a> · <a href="../imprint/">Imprint &amp; Privacy</a></p><a href="../">Back to ToskLight</a></div></footer></body></html>`,
	);
}

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

mkdirSync(resolve(siteRoot, "license"), { recursive: true });
const licenseText = readFileSync(resolve(ROOT, "LICENSE"), "utf8");
writeFileSync(
	resolve(siteRoot, "license", "index.html"),
	`<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="theme-color" content="#07090d"><title>ToskLight Community License</title><link rel="icon" href="../icon.png" type="image/png"><link rel="stylesheet" href="../site.css"></head><body class="legal-page"><nav class="topbar shell"><a class="wordmark" href="../"><img src="../icon.png" alt=""><span>ToskLight</span></a><div class="nav-links"><a href="../downloads/">Downloads</a><a href="../performance/">Development</a><a class="nav-cta" href="../">Back to the suite</a></div></nav><main class="document-page shell"><header class="document-hero"><p class="eyebrow">Terms for using ToskLight</p><h1>Community License.</h1><p>Use ToskLight for your shows, study it, and build on it under the terms below.</p></header><article class="license-document"><pre>${escapeHtml(licenseText)}</pre></article></main><footer><div class="shell download-footer"><p><a href="../license.txt">Plain-text version</a> · <a href="../third-party-licenses.html">Third-party licenses</a> · <a href="../imprint/">Imprint &amp; Privacy</a></p><a href="../">← Back to ToskLight</a></div></footer></body></html>`,
);

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
  ["__CONTROL_GALLERY__", productGallery("control")],
  ["__ARCHITECT_GALLERY__", productGallery("architect")],
  ["__PIXEL_GALLERY__", productGallery("pixel")],
  ["__DEMO__", demo],
  ["__RELEASE_URL__", releaseUrl],
]) {
  if (!page.includes(placeholder)) {
    console.error(`error: ${target} has no ${placeholder} placeholder`);
    process.exit(1);
  }
  page = page.replaceAll(placeholder, replacement);
}
writeFileSync(target, page);
const assetCount =
  PORTABLE_ASSETS.length + PLATFORMS.length;
console.log(
  `Stamped ${target} with version ${version}, ${GALLERY.length} screenshots, ` +
    `${assetCount} download links for ${tag}`,
);
