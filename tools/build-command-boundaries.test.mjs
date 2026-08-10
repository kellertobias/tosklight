import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"..",
);
const read = (relativePath) =>
	fs.readFileSync(path.join(repositoryRoot, relativePath), "utf8");

function shellFunction(source, name, nextName) {
	const start = source.indexOf(`${name}() {`);
	const end = source.indexOf(`\n${nextName}() {`, start);
	assert.notEqual(start, -1, `${name} should exist`);
	assert.notEqual(end, -1, `${nextName} should follow ${name}`);
	return source.slice(start, end);
}

test("open reuses dependencies and builds only the main desktop application", () => {
	const buildScript = read("tools/build.sh");
	const openFunction = shellFunction(
		buildScript,
		"build_debug_and_open",
		"build_debug_app_bundle",
	);
	const appBuildFunction = shellFunction(
		buildScript,
		"build_debug_app_bundle",
		"archive_release_locked",
	);

	assert.match(openFunction, /ensure-workspace-dependencies\.mjs/u);
	assert.match(openFunction, /ensure-control-frontend\.mjs/u);
	assert.doesNotMatch(openFunction, /npm ci|build_icon_contact_sheets/u);
	assert.match(appBuildFunction, /light-headless/u);
	assert.match(appBuildFunction, /CONTROL_TAURI_CONFIG/u);
	assert.doesNotMatch(appBuildFunction, /HARDWARE_DIR|HARDWARE_TAURI_CONFIG/u);
});

test("repository Tauri overlays disable a duplicate frontend build", () => {
	const configWriter = read("tools/write-tauri-artifact-config.mjs");
	assert.match(configWriter, /beforeBuildCommand: ""/u);
});

test("the Viz release builds only the bundle format its staging step consumes", () => {
	const workflow = read(".github/workflows/release.yml");
	const buildStart = workflow.indexOf(
		"- name: Build the ToskLight Viz Editor application",
	);
	const stageStart = workflow.indexOf(
		"- name: Stage the Viz release artifacts",
		buildStart,
	);
	assert.notEqual(buildStart, -1, "the Viz release build should exist");
	assert.notEqual(
		stageStart,
		-1,
		"Viz artifact staging should follow its build",
	);
	const buildStep = workflow.slice(buildStart, stageStart);

	assert.match(buildStep, /bundle_args=\(--no-bundle\)/u);
	assert.match(buildStep, /macOS\) bundle_args=\(--bundles app\)/u);
	assert.match(buildStep, /Windows\) bundle_args=\(--bundles nsis\)/u);
	assert.doesNotMatch(buildStep, /--bundles all/u);
});

test("open:viz builds every helper path it exports", () => {
	const buildScript = read("tools/build.sh");
	const helperBuild = shellFunction(
		buildScript,
		"build_visualizer_headless",
		"open_visualizer",
	);
	const openStart = buildScript.indexOf("open_visualizer() {");
	const openEnd = buildScript.indexOf("\ncase ", openStart);
	assert.notEqual(openStart, -1, "open_visualizer should exist");
	assert.notEqual(openEnd, -1, "the command dispatcher should follow open_visualizer");
	const openVisualizer = buildScript.slice(openStart, openEnd);

	assert.match(helperBuild, /-p light-headless --bin light-headless/u);
	assert.match(openVisualizer, /build_visualizer_headless/u);
	assert.match(
		openVisualizer,
		/TOSKLIGHT_VIZ_HEADLESS="\$TARGET_DIR\/release\/light-headless"/u,
	);
});

test("fast unit tests and comprehensive verification remain distinct", () => {
	const packageManifest = JSON.parse(read("package.json"));
	const testScript = read("tools/test.sh");
	const workflow = read(".github/workflows/release.yml");
	const workspaceJob =
		/^ {2}workspace:\n([\s\S]*?)(?=^ {2}[\w-]+:\n)/mu.exec(workflow)?.[1] ?? "";

	assert.equal(packageManifest.scripts["test:unit"], "bash tools/test.sh unit");
	assert.equal(
		packageManifest.scripts["test:verify"],
		"bash tools/test.sh verify",
	);
	assert.match(testScript, /unit\(\)\{ typescript_unit; rust_unit; \}/u);
	assert.match(
		testScript,
		/rust_workspace\(\)\{[\s\S]*\(cd "\$UI" && npm run build\)[\s\S]*cargo test/u,
	);
	assert.match(workspaceJob, /actions\/setup-node/u);
	assert.match(workspaceJob, /run: npm ci/u);
	assert.match(
		testScript,
		/verify\(\)\{[\s\S]*architecture[\s\S]*rust_workspace/u,
	);
	assert.match(workflow, /bash tools\/test\.sh rust-workspace/u);
});

test("release packaging and Pages cover the supported product matrix", () => {
	const workflow = read(".github/workflows/release.yml");
	const mediaWorkflow = read(".github/workflows/media-release.yml");
	const mediaQualityWorkflow = read(".github/workflows/media.yml");
	const landingPage = read("tools/render-landing-page.mjs");

	for (const asset of [
		"tosklight-$VERSION-macos-arm64.zip",
		"light-headless-$VERSION-macos-arm64.zip",
		"tosklight-visualizer-$VERSION-macos-arm64.zip",
		"tosklight-$VERSION-windows-amd64-setup.exe",
		"light-headless-$VERSION-windows-amd64.zip",
		"tosklight-visualizer-$VERSION-windows-amd64.zip",
		"tosklight-$VERSION-linux-amd64.AppImage",
		"light-headless-$VERSION-linux-amd64.zip",
		"tosklight-visualizer-$VERSION-linux-amd64.zip",
		"light-headless-$VERSION-linux-arm64.zip",
	]) {
		assert.ok(workflow.includes(asset), `release workflow should require ${asset}`);
	}
	assert.match(
		workflow,
		/slug: linux-amd64[\s\S]*?desktop: true[\s\S]*?viz: true/u,
	);
	assert.match(workflow, /binary="[^"]*viz-renderer\$suffix"[\s\S]*--demo --verify/u);
	assert.match(mediaWorkflow, /echo "build=true" >> "\$GITHUB_OUTPUT"/u);
	assert.doesNotMatch(mediaWorkflow, /MEDIA_PATHS|git diff --quiet/u);
	assert.match(mediaQualityWorkflow, /\.github\/workflows\/media-release\.yml/u);

	for (const slug of ["macos-arm64", "windows-amd64", "linux-amd64", "linux-arm64"]) {
		assert.ok(
			mediaWorkflow.includes(`slug: ${slug}`),
			`Media release should build ${slug}`,
		);
		assert.ok(
			landingPage.includes(`tosklight-media-\${v}-${slug}.zip`),
			`Pages should link the Media Server for ${slug}`,
		);
	}
	for (const slug of ["macos-arm64", "windows-amd64", "linux-amd64"]) {
		assert.ok(
			landingPage.includes(`tosklight-visualizer-\${v}-${slug}.zip`),
			`Pages should link ToskLight PreViz for ${slug}`,
		);
	}
});

test("non-main branch pushes run validation without release work", () => {
	const workflow = read(".github/workflows/release.yml");
	const mediaRelease = read(".github/workflows/media-release.yml");

	assert.match(workflow, /push:[\s\S]*?branches: \["\*\*"\]/u);
	for (const job of [
		"metadata",
		"workspace",
		"native-extension-draft",
		"usb-dmx",
		"e2e-build",
	]) {
		assert.match(
			workflow,
			new RegExp(`\\n  ${job}:\\n[\\s\\S]*?\\n    if: github\\.ref == 'refs/heads/main'`),
			`${job} should be main-only`,
		);
	}
	assert.match(
		workflow,
		/unit:[\s\S]*?run: npm run test:architecture[\s\S]*?run: npm run test:unit/u,
	);
	assert.match(
		workflow,
		/publish-marketing:[\s\S]*?github\.ref == 'refs\/heads\/main'/u,
	);
	assert.match(
		mediaRelease,
		/workflow_dispatch' && github\.ref == 'refs\/heads\/main'/u,
	);
});
