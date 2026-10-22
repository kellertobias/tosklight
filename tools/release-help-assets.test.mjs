import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import test from "node:test";
import vm from "node:vm";
import { artifactPaths } from "./artifact-paths.mjs";

const { load } = createRequire(import.meta.url)("js-yaml");
const workflow = load(
	fs.readFileSync(
		new URL("../.github/workflows/release.yml", import.meta.url),
		"utf8",
	),
);
const producer = workflow.jobs["help-assets"];
const documentation = workflow.jobs.documentation;
const build = workflow.jobs.build;
const byName = (job, name) => job.steps.find((step) => step.name === name);
const restoreName = "Verify and install generated Help before compilation";
function inlineNode(step) {
	const match = step.run.match(
		/node --input-type=module <<'NODE'\n([\s\S]*?)\nNODE/u,
	);
	assert.ok(
		match,
		"the actual workflow must carry its executable transfer script",
	);
	return match[1];
}
const stageSource = inlineNode(
	byName(producer, "Bind generated Help to its source and image checksums"),
);
const restoreSource = inlineNode(byName(build, restoreName));
const artifactName = "release-help-gallery-${{ github.sha }}";

test("release Help production stays bounded and reuses the exact E2E binary", () => {
	assert.equal(producer.needs, "e2e-build");
	assert.equal(producer.env.LIGHT_REUSE_E2E_BUILD, "1");
	const download = producer.steps.find((step) =>
		step.uses?.startsWith("actions/download-artifact@"),
	);
	assert.equal(download.with.name, "playwright-application-${{ github.sha }}");
	const capture = byName(
		producer,
		"Capture every Help image from this source revision",
	).run;
	assert.ok(
		capture.indexOf("rm -rf docs/help/assets/screenshots") <
			capture.indexOf("bash tools/test.sh help-screenshots\n"),
	);
	assert.match(
		capture,
		/bash tools\/test\.sh help-screenshots\nbash tools\/test\.sh help-screenshots-live\nnode tools\/check-help-screenshot-set\.mjs/u,
	);
	assert.ok(
		!producer.steps.some((step) =>
			step.run?.includes("bash tools/test.sh storybook"),
		),
		"full story contracts must not delay platform compilation",
	);
	const native = byName(producer, "Restore reviewed native screenshot inputs");
	assert.ok(
		producer.steps.indexOf(native) <
			producer.steps.indexOf(
				byName(producer, "Capture every Help image from this source revision"),
			),
	);
	for (const name of [
		"HELP_NATIVE_CAPTURE_RELEASE",
		"HELP_NATIVE_CAPTURE_ASSET",
		"HELP_NATIVE_CAPTURE_SHA256",
	]) {
		assert.equal(native.env[name], "${{ vars." + name + " }}");
		assert.ok(
			native.run.includes("${" + name + ":?"),
			"missing review pins must fail visibly",
		);
	}
	assert.match(
		native.run,
		/sha256sum --check[\s\S]*manual-captures\.py restore-native/u,
	);
	const upload = producer.steps.find((step) =>
		step.uses?.startsWith("actions/upload-artifact@"),
	);
	assert.equal(upload.with.name, artifactName);
	assert.equal(upload.with["if-no-files-found"], "error");
});

test("platform compilation and manual rendering consume the same validated Help artifact", () => {
	assert.ok(build.needs.includes("help-assets"));
	assert.ok(
		!build.needs.includes("documentation"),
		"the complete Storybook suite is not a native build input",
	);
	const firstCompile = build.steps.findIndex((step) =>
		step.run?.includes("cargo build"),
	);
	const install = build.steps.findIndex((step) => step.name === restoreName);
	assert.ok(
		install >= 0 && install < firstCompile,
		"hydrate Help before the early release performance probe too",
	);
	for (const job of [documentation, build]) {
		const download = byName(job, "Download exact-commit Help gallery");
		assert.equal(download.with.name, artifactName);
		assert.equal(inlineNode(byName(job, restoreName)), restoreSource);
	}
	const contract = documentation.steps.findIndex(
		(step) =>
			step.name === "Render every Storybook story and screenshot candidate",
	);
	const restore = documentation.steps.findIndex(
		(step) => step.name === restoreName,
	);
	const manual = documentation.steps.findIndex(
		(step) => step.name === "Build and verify the operator manual",
	);
	assert.ok(
		contract < restore && restore < manual,
		"Storybook candidates must not overwrite the complete fresh gallery before the manual",
	);
	const buildScript = fs.readFileSync(
		new URL("../crates/light/adapters/headless/build.rs", import.meta.url),
		"utf8",
	);
	assert.match(
		buildScript,
		/cargo:rerun-if-changed=.*docs\/help/u,
		"restored captures must invalidate cached embedded Help",
	);
});

test("PRs and feature branches retain documentation checks when release captures are skipped", () => {
	for (const [frontends, gallery, expected] of [
		["success", "success", true],
		["success", "skipped", true],
		["success", "failure", false],
		["failure", "success", false],
	]) {
		assert.equal(
			vm.runInNewContext(
				documentation.if.replaceAll(
					"needs.help-assets",
					'needs["help-assets"]',
				),
				{
					always: () => true,
					cancelled: () => false,
					needs: {
						metadata: { result: "skipped" },
						frontends: { result: frontends },
						"help-assets": { result: gallery },
					},
				},
			),
			expected,
		);
	}
	assert.deepEqual(documentation.needs, [
		"metadata",
		"frontends",
		"help-assets",
	]);
	for (const name of [
		"Resolve generated Help gallery path",
		"Download exact-commit Help gallery",
		restoreName,
	]) {
		assert.equal(
			byName(documentation, name).if,
			"needs.help-assets.result == 'success'",
		);
	}
});

const commit = "a".repeat(40);
const png = Buffer.from(
	"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jR1cAAAAASUVORK5CYII=",
	"base64",
);
function captureFixture() {
	fs.mkdirSync(artifactPaths.tmp, { recursive: true });
	const root = fs.mkdtempSync(
		path.join(artifactPaths.tmp, "release-help-assets-test-"),
	);
	const source = path.join(root, "docs/help/assets/screenshots");
	const stage = path.join(root, ".artifacts/tmp/release-help-gallery");
	fs.mkdirSync(path.join(source, "panes"), { recursive: true });
	for (const file of ["overview.png", "panes/tile.png"])
		fs.writeFileSync(path.join(source, file), png);
	fs.writeFileSync(
		path.join(root, "docs/help/screenshot-manifest.json"),
		JSON.stringify({
			version: 1,
			entries: [{ file: "overview.png" }, { file: "panes/tile.png" }],
		}),
	);
	return { root, source, stage };
}
function execute(source, fixture, overrides = {}) {
	return spawnSync(process.execPath, ["--input-type=module", "-e", source], {
		cwd: fixture.root,
		encoding: "utf8",
		env: {
			...process.env,
			GITHUB_SHA: commit,
			RELEASE_HELP_GALLERY_DIR: fixture.stage,
			...overrides,
		},
	});
}
function stagedFixture() {
	const fixture = captureFixture();
	const result = execute(stageSource, fixture);
	assert.equal(result.status, 0, result.stderr);
	fs.rmSync(fixture.source, { recursive: true });
	fs.mkdirSync(fixture.source, { recursive: true });
	fs.writeFileSync(
		path.join(fixture.source, "old-tracked.png"),
		"stale legacy file",
	);
	return fixture;
}

test("the workflow transfer installs every declared PNG and replaces stale legacy assets", () => {
	const fixture = stagedFixture();
	try {
		fs.writeFileSync(
			path.join(fixture.stage, "screenshots/unverified.png"),
			png,
		);
		const result = execute(restoreSource, fixture);
		assert.equal(result.status, 0, result.stderr);
		assert.ok(!fs.existsSync(path.join(fixture.source, "unverified.png")));
		assert.deepEqual(
			fs.readFileSync(path.join(fixture.source, "panes/tile.png")),
			png,
		);
		assert.deepEqual(
			fs.readFileSync(path.join(fixture.source, "overview.png")),
			png,
		);
		assert.ok(!fs.existsSync(path.join(fixture.source, "old-tracked.png")));
		const metadata = JSON.parse(
			fs.readFileSync(path.join(fixture.stage, "source.json")),
		);
		assert.equal(metadata.sourceCommit, commit);
		assert.deepEqual(Object.keys(metadata.files).sort(), [
			"overview.png",
			"panes/tile.png",
		]);
	} finally {
		fs.rmSync(fixture.root, { recursive: true, force: true });
	}
});

test("the workflow binds manifest identity across LF and Windows CRLF checkouts", () => {
	for (const [producerEnding, consumerEnding, artifactEnding] of [
		["\n", "\r\n", "\n"],
		["\r\n", "\n", "\n"],
		["\n", "\r\n", "\r\n"],
	]) {
		const fixture = captureFixture();
		try {
			const manifestPath = path.join(
				fixture.root,
				"docs/help/screenshot-manifest.json",
			);
			const canonical =
				JSON.stringify(
					JSON.parse(fs.readFileSync(manifestPath, "utf8")),
					null,
					2,
				) + "\n";
			fs.writeFileSync(
				manifestPath,
				canonical.replaceAll("\n", producerEnding),
			);
			const staged = execute(stageSource, fixture);
			assert.equal(staged.status, 0, staged.stderr);
			const stagedManifest = path.join(
				fixture.stage,
				"screenshot-manifest.json",
			);
			assert.equal(
				fs.readFileSync(stagedManifest, "utf8"),
				canonical,
				"the producer always hashes and uploads canonical LF bytes",
			);
			fs.writeFileSync(
				manifestPath,
				canonical.replaceAll("\n", consumerEnding),
			);
			fs.writeFileSync(
				stagedManifest,
				canonical.replaceAll("\n", artifactEnding),
			);
			fs.rmSync(fixture.source, { recursive: true });
			const restored = execute(restoreSource, fixture);
			assert.equal(restored.status, 0, restored.stderr);
			assert.deepEqual(
				fs.readFileSync(path.join(fixture.source, "overview.png")),
				png,
			);
			assert.deepEqual(
				fs.readFileSync(path.join(fixture.source, "panes/tile.png")),
				png,
			);
		} finally {
			fs.rmSync(fixture.root, { recursive: true, force: true });
		}
	}
});

test("the workflow refuses foreign, incomplete, and corrupt Help before changing embedded inputs", () => {
	const mutations = [
		(fixture) => {
			const file = path.join(fixture.stage, "source.json");
			const source = JSON.parse(fs.readFileSync(file));
			source.sourceCommit = "b".repeat(40);
			fs.writeFileSync(file, JSON.stringify(source));
		},
		(fixture) =>
			fs.appendFileSync(
				path.join(fixture.root, "docs/help/screenshot-manifest.json"),
				"\n",
			),
		(fixture) =>
			fs.rmSync(path.join(fixture.stage, "screenshots/panes/tile.png")),
		(fixture) =>
			fs.appendFileSync(
				path.join(fixture.stage, "screenshots/overview.png"),
				"corrupt",
			),
	];
	for (const mutate of mutations) {
		const fixture = stagedFixture();
		try {
			mutate(fixture);
			const result = execute(restoreSource, fixture);
			assert.notEqual(result.status, 0);
			assert.equal(
				fs.readFileSync(path.join(fixture.source, "old-tracked.png"), "utf8"),
				"stale legacy file",
			);
		} finally {
			fs.rmSync(fixture.root, { recursive: true, force: true });
		}
	}
});

test("documentation restores reviewed native inputs before every Storybook capture", () => {
	const native = byName(
		documentation,
		"Restore reviewed native screenshot inputs",
	);
	assert.ok(native, "a fresh checkout must hydrate untracked native stills");
	const render = byName(
		documentation,
		"Render every Storybook story and screenshot candidate",
	);
	assert.ok(
		documentation.steps.indexOf(native) < documentation.steps.indexOf(render),
	);
	assert.equal(
		native.if,
		undefined,
		"feature and PR checks also need native screenshot inputs",
	);
	assert.equal(native.env.GH_TOKEN, "${{ github.token }}");
	for (const name of [
		"HELP_NATIVE_CAPTURE_RELEASE",
		"HELP_NATIVE_CAPTURE_ASSET",
		"HELP_NATIVE_CAPTURE_SHA256",
	]) {
		assert.equal(native.env[name], "${{ vars." + name + " }}");
		assert.ok(
			native.run.includes("${" + name + ":?"),
			"missing review pins must fail visibly",
		);
	}
	assert.equal(
		native.run,
		byName(producer, "Restore reviewed native screenshot inputs").run,
	);
	assert.match(native.run, /gh release download/);
	assert.match(
		native.run,
		/sha256sum --check[\s\S]*manual-captures\.py restore-native/u,
	);
	assert.doesNotMatch(JSON.stringify(native), /AWS_|secrets\./u);
	const syntax = spawnSync("bash", ["-n"], {
		input: native.run,
		encoding: "utf8",
	});
	assert.equal(syntax.status, 0, syntax.stderr);
});

test("the one-off rebuild selects exactly 1.0.0 before inspecting historical tags", () => {
	const metadata = workflow.jobs.metadata;
	const select = byName(metadata, "Select the build and release version");
	assert.equal(select.env.REBUILD_VERSION, "${{ inputs.rebuild_version }}");
	assert.match(select.run, /node tools\/release-epoch\.cjs build-version/u);
	assert.doesNotMatch(select.run, /git tag --list/u);
	fs.mkdirSync(artifactPaths.tmp, { recursive: true });
	const scratch = fs.mkdtempSync(
		path.join(artifactPaths.tmp, "one-off-release-version-"),
	);
	try {
		const bin = path.join(scratch, "bin");
		fs.mkdirSync(bin);
		// Simulate the still-existing 2.x tags. An explicit rebuild must not calculate from them.
		fs.writeFileSync(
			path.join(bin, "git"),
			'#!/bin/bash\nprintf "v2.6.2\\n"\n',
		);
		fs.writeFileSync(
			path.join(bin, "npm"),
			'#!/bin/bash\nprintf "version=1.0.0\\nrelease=true\\n" >> "$LIGHT_RELEASE_OUTPUT"\n',
		);
		fs.writeFileSync(path.join(bin, "node"), '#!/bin/bash\nprintf "1.0.0\\n"\n');
		for (const name of ["git", "npm", "node"])
			fs.chmodSync(path.join(bin, name), 0o755);
		const run = (event, ref, version) => {
			const output = path.join(scratch, "outputs");
			fs.writeFileSync(output, "");
			const result = spawnSync("bash", ["-c", select.run], {
				encoding: "utf8",
				env: {
					...process.env,
					PATH: `${bin}:${process.env.PATH}`,
					EVENT_NAME: event,
					REF_NAME: ref,
					REBUILD_VERSION: version,
					GITHUB_OUTPUT: output,
				},
			});
			return { ...result, outputs: fs.readFileSync(output, "utf8") };
		};
		const rebuild = run("workflow_dispatch", "refs/heads/main", "1.0.0");
		assert.equal(rebuild.status, 0, rebuild.stderr);
		assert.equal(rebuild.outputs, "version=1.0.0\nrelease=false\n");
		for (const [event, ref, version] of [
			["workflow_dispatch", "refs/heads/main", "2.0.0"],
			["workflow_dispatch", "refs/heads/main", "1.0.0; echo unsafe"],
			["workflow_dispatch", "refs/heads/feature", "1.0.0"],
			["push", "refs/heads/main", "1.0.0"],
		]) {
			const invalid = run(event, ref, version);
			assert.notEqual(invalid.status, 0, `${event} ${ref} ${version}`);
			assert.equal(
				invalid.outputs,
				"",
				"invalid overrides must emit no version",
			);
		}
		const normal = run("push", "refs/heads/main", "");
		assert.equal(normal.status, 0, normal.stderr);
		assert.equal(
			normal.outputs,
			"version=1.0.0\nrelease=false\nversion=1.0.0\nrelease=true\n",
		);
		const checks = run("workflow_dispatch", "refs/heads/main", "");
		assert.equal(checks.status, 0, checks.stderr);
		assert.equal(checks.outputs, "version=1.0.0\nrelease=false\n");
	} finally {
		fs.rmSync(scratch, { recursive: true, force: true });
	}
});

test("the explicit rebuild exports platform, demo and manual artifacts without publishing", () => {
	const eligible = (job, event, ref, version = "") =>
		vm.runInNewContext(
			job.if,
			{
				github: { event_name: event, ref },
				inputs: { rebuild_version: version },
			},
			{ timeout: 100 },
		);
	for (const name of ["build", "demo-show"]) {
		const job = workflow.jobs[name];
		assert.equal(
			eligible(job, "workflow_dispatch", "refs/heads/main", "1.0.0"),
			true,
		);
		assert.equal(eligible(job, "workflow_dispatch", "refs/heads/main"), false);
		assert.equal(
			eligible(job, "workflow_dispatch", "refs/heads/main", "2.0.0"),
			false,
		);
		assert.equal(
			eligible(job, "workflow_dispatch", "refs/heads/feature", "1.0.0"),
			false,
		);
		assert.equal(eligible(job, "push", "refs/heads/main"), true);
	}
	assert.equal(
		eligible(
			workflow.jobs.metadata,
			"workflow_dispatch",
			"refs/heads/feature",
			"1.0.0",
		),
		true,
		"the selector must run and reject an explicit override on another branch",
	);
	assert.equal(
		eligible(
			workflow.jobs.release,
			"workflow_dispatch",
			"refs/heads/main",
			"1.0.0",
		),
		false,
	);
	assert.equal(
		eligible(workflow.jobs.release, "push", "refs/heads/main"),
		true,
	);
	assert.equal(
		build.env.LIGHT_RELEASE_VERSION,
		"${{ needs.metadata.outputs.version }}",
	);
	assert.ok(documentation.needs.includes("metadata"));
	const manual = byName(documentation, "Build and verify the operator manual");
	assert.equal(
		manual.env.LIGHT_MANUAL_VERSION,
		"${{ needs.metadata.outputs.version }}",
	);
	const upload = documentation.steps.find(
		(step) => step.with?.name === "one-off-1.0.0-manual-${{ github.sha }}",
	);
	assert.ok(upload);
	assert.equal(
		eligible(upload, "workflow_dispatch", "refs/heads/main", "1.0.0"),
		true,
	);
	assert.equal(eligible(upload, "push", "refs/heads/main"), false);
	assert.equal(
		eligible(upload, "workflow_dispatch", "refs/heads/feature", "1.0.0"),
		false,
	);
	assert.equal(
		vm.runInNewContext(
			documentation.if.replaceAll("needs.help-assets", 'needs["help-assets"]'),
			{
				always: () => true,
				cancelled: () => false,
				needs: {
					metadata: { result: "failure" },
					frontends: { result: "success" },
					"help-assets": { result: "success" },
				},
			},
		),
		false,
		"rejected override must prevent manual publication artifacts",
	);
	assert.equal(upload.with.path, ".artifacts/generated/manual");
	assert.equal(upload.with["if-no-files-found"], "error");
});
