import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
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

test("open launches the existing main desktop build without rebuilding", () => {
	const buildScript = read("tools/build.sh");
	const openFunction = shellFunction(
		buildScript,
		"open_debug_app",
		"build_debug_and_open",
	);

	assert.match(openFunction, /require_built_app/u);
	assert.match(openFunction, /require_built_file/u);
	assert.match(openFunction, /npm run build:open/u);
	assert.match(openFunction, /already answering/u);
	assert.doesNotMatch(
		openFunction,
		/cargo build|ensure-control-frontend|build_debug_app_bundle|stop_running/u,
	);
});

test("build:open preserves the focused main desktop rebuild", () => {
	const buildScript = read("tools/build.sh");
	const openFunction = shellFunction(
		buildScript,
		"build_debug_and_open",
		"open_hardware_controls",
	);
	const appBuildFunction = shellFunction(
		buildScript,
		"build_debug_app_bundle",
		"archive_release_locked",
	);

	assert.match(openFunction, /ensure-workspace-dependencies\.mjs/u);
	assert.match(openFunction, /ensure-control-frontend\.mjs/u);
	assert.match(openFunction, /stop_running/u);
	assert.match(openFunction, /npm run build:open/u);
	assert.doesNotMatch(openFunction, /npm ci|build_icon_contact_sheets/u);
	assert.match(appBuildFunction, /light-headless/u);
	assert.match(appBuildFunction, /CONTROL_TAURI_CONFIG/u);
	assert.doesNotMatch(appBuildFunction, /HARDWARE_DIR|HARDWARE_TAURI_CONFIG/u);
});

test("package scripts split launch-only and build-and-open commands", () => {
	const scripts = JSON.parse(read("package.json")).scripts;

	assert.equal(scripts.open, "bash tools/build.sh open");
	assert.equal(scripts["build:open"], "bash tools/build.sh build-open");
	for (const [app, directName] of [
		["hardware-controls", "hardware-controls"],
		["media", "media"],
		["viz", "viz"],
		["viz-editor", "viz-editor"],
	]) {
		assert.equal(
			scripts[`open:${app}`],
			`bash tools/build.sh open-${directName}`,
		);
		assert.equal(
			scripts[`build:${app}:open`],
			`bash tools/build.sh build-${directName}-open`,
		);
	}
});

test("secondary app open functions only consume existing artifacts", () => {
	const buildScript = read("tools/build.sh");
	for (const [openName, nextName, buildCommand] of [
		[
			"open_hardware_controls",
			"build_hardware_controls_and_open",
			"npm run build:hardware-controls:open",
		],
		[
			"open_viz_editor",
			"build_viz_editor_and_open",
			"npm run build:viz-editor:open",
		],
		["open_media", "build_media_and_open", "npm run build:media:open"],
	]) {
		const openFunction = shellFunction(buildScript, openName, nextName);
		assert.match(openFunction, /require_built_(?:file|app)/u);
		assert.ok(openFunction.includes(buildCommand));
		assert.doesNotMatch(openFunction, /cargo build|npm run tauri:build/u);
	}
});

test("repository Tauri overlays disable a duplicate frontend build", () => {
	const configWriter = read("tools/write-tauri-artifact-config.mjs");
	assert.match(configWriter, /beforeBuildCommand: ""/u);
	assert.match(configWriter, /externalBin/u);
	assert.match(configWriter, /light-headless/u);
	assert.match(configWriter, /viz-renderer/u);
});

test("the control Tauri overlay resolves both target-specific sidecars", () => {
	const temporaryParent = path.join(repositoryRoot, ".artifacts", "tmp");
	fs.mkdirSync(temporaryParent, { recursive: true });
	const temporary = fs.mkdtempSync(
		path.join(temporaryParent, "tauri-sidecar-contract."),
	);
	try {
		const target = "aarch64-apple-darwin";
		for (const binary of ["light-headless", "viz-renderer"]) {
			fs.writeFileSync(path.join(temporary, `${binary}-${target}`), binary);
		}
		const output = path.join(temporary, "control.json");
		execFileSync(
			process.execPath,
			[
				path.join(repositoryRoot, "tools/write-tauri-artifact-config.mjs"),
				"control",
				output,
			],
			{
				env: {
					...process.env,
					LIGHT_DESKTOP_SIDECAR_DIR: temporary,
					LIGHT_DESKTOP_SIDECAR_TARGET: target,
				},
			},
		);
		const config = JSON.parse(fs.readFileSync(output, "utf8"));
		assert.deepEqual(config.bundle.externalBin, [
			path.join(temporary, "light-headless"),
			path.join(temporary, "viz-renderer"),
		]);
	} finally {
		fs.rmSync(temporary, { recursive: true, force: true });
	}
});

test("release Desk bundles both supervised helpers before packaging", () => {
	const workflow = read(".github/workflows/release.yml");
	const sidecarBuild = workflow.indexOf(
		"- name: Build the Desk server, Stage renderer, and Media Server",
	);
	const sidecars = workflow.indexOf(
		"- name: Stage the Desk server and Stage renderer sidecars",
	);
	const desktop = workflow.indexOf(
		"- name: Build the ToskLight desktop application",
	);
	assert.ok(sidecarBuild >= 0 && sidecarBuild < sidecars);
	assert.match(
		workflow.slice(sidecarBuild, sidecars),
		/-p light-headless --bin light-headless[\s\S]*-p viz-renderer --bin viz-renderer/u,
	);
	assert.ok(sidecars < desktop);
	assert.match(workflow.slice(sidecars, desktop), /light-headless/u);
	assert.match(workflow.slice(sidecars, desktop), /viz-renderer/u);
	const smoke = read("tools/ci-smoke-built-desktop.mjs");
	assert.match(smoke, /light-headless/u);
	assert.match(smoke, /viz-renderer/u);
	assert.match(smoke, /api\/v2\/readiness/u);
	assert.match(smoke, /bundle\/appimage/u);
	assert.match(smoke, /APPIMAGE_EXTRACT_AND_RUN/u);
	assert.match(smoke, /Bundled Light server log/u);
});

test("release launch-smokes the Viz Editor and Media Server", () => {
	const workflow = read(".github/workflows/release.yml");
	const mediaSmoke = read("tools/ci-smoke-media-server.mjs");
	assert.match(workflow, /Smoke-test the built Media Server startup/u);
	assert.match(workflow, /node tools\/ci-smoke-media-server\.mjs/u);
	assert.match(mediaSmoke, /api\/v2\/health/u);
	assert.match(mediaSmoke, /health\.outputs > 0/u);
	assert.match(mediaSmoke, /interfaceResponse\.ok/u);
	assert.match(workflow, /Smoke-test the ToskLight Viz Editor application/u);
	assert.match(workflow, /viz-editor\$suffix/u);
	assert.match(workflow, /xvfb-run -a "\$binary" --verify/u);
});

test("native Timecode audio stays out of the ARM headless build", () => {
	const workflow = read(".github/workflows/release.yml");
	const appManifest = read("apps/light-headless/Cargo.toml");
	const runtimeManifest = read("crates/light/adapters/headless/Cargo.toml");
	const armStart = workflow.indexOf("label: Linux (ARM64 / Raspberry Pi)");
	const windowsStart = workflow.indexOf("label: Windows (x86_64)", armStart);

	assert.ok(armStart >= 0 && windowsStart > armStart);
	assert.match(
		workflow.slice(armStart, windowsStart),
		/--no-default-features/u,
	);
	assert.match(
		workflow,
		/Install Linux ARM64 server dependencies[\s\S]*?libudev-dev/u,
	);
	assert.match(appManifest, /default = \["native-audio-output"\]/u);
	assert.match(
		appManifest,
		/native-audio-output = \["light-headless-runtime\/native-audio-output"\]/u,
	);
	assert.match(runtimeManifest, /native-audio-output = \["dep:cpal"\]/u);
	assert.match(runtimeManifest, /cpal = \{[^\n]*optional = true[^\n]*\}/u);
});

test("macOS release apps are sealed only after their final helpers and resources", () => {
	const workflow = read(".github/workflows/release.yml");
	const assembler = read("tools/assemble-release-bundle.sh");
	const sealer = read("tools/seal-macos-app.sh");
	const desktopBuild = workflow.indexOf(
		"- name: Build the ToskLight desktop application",
	);
	const desktopSeal = workflow.indexOf(
		"- name: Seal the completed macOS Desk bundle",
	);
	const desktopSmoke = workflow.indexOf(
		"- name: Smoke-test the built desktop application",
	);

	assert.ok(desktopBuild >= 0 && desktopBuild < desktopSeal);
	assert.ok(desktopSeal < desktopSmoke);
	assert.match(sealer, /codesign --force --deep --sign - --timestamp=none/u);
	assert.match(sealer, /codesign --verify --deep --strict/u);
	assert.match(workflow, /ToskLight Visualizer\.app[\s\S]*seal-macos-app\.sh/u);
	assert.match(
		workflow,
		/editor_app="\$bundle\/macos\/ToskLight Visualizer\.app"[\s\S]*accessory\/viz-editor[\s\S]*ToskLight Visualizer\.app\/Contents\/MacOS\/viz-editor[\s\S]*seal-macos-app\.sh/u,
	);
	assert.match(
		workflow,
		/bundle-media-macos\.sh[\s\S]*seal-macos-app\.sh/u,
	);
	assert.match(assembler, /codesign --verify --deep --strict/u);
	assert.match(assembler, /macos-first-start\.txt/u);
	assert.match(assembler, /sign-macos-apps-locally\.sh/u);
});

test("the Viz release builds only the bundle format its staging step consumes", () => {
	const workflow = read(".github/workflows/release.yml");
	const frontends =
		/^ {2}frontends:\n([\s\S]*?)(?=^ {2}[\w-]+:\n)/mu.exec(workflow)?.[1] ?? "";
	const buildStart = workflow.indexOf(
		"- name: Build the ToskLight Viz Editor application",
	);
	const stageStart = workflow.indexOf(
		"- name: Stage the Viz release artifacts",
		buildStart,
	);
	assert.notEqual(buildStart, -1, "the Viz release build should exist");
	assert.match(frontends, /npm run --prefix apps\/viz-editor build/u);
	assert.match(frontends, /name: shared-frontends-\$\{\{ github\.sha \}\}/u);
	assert.doesNotMatch(
		workflow.slice(buildStart, stageStart),
		/npm run --prefix apps\/viz-editor build/u,
	);
	assert.notEqual(
		stageStart,
		-1,
		"Viz artifact staging should follow its build",
	);
	const buildStep = workflow.slice(buildStart, stageStart);

	assert.match(buildStep, /bundle_args=\(--no-bundle\)/u);
	assert.match(buildStep, /macOS\) bundle_args=\(--bundles app\)/u);
	assert.doesNotMatch(buildStep, /Windows\) bundle_args=\(--bundles nsis\)/u);
	assert.doesNotMatch(buildStep, /--bundles all/u);
});

test("the Viz release keeps one product identity and literal accessory names", () => {
	const workflow = read(".github/workflows/release.yml");
	const assembler = read("tools/assemble-release-bundle.sh");
	const config = read("apps/viz-editor/src-tauri/tauri.conf.json");

	assert.match(config, /"productName": "ToskLight Visualizer"/u);
	assert.match(config, /"title": "Rig Editor"/u);
	assert.doesNotMatch(workflow, /tosklight-viz-\$version-windows-amd64-setup/u);
	assert.match(assembler, /ToskLight Visualizer\.exe/u);
	assert.doesNotMatch(assembler, /mv "\$previz\/viz-editor(?:\.exe)?"/u);
});

test("open:viz requires every helper path it exports without rebuilding", () => {
	const buildScript = read("tools/build.sh");
	const helperBuild = shellFunction(
		buildScript,
		"build_visualizer_headless",
		"open_visualizer",
	);
	const openStart = buildScript.indexOf("open_visualizer() {");
	const openEnd = buildScript.indexOf(
		"\nbuild_visualizer_and_open() {",
		openStart,
	);
	assert.notEqual(openStart, -1, "open_visualizer should exist");
	assert.notEqual(
		openEnd,
		-1,
		"build_visualizer_and_open should follow open_visualizer",
	);
	const openVisualizer = buildScript.slice(openStart, openEnd);

	assert.match(helperBuild, /-p light-headless --bin light-headless/u);
	assert.match(openVisualizer, /require_built_file/u);
	assert.match(openVisualizer, /npm run build:viz:open/u);
	assert.doesNotMatch(
		openVisualizer,
		/build_visualizer\n|build_viz_editor\n|build_visualizer_headless\n/u,
	);
	assert.match(
		openVisualizer,
		/TOSKLIGHT_VIZ_HEADLESS="\$TARGET_DIR\/release\/light-headless"/u,
	);
});

test("build:viz:open builds every helper before using launch-only open", () => {
	const buildScript = read("tools/build.sh");
	const start = buildScript.indexOf("build_visualizer_and_open() {");
	const end = buildScript.indexOf("\ncase ", start);
	assert.notEqual(start, -1);
	assert.notEqual(end, -1);
	const buildAndOpen = buildScript.slice(start, end);

	assert.match(buildAndOpen, /build_visualizer/u);
	assert.match(buildAndOpen, /build_viz_editor/u);
	assert.match(buildAndOpen, /build_visualizer_headless/u);
	assert.match(buildAndOpen, /open_visualizer/u);
});

test("fast unit tests and comprehensive verification remain distinct", () => {
	const packageManifest = JSON.parse(read("package.json"));
	const testScript = read("tools/test.sh");
	const workflow = read(".github/workflows/release.yml");
	const workspaceJob =
		/^ {2}workspace:\n([\s\S]*?)(?=^ {2}[\w-]+:\n)/mu.exec(workflow)?.[1] ?? "";

	assert.equal(packageManifest.scripts["test:unit"], "bash tools/test.sh unit");
	assert.equal(
		packageManifest.scripts["test:typescript-unit"],
		"bash tools/test.sh typescript-unit",
	);
	assert.equal(
		packageManifest.scripts["test:verify"],
		"bash tools/test.sh verify",
	);
	assert.match(testScript, /unit\(\)\{ typescript_unit; rust_unit; \}/u);
	assert.match(
		testScript,
		/rust_workspace\(\)\{[\s\S]*LIGHT_REUSE_FRONTEND_BUILD[\s\S]*\(cd "\$UI" && npm run build\)[\s\S]*cargo test/u,
	);
	assert.match(workspaceJob, /actions\/setup-node/u);
	assert.match(workspaceJob, /run: npm ci/u);
	assert.match(
		testScript,
		/verify\(\)\{[\s\S]*architecture[\s\S]*rust_workspace/u,
	);
	assert.match(workflow, /bash tools\/test\.sh rust-workspace/u);
	assert.match(
		workflow,
		/workspace:[\s\S]*?needs: frontends[\s\S]*?LIGHT_REUSE_FRONTEND_BUILD: "1"[\s\S]*?name: shared-frontends-\$\{\{ github\.sha \}\}/u,
	);
	assert.match(
		workflow,
		/e2e-build:[\s\S]*?needs: frontends[\s\S]*?LIGHT_REUSE_FRONTEND_BUILD: "1"[\s\S]*?name: shared-frontends-\$\{\{ github\.sha \}\}/u,
	);
});

test("CI keeps shared inputs out of release downloads and strips debug symbols", () => {
	const workflow = read(".github/workflows/release.yml");
	const documentation = read(".github/workflows/documentation.yml");

	for (const source of [workflow, documentation]) {
		assert.match(source, /CARGO_PROFILE_DEV_DEBUG: "0"/u);
		assert.match(source, /CARGO_PROFILE_TEST_DEBUG: "0"/u);
		assert.doesNotMatch(source, /name: release-frontends-/u);
	}
	assert.match(workflow, /name: shared-frontends-\$\{\{ github\.sha \}\}/u);
	assert.match(
		workflow,
		/find \. -maxdepth 1 -type f ! -name report-checksums\.txt -print0[\s\S]*xargs -0 sha256sum/u,
	);
	assert.doesNotMatch(workflow, /sha256sum \* > report-checksums\.txt/u);
	assert.match(workflow, /group: github-ci-release-\$\{\{ github\.ref \}\}/u);
});

test("release packaging and Pages cover the supported product matrix", () => {
	const workflow = read(".github/workflows/release.yml");
	const publicationWorkflow = read(".github/workflows/documentation.yml");
	const landingPage = read("tools/render-landing-page.mjs");

	for (const asset of [
		"tosklight-bundle-macos_arm64.zip",
		"tosklight-bundle-windows_amd64.zip",
		"tosklight-bundle-linux_amd64.zip",
		"tosklight-bundle-linux_arm64.zip",
		"assets-demo-show.show",
		"report-release-metadata.json",
		"report-checksums.txt",
	]) {
		assert.ok(
			workflow.includes(asset),
			`release workflow should require ${asset}`,
		);
	}
	for (const asset of [
		"assets-handbook.pdf",
		"report-performance-status.json",
		"report-performance.zip",
		"report-documentation.json",
	]) {
		assert.ok(
			publicationWorkflow.includes(asset),
			`scheduled publication should generate ${asset}`,
		);
	}
	assert.match(
		workflow,
		/slug: linux-amd64[\s\S]*?desktop: true[\s\S]*?viz: true/u,
	);
	assert.match(
		workflow,
		/binary="[^"]*viz-renderer\$suffix"[\s\S]*--demo --verify/u,
	);
	assert.match(workflow, /-p media-server --bin media-server/u);
	assert.doesNotMatch(workflow, /default-demo-show --dir assets/u);
	assert.match(
		workflow,
		/default-demo-show-\$attempt[\s\S]*?demo_show="\$attempt_dir\/demo\.show"/u,
	);
	assert.match(
		workflow,
		/Assemble the operator-facing platform bundle[\s\S]*assemble-release-bundle\.sh/u,
	);
	assert.equal(
		fs.existsSync(path.join(repositoryRoot, ".github/workflows/media-release.yml")),
		false,
	);
	assert.equal(fs.existsSync(path.join(repositoryRoot, ".github/workflows/media.yml")), false);

	for (const slug of [
		"macos_arm64",
		"windows_amd64",
		"linux_amd64",
		"linux_arm64",
	]) {
		assert.ok(
			landingPage.includes(`tosklight-bundle-${slug}.zip`),
			`Pages should link the platform bundle for ${slug}`,
		);
	}
	for (const slug of [
		"macos-arm64",
		"windows-amd64",
		"linux-amd64",
		"linux-arm64",
	]) {
		assert.ok(
			workflow.includes(`slug: ${slug}`),
			`Media should build ${slug}`,
		);
	}
	assert.match(landingPage, /assets-demo-show\.show/u);
	assert.match(landingPage, /assets-handbook\.pdf/u);
	assert.match(workflow, /name: Release \/ Generate default demo show/u);
	assert.match(workflow, /npm run test:demo-show/u);
	assert.match(
		workflow,
		/Generate and validate the completed portable demo show through the API/u,
	);
	assert.doesNotMatch(workflow, /run: npm run demo-show(?:\s|$)/u);
	assert.match(
		workflow,
		/name: playwright-application-\$\{\{ github\.sha \}\}/u,
	);
	assert.doesNotMatch(
		workflow,
		/npm run test:demo(?!-show)|name: product-demo|Documentation \/ Product demo/u,
	);
});

test("non-main branch pushes run validation without release work", () => {
	const workflow = read(".github/workflows/release.yml");

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
			new RegExp(
				`\\n  ${job}:\\n[\\s\\S]*?\\n    if: github\\.ref == 'refs/heads/main'`,
			),
			`${job} should be main-only`,
		);
	}
	assert.match(
		workflow,
		/\n {2}build:\n[\s\S]*?\n {4}if: github\.event_name == 'push' && github\.ref == 'refs\/heads\/main'/u,
		"platform bundles should be main-push-only",
	);
	assert.match(
		workflow,
		/unit:[\s\S]*?run: npm run test:architecture[\s\S]*?github\.ref == 'refs\/heads\/main'[\s\S]*?run: npm run test:typescript-unit[\s\S]*?github\.ref != 'refs\/heads\/main'[\s\S]*?run: npm run test:unit/u,
	);
	assert.match(
		read(".github/workflows/documentation.yml"),
		/schedule:[\s\S]*?cron:[\s\S]*?publish-marketing:/u,
	);
	assert.match(
		workflow,
		/build:[\s\S]*?needs: \[metadata, frontends\][\s\S]*?fail-fast: true/u,
	);
	assert.match(
		workflow,
		/release:[\s\S]*?needs:[\s\S]*?- workspace[\s\S]*?- native-extension-draft[\s\S]*?- usb-dmx[\s\S]*?- e2e[\s\S]*?- build/u,
	);
});
