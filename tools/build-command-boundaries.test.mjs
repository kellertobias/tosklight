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

	assert.equal(packageManifest.scripts["test:unit"], "bash tools/test.sh unit");
	assert.equal(
		packageManifest.scripts["test:verify"],
		"bash tools/test.sh verify",
	);
	assert.match(testScript, /unit\(\)\{ typescript_unit; rust_unit; \}/u);
	assert.match(
		testScript,
		/verify\(\)\{[\s\S]*architecture[\s\S]*rust_workspace/u,
	);
	assert.match(workflow, /npm run test:verify/u);
});
