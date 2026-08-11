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
	const visualizer = workflow.indexOf(
		"- name: Build the standalone visualizer",
	);
	const sidecars = workflow.indexOf(
		"- name: Stage the Desk server and Stage renderer sidecars",
	);
	const desktop = workflow.indexOf(
		"- name: Build the ToskLight desktop application",
	);
	assert.ok(visualizer >= 0 && visualizer < sidecars);
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

test("macOS release apps are sealed only after their final helpers and resources", () => {
	const workflow = read(".github/workflows/release.yml");
	const mediaWorkflow = read(".github/workflows/media-release.yml");
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
	assert.match(workflow, /ToskLight Viz Editor\.app[\s\S]*seal-macos-app\.sh/u);
	assert.match(
		mediaWorkflow,
		/bundle-media-macos\.sh[\s\S]*seal-macos-app\.sh/u,
	);
	assert.match(assembler, /codesign --verify --deep --strict/u);
	assert.match(assembler, /macos-first-start\.txt/u);
	assert.match(assembler, /sign-macos-apps-locally\.sh/u);
});

test("the Viz release builds only the bundle format its staging step consumes", () => {
	const workflow = read(".github/workflows/release.yml");
	const frontendStart = workflow.indexOf(
		"- name: Build the ToskLight Viz Editor frontend",
	);
	const buildStart = workflow.indexOf(
		"- name: Build the ToskLight Viz Editor application",
	);
	const stageStart = workflow.indexOf(
		"- name: Stage the Viz release artifacts",
		buildStart,
	);
	assert.notEqual(frontendStart, -1, "the Viz frontend build should exist");
	assert.notEqual(buildStart, -1, "the Viz release build should exist");
	assert.ok(
		frontendStart < buildStart,
		"the Viz frontend should be built before Tauri packages it",
	);
	assert.match(
		workflow.slice(frontendStart, buildStart),
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
	assert.notEqual(
		openEnd,
		-1,
		"the command dispatcher should follow open_visualizer",
	);
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
		"tosklight-bundle-macos_arm64.zip",
		"tosklight-bundle-windows_amd64.zip",
		"tosklight-bundle-linux_amd64.zip",
		"tosklight-bundle-linux_arm64.zip",
		"assets-demo-show.show",
		"assets-handbook.pdf",
		"report-checksums.txt",
		"report-performance-status.json",
		"report-performance.zip",
	]) {
		assert.ok(
			workflow.includes(asset),
			`release workflow should require ${asset}`,
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
	assert.match(
		workflow,
		/media-build:[\s\S]*?uses: \.\/\.github\/workflows\/media-release\.yml/u,
	);
	assert.match(mediaWorkflow, /workflow_call:/u);
	assert.doesNotMatch(mediaWorkflow, /workflow_run|gh release upload/u);
	assert.match(
		mediaQualityWorkflow,
		/\.github\/workflows\/media-release\.yml/u,
	);

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
			mediaWorkflow.includes(`slug: ${slug}`),
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
			new RegExp(
				`\\n  ${job}:\\n[\\s\\S]*?\\n    if: github\\.ref == 'refs/heads/main'`,
			),
			`${job} should be main-only`,
		);
	}
	assert.match(
		workflow,
		/\n {2}media-build:\n[\s\S]*?\n {4}if: [^\n]*github\.ref == 'refs\/heads\/main'/u,
		"media-build should be main-only",
	);
	assert.match(
		workflow,
		/unit:[\s\S]*?run: npm run test:architecture[\s\S]*?run: npm run test:unit/u,
	);
	assert.match(
		workflow,
		/publish-marketing:[\s\S]*?github\.ref == 'refs\/heads\/main'/u,
	);
	assert.match(mediaRelease, /workflow_call:/u);
	assert.doesNotMatch(mediaRelease, /workflow_dispatch|workflow_run/u);
});
