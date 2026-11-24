#!/usr/bin/env node
// Stamp the workspace version into the assembled landing page and build its screenshot
// gallery from reviewed marketing captures and explicitly documented static stills.

import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { artifactPaths } from "./artifact-paths.mjs";
import { marketingWebImages, responsiveScreenshot } from "./marketing-web-images.mjs";
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
	control: ["application-overview-icons.png", "tracked-programming.png", "dynamics.png"],
	architect: ["architect-native-export-4k.png", "architect-cad.png", "architect-patch.png"],
	pixel: ["pixel-eight-layer-composite.png", "media-server-library.png", "media-server-playback.png"],
};
// Homepage cards explain product capabilities; detail figures retain their capture-specific captions.
const HOMEPAGE_SHOT_COPY = {
	"application-overview-icons.png": { title: "Dynamic Screen Layouts", caption: "Arrange your workspace around the way you program and run the show." },
	"dynamics.png": { title: "Desk-style Effect Engine", caption: "Shape functions and keyframes, distribute effects across the rig, and keep them on the beat." },
	"pixel-eight-layer-composite.png": { title: "Up to Eight Layers", caption: "Combine video, transparent images, visualizers, and text into one live output." },
	"media-server-library.png": { title: "Library Management Built In", caption: "Prepare and organize the visual assets you need for the show." },
	"media-server-playback.png": { title: "Control via DMX or in the Browser", caption: "Run Pixel from a lighting desk over Art-Net or sACN, or operate its live controls in your browser." },
	"architect-native-export-4k.png": { title: "Live 3D Visualizer", caption: "Preview your show live, then create a full-venue Ultra 4K image for your customer." },
	"architect-cad.png": { title: "Generate Your Venue Plans", caption: "Build the venue and rig, then export measured PDF plans for the crew." },
	"architect-patch.png": { title: "Patch Your Fixtures", caption: "Prepare fixture types, modes, universes, and addresses in the same Show." },
};
const DETAIL_GALLERIES = {
	control: ["application-overview-icons.png", "tracked-programming.png", "control-programmer.png", "control-groups.png", "control-cues.png", "control-timecode.png", "control-output-engine.png", ["dynamics.png", "dynamics-projection.png", "dynamics-phase.png"], "control-media-pane.png", ["control-playbacks-software.png", "control-playbacks-hardware.png"]],
	pixel: ["media-server-library.png", "media-server-visualizers.png", "media-server-text.png", "pixel-eight-layer-composite.png", ["pixel-playlists.png", "pixel-schedule.png"], "media-server-outputs.png"],
	architect: [["architect-native-export-4k.png", "architect-wireframe-native.png", "architect-renderer-draft.png", "architect-renderer-standard.png", "architect-renderer-ultra-live.png"], "architect-show-transfer.png", "architect-cad.png", "architect-patch.png", "control-position-reference.png", ["architect-laser-code.png", "architect-laser-output.png"], null, ["architect-media-surface-config.png", "architect-media-crop-config.png", "architect-media-output-config.png"]],
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
const LIGHTBOX = `<dialog class="lightbox" id="image-lightbox" aria-labelledby="lightbox-caption"><button class="lightbox-close" type="button" aria-label="Close enlarged image">Close</button><figure><img id="lightbox-image" alt=""><figcaption id="lightbox-caption"></figcaption></figure></dialog>`;


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

// Generate bounded web derivatives from the reviewed captures.
const webImages = await marketingWebImages(GALLERY, SCREENSHOTS, resolve(siteRoot, "screenshots"));
const renderFigure = ({ file, title, caption, wide }) => {
  // Native inputs are restored and reviewed by the capture job. Pages must consume its artifact,
  // never replace it with an older tracked still from this checkout.
  const webImage = webImages.get(file);
  return (
    `<figure class="shot${wide ? " shot-wide" : ""}">` +
    `<button class="shot-image" type="button" aria-label="Enlarge ${escapeHtml(title)}">` +
			`<img ${responsiveScreenshot(webImage)} alt="${escapeHtml(title)}: ${escapeHtml(caption)}" loading="lazy" decoding="async"></button>` +
    `<figcaption><strong>${escapeHtml(title)}</strong> ${escapeHtml(caption)}</figcaption>` +
    `</figure>`
  );
};

// Copy the complete reviewed set so published README and deep links remain stable, while the
// landing page itself stays focused on a small, intentional selection for each product.
for (const entry of GALLERY) renderFigure(entry);
const productGallery = (product) =>
	PRODUCT_GALLERIES[product]
		.map((file) => renderFigure({ ...galleryByFile.get(file), ...HOMEPAGE_SHOT_COPY[file] }))
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
        <section class="homebrew-install" id="homebrew" aria-labelledby="homebrew-title">
          <div>
            <h2 id="homebrew-title">Install with Homebrew.</h2>
            <p>On an Apple Silicon Mac, use <a href="https://brew.sh/">Homebrew</a> as an alternative to downloading the ZIP. These commands install ToskLight Control, ToskLight Architect with its visualizer, and ToskLight Pixel from GitHub Releases.</p>
            <p>Homebrew also installs FFmpeg for Pixel media import. Testing builds are signed locally and verified during installation; valid Developer ID signatures are preserved.</p>
          </div>
          <div class="homebrew-commands">
            <pre aria-label="Homebrew installation commands" tabindex="0"><code>brew tap kellertobias/tosklight https://github.com/kellertobias/tosklight.git
brew trust --cask kellertobias/tosklight/tosklight
brew install --cask kellertobias/tosklight/tosklight</code></pre>
            <p>To update, run <code>brew update</code>, then <code>brew upgrade --cask --greedy kellertobias/tosklight/tosklight</code>.</p>
            <p>The <a href="https://github.com/kellertobias/tosklight/blob/main/tools/install-homebrew.sh">installation script</a> also refreshes local signatures on existing installations. See the <a href="../manual/#page-00-quick-start-01-installation-and-first-start-md">first-start guide</a> for macOS approval steps.</p>
          </div>
        </section>
        <section class="additional-downloads" aria-labelledby="additional-downloads-title">
          <div><p class="eyebrow">Useful alongside the suite</p><h2 id="additional-downloads-title">Start with a show and a handbook.</h2></div>
          <div class="additional-download-grid">${additionalDownloads}</div>
        </section>
        <aside class="install-notes" aria-label="Installation notes">
          <div class="unsigned-warning"><strong>Testing-build approvals</strong><p>macOS may still require Open Anyway approval after local signing. Unsigned Windows installers can trigger SmartScreen.</p><a class="download-button" href="../manual/#page-00-quick-start-01-installation-and-first-start-md">Installation steps</a></div>
          <div><strong>Need the details?</strong><p>Read the <a href="../manual/#page-00-quick-start-01-installation-and-first-start-md">installation guide</a>, review <a href="${escapeHtml(releaseUrl)}">release notes</a>, or inspect the measured evidence on <a href="../performance/">Development</a>.</p></div>
        </aside>
      </section>
    </main>
    <footer><div class="shell download-footer"><p>Free to use, modify, and operate under the <a href="../license/">ToskLight Community License</a>. · <a href="../third-party-licenses.html">Third-party licenses</a> · <a href="../impressum/">Impressum</a> · <a href="../datenschutz/">Datenschutz</a></p><a href="../">← Back to ToskLight</a></div></footer>
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
			"Full tracking, preset and group referencing, lightweight plan and wireframe Stage views, and GDTF/MVR support, driving DMX over network protocols or supported USB interfaces from your computer. Use Architect for rendered lighting, haze, and shadows.",
		benefits: [
			["Build a show that survives change.", "Groups and presets keep a look connected. Re-focus a light or correct a house white once, and the cues that use it stay current."],
			["Program at your own pace.", "Point at fixtures and shape a look directly, then take on the desk-style command line, tracking, and deeper programming when you are ready."],
			["Know what is on stage.", "Follow each fixture from the programmer through playback to the final output. Inspect placement and aim in Control’s lightweight Stage, and preview rendered lighting in Architect."],
		],
		images: DETAIL_GALLERIES.control,
		manual: "../manual/#page-10-desk-index-md",
		sections: [
			["Why it exists", "Professional concepts should survive a smaller budget.", "The industry-standard consoles are excellent and worth every penny. If you can afford one, buy one. The difficulty sits below that budget: smaller productions fall back on open or low-cost software built on entirely different concepts, so the workflow does not carry over. You learn a model that stops applying the day you decide to take this seriously.", "ToskLight bridges that gap: professional concepts, free to use, running on macOS, Windows and Linux. It follows the conventions established by industry-standard desks, including full tracking and proper referencing of groups and presets, so the hours you put in here carry over to the craft you want to practise. The entry point is different: point at things to get a show running, then grow into the command line and the deeper structure as you need them."],
			["The desk", "Everything an operator touches, on one surface.", "The Fixture Sheet lists every patched fixture with its live dimmer, colour, position, beam and focus values, and marks whether each one comes from the Programmer, playback or the default. It answers the operator's real question: what is this light doing, and who told it to?", "Built-in views cover the work most shows need, while Desktops let you arrange your own. The lower section switches between the Programmer and freely configurable playbacks, reshaping itself when a hardware surface is connected."],
			["The programmer", "A programmer that works the way desks work.", "Intensity, colour, position, beam, shapers, focus, control and media sit in familiar attribute banks. Everything is reachable through the touch surface or typed on the command line, which becomes the faster path once it is in your fingers.", "Values stay in the Programmer until they are recorded. Blind, preview and highlight let you build the next look while the current one remains live."],
			["Referencing", "Cues point at groups and presets, not at numbers.", "Groups collect the fixtures you actually talk about during a show and presets hold the looks those groups take. Cues reference those objects instead of storing a fixed value on every fixture.", "Change the house warm preset once and every cue that used it follows. Repatch a group after a fixture fails and the cues keep working."],
			["Tracking", "Full tracking, so a cue only stores what it changes.", "Each cue stores only what it changes and everything else tracks through from the cues before it. A cue list reads as a sequence of intentions rather than a series of complete stage states.", "That saves the afternoon when a lamp is missing from a look: add it once and it stays in every cue that follows, instead of being pasted into each of them."],
			["Timecode · In development", "Build a show against a shared timeline.", "Place Cue triggers, edit Cue fade points, and arrange audio and Speed Groups on the timeline. The overview keeps navigation close while you work on an individual Cue.", "Still in development — use with caution. Test your show thoroughly before relying on Timecode for a live performance."],
			["Output", "Connect over the network or USB.", "Send DMX through Art-Net, sACN, or supported USB DMX interfaces. Explicit output routes keep the show independent of the network or adapter used at tonight’s venue.", "Frames go out on a fixed schedule, keeping the output rate consistent while you program and run the show."],
			["Dynamics", "Shape movement. Keep it on the beat.", "Build effects for intensity, color, position, and other supported parameters using functions or editable keyframes. Distribute them across the rig in 3D, then shape their ordering and phase to create coordinated movement.", "Like working the sync controls on a DJ desk, five Speed Groups keep chases and Dynamics on a shared beat: tap the tempo, enter BPM, or follow the music through Sound to Light. Choose whether a Dynamic starts immediately, joins the running sync phase, or waits for the next beat or four-beat bar."],
			["Media Server", "Keep your media server close to the desk.", "Browse media and control layers from a compatible media server in your Control workspace. CITP/MSEX supplies names, thumbnails, and previews where supported.", "Use Pixel or another compatible server, and arrange the Media pane alongside your fixtures, presets, and playbacks."],
			["Hardware", "Choose the controls that suit your show.", "Run the desk from its software playbacks or connect an OSC control surface. The attached surface shares Control’s Programmer, command line, and playback pages.", "Custom hardware bridges can use the documented OSC controls and feedback. The UI adapts when hardware connects: a compact playback view frees room when the controller supplies buttons and faders you no longer need at full touchscreen size."],
		],
		coming: [["Position / PSN support", "Public beta"], ["Timecode and timeline editor", "In development · use with caution"], ["Plugin support", "In planning"]],
	},
	{
		path: "pixel",
		name: "ToskLight Pixel",
		icon: "tosklight-pixel.png",
		category: "Open source live media server",
		title: "ToskLight Pixel",
		intro:
			"Play video, layer images and text, and create visuals that react to music. Run Pixel from your lighting desk or use its browser controls to prepare and play content.",
		benefits: [
			["Eight layers, one output.", "Build a rich visual picture from up to eight independently controlled layers, including masks and master shapes that keep the composition intentional."],
			["A library made for live work.", "Bring in videos and transparent images, add flexible text such as time and text files, and use live sound-driven visualizers. Pixel prepares imported media for efficient playback."],
			["Scheduled playback, desk priority.", "Run playlists at fixed dates or every week, with repeats, fades and a fallback. Incoming DMX suspends the schedule until the configured silence timeout expires."],
		],
		images: DETAIL_GALLERIES.pixel,
		manual: "../manual/#page-30-tosklight-media-index-md",
		sections: [
			["Library", "Keep your show media in one place.", "Organize videos and transparent images in folders, then browse thumbnails to choose content for each layer. Text and generated visuals are available from the same library.", "Pixel converts imported media for playback. Each item has a folder and file address so you can select it from your lighting desk."],
			["Visualizers", "Create visuals that follow the music.", "Choose a visualizer and adjust its colours and movement to suit the show. Live audio drives its response to the music.", "Use it as a layer on its own, or combine it with video, images, and text."],
			["Text", "Show a message, clock, or countdown.", "Add titles and audience messages, or count down to doors opening. Edit the text and its appearance in Pixel.", "Place text over video and images, then use the layer controls to adjust its position and opacity."],
			["Layers", "Combine up to eight layers.", "Layer video, images, text, and generated visuals into one output. Adjust each layer’s position and opacity, add masks, and apply effects while watching the combined picture.", "Control each layer in Pixel’s browser interface or from your lighting desk over Art-Net or sACN."],

			["Playlists and schedules", "Schedule playback for the venue.", "Arrange clips with repeats and fades, and schedule them for specific dates or each week. Keep a logo or information image on another layer while the playlist runs.", "Set priorities for overlapping events and choose a fallback playlist for the gaps. Incoming DMX takes over immediately. Scheduled playback resumes after the desk stops sending for a configurable time; missed events are skipped."],
			["Output", "Play to a screen or projector.", "Send the finished picture to a connected display, including HDMI. A compatible lighting desk can browse media names and thumbnails and receive live previews through CITP/MSEX.", "DMX pixel output and multiple outputs are in public beta. Check the mapping on the hardware you plan to use for the show."],
		],
		coming: [["DMX output and multiple outputs", "Public beta"], ["More visualizers", "The visualizer catalogue continues to grow."]],
	},
	{
		path: "architect",
		name: "ToskLight Architect",
		icon: "tosklight-architect.png",
		category: "Open source visualizer and show CAD",
		title: "ToskLight Architect",
		intro:
			"Plan your venue and lighting rig in 3D. Place and patch fixtures, export PDF drawings for the crew, and preview lighting and video before you arrive on site.",
		benefits: [
			["Plan and print with confidence.", "Create a measured CAD plan for fixtures, scenery, media surfaces, and installed geometry, then print the information the crew needs."],
			["Visualise the actual rig.", "Use detailed fixture models, beams, focus, colour, haze, and venue geometry to make creative choices before the audience arrives."],
			["Stay open on site.", "Visualise real Art-Net or sACN from ToskLight or another compatible desk, so the plan remains useful across the production."],
		],
		images: DETAIL_GALLERIES.architect,
		manual: "../manual/#page-20-tosklight-previz-index-md",
		sections: [
			["Visualization", "Preview the lighting in your venue.", "See fixture models, beams, gobos, and haze in the venue you have drawn. Choose a camera angle and rendering quality to inspect the look, or receive live Art-Net or sACN from a compatible desk.", "The lighting Visualizer runs separately from Control’s lightweight Stage view. You can close it while programming and reopen it when you want a rendered preview."],
			["Interchange", "Move your rig between planning tools.", "Import and export MVR to transfer the rig between applications. Import GDTF fixture files with their modes and physical information.", "Fixtures use Architect’s supplied 3D models or the images and models included in their fixture package."],
			["CAD", "Export venue and rig plans as PDFs.", "Draw the venue, place the rig, and prepare measured plans for the crew. Choose the views that show the information each department needs.", "Export a complete plan or a cutaway section as a PDF."],
			["Patch", "Prepare the patch alongside the plan.", "Set fixture types, modes, universes, and addresses in the Patch sheet while working on the rig in CAD.", "Transfer the rig to Control with its fixture data and patch, ready for programming."],
			["Moving scenes", "Move fixtures and scenery together.", "Place fixtures and scenery relative to a shared 3D point. Move that point through PSN or DMX and the attached objects follow, keeping their positions relative to one another.", "These position references are saved in the portable Show used by both Control and Architect. The screenshot shows them in Control’s Patch sheet."],
			["Lasers", "Preview laser patterns in the rig.", "A fixture’s JavaScript scan function turns DMX values into the laser path shown in the Visualizer.", "Write a scan function for the fixture to control the pattern’s shape, brightness, scan speed, and position."],
			["Early prototypes", "Check curtain drops, flames, and sparks.", "Preview falling curtains, flame jets, and sparklers in the rig. These simulations are early prototypes with basic visuals.", "Use them to check DMX triggers. They are not yet suitable for judging the final appearance of these effects."],
			["Media integration", "Preview Pixel video on your LED walls.", "Show live video from Pixel on the screens and LED panels in your 3D venue. See the video alongside the lighting as you work on the show.", "Choose a Pixel output, set the panels’ physical size and placement, and crop the video for each panel. In this example, three panels each show one third of the same picture."],
		],
		coming: [["Media server integration", "Being tested"], ["Paperwork rendering", "Mostly complete, with further improvements in progress."], ["Crowd simulation", "Being improved."]],
	},
];

for (const product of productPages) {
	const imageFiles = product.images.flat().filter(Boolean);
	if (imageFiles.length !== new Set(imageFiles).size || product.images.length !== product.sections.length) {
		throw new Error(`Detail page ${product.path} must map each section to a unique screenshot or no screenshot`);
	}
	for (const file of imageFiles) {
		if (!galleryByFile.has(file)) throw new Error(`Missing detail screenshot ${file}`);
	}
	const benefits = product.benefits
		.map(([title, copy]) => `<article><h2>${escapeHtml(title)}</h2><p>${escapeHtml(copy)}</p></article>`)
		.join("");
	const sections = `<section class="product-detail-story shell">${product.sections.map(([label, title, first, second], index) => {
		const sectionImages = product.images[index];
		const files = Array.isArray(sectionImages) ? sectionImages : sectionImages ? [sectionImages] : [];
		const figures = files.map((file) => {
			const entry = galleryByFile.get(file);
			const image = webImages.get(file).file;
			const url = `../screenshots/${escapeHtml(image)}`;
			const webImage = webImages.get(file);
			return `<figure class="shot"><a class="shot-image" href="${url}" aria-label="Open ${escapeHtml(entry.title)} at full size"><img ${responsiveScreenshot(webImage, "../screenshots/")} alt="${escapeHtml(entry.title)}" loading="lazy"></a><figcaption><strong>${escapeHtml(entry.title)}</strong> ${escapeHtml(entry.caption)}</figcaption></figure>`;
		});
		const leadingImage = product.path === "architect" && label === "Visualization" && files.length > 1;
		const galleryColumns = label === "Hardware" ? 1 : leadingImage ? 2 : files.length;
		const gallery = (items) => `<div class="product-detail-section-gallery" style="--gallery-columns:${galleryColumns}" aria-label="${escapeHtml(label)} screenshots">${items.join("")}</div>`;
		const figure = leadingImage ? figures[0] + gallery(figures.slice(1)) : files.length > 1 ? gallery(figures) : figures.join("");
		return `<article><div class="product-detail-copy"><p class="eyebrow">${escapeHtml(label)}</p><h2>${escapeHtml(title)}</h2></div><div class="product-detail-text"><p>${escapeHtml(first)}</p><p>${escapeHtml(second)}</p></div>${figure}</article>`;
	}).join("")}</section>`;
	const coming = product.coming ? `<section class="product-detail-coming shell"><p class="eyebrow">What is coming</p><h2>Still moving forward.</h2><div>${product.coming.map(([title, status]) => `<article><strong>${escapeHtml(title)}</strong><p>${escapeHtml(status)}</p></article>`).join("")}</div></section>` : "";
	mkdirSync(resolve(siteRoot, product.path), { recursive: true });
	writeFileSync(
		resolve(siteRoot, product.path, "index.html"),
		`<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><meta name="theme-color" content="#07090d"><title>${escapeHtml(product.name)}</title><link rel="icon" href="../icon.png" type="image/png"><link rel="stylesheet" href="../site.css"></head><body class="product-detail-page product-detail-${escapeHtml(product.path)}"><nav class="topbar shell" aria-label="Primary navigation"><a class="wordmark" href="../"><img src="../icon.png" alt=""><span>ToskLight</span></a><div class="nav-links"><a class="is-active" href="../#applications">Applications</a><a href="../performance/">Development</a><a href="../manual/">Manual</a><a class="nav-cta" href="../downloads/">Downloads</a></div></nav><main><header class="product-detail-hero shell"><p class="product-category">${escapeHtml(product.category)}</p><div class="product-detail-title-row"><h1>${escapeHtml(product.title)}</h1><img src="../product-icons/${escapeHtml(product.icon)}" alt="${escapeHtml(product.name)} icon"></div><p>${escapeHtml(product.intro)}</p><div class="actions"><a class="button button-primary" href="../downloads/">Download</a><a class="button" href="${escapeHtml(product.manual)}">Read the manual</a></div></header>${sections}${coming}</main><footer><div class="shell download-footer"><p><a href="../license/">ToskLight Community License</a> · <a href="../third-party-licenses.html">Third-party licenses</a> · <a href="../impressum/">Impressum</a> · <a href="../datenschutz/">Datenschutz</a></p><a href="../">Back to ToskLight</a></div></footer>${LIGHTBOX}<script src="../site.js"></script></body></html>`,
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
	`<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="theme-color" content="#07090d"><title>ToskLight Community License</title><link rel="icon" href="../icon.png" type="image/png"><link rel="stylesheet" href="../site.css"></head><body class="legal-page"><nav class="topbar shell"><a class="wordmark" href="../"><img src="../icon.png" alt=""><span>ToskLight</span></a><div class="nav-links"><a href="../downloads/">Downloads</a><a href="../performance/">Development</a><a class="nav-cta" href="../">Back to the suite</a></div></nav><main class="document-page shell"><header class="document-hero"><p class="eyebrow">Terms for using ToskLight</p><h1>Community License.</h1><p>Use ToskLight for your shows, study it, and build on it under the terms below.</p></header><article class="license-document"><pre>${escapeHtml(licenseText)}</pre></article></main><footer><div class="shell download-footer"><p><a href="../license.txt">Plain-text version</a> · <a href="../third-party-licenses.html">Third-party licenses</a> · <a href="../impressum/">Impressum</a> · <a href="../datenschutz/">Datenschutz</a></p><a href="../">← Back to ToskLight</a></div></footer></body></html>`,
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
// The hero is authored directly in the landing-page template rather than renderFigure.
page = page.replace(/<img\b(?=[^>]*\bsrc="screenshots\/([^" ]+\.png)")[^>]*>/gu, (image, file) => {
  const webImage = webImages.get(file);
  if (!webImage) throw new Error(`Unknown hero screenshot ${file}`);
  return image.replace(/src="[^"]+"/u, responsiveScreenshot(webImage));
});
writeFileSync(target, page);
const assetCount =
  PORTABLE_ASSETS.length + PLATFORMS.length;
console.log(
  `Stamped ${target} with version ${version}, ${GALLERY.length} screenshots, ` +
    `${assetCount} download links for ${tag}`,
);
