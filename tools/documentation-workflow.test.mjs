import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
	chmodSync,
	mkdirSync,
	mkdtempSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { join } from "node:path";
import test from "node:test";
import vm from "node:vm";
import { artifactPaths } from "./artifact-paths.mjs";

const source = await readFile(
	process.env.LIGHT_DOCUMENTATION_WORKFLOW_TEST_SOURCE ??
		new URL("../.github/workflows/documentation.yml", import.meta.url),
	"utf8",
);
const concurrency = source.match(/^concurrency:\n([\s\S]*?)(?=^\S)/m)?.[1];
assert.ok(concurrency, "actual workflow concurrency must exist");
const group = concurrency.match(/ {2}group: (.+)(?:\n((?: {4}.+\n)+))?/);
assert.ok(group, "actual workflow concurrency group must exist");
const groupExpression = group[1] === ">-" ? group[2].trim() : group[1];
const jobGuard = source
	.match(
		/ {2}release-metadata:\n[\s\S]*? {4}if: >-\n([\s\S]*?) {4}runs-on:/,
	)?.[1]
	.trim();
assert.ok(jobGuard, "actual job eligibility guard must exist");

// This workflow uses only string equality, short-circuit &&/||, property reads and format.
// Evaluate that actual expression subset (not a duplicate eligibility implementation).
// Hosted concurrency scheduling remains a separately verified integration boundary.
function evaluate(expression, github) {
	return vm.runInNewContext(
		`(${expression})`,
		{
			github,
			format: (template, value) => template.replace("{0}", String(value)),
		},
		{ timeout: 100 },
	);
}
function resolvedGroup(github) {
	if (!groupExpression.startsWith("${{")) return groupExpression;
	return evaluate(groupExpression.slice(3, -2).trim(), github);
}
function completedRun(id, overrides = {}) {
	return {
		event_name: "workflow_run",
		run_id: id,
		event: {
			workflow_run: {
				conclusion: "success",
				event: "push",
				head_branch: "main",
				...overrides,
			},
		},
	};
}

test("eligible publication stays serialized without cancelling the active release", () => {
	const eligible = [
		completedRun(1),
		completedRun(2),
		{ event_name: "schedule", run_id: 3, event: {} },
		{
			event_name: "workflow_dispatch",
			run_id: 4,
			event: { inputs: { force: true } },
		},
		{
			event_name: "workflow_dispatch",
			run_id: 5,
			event: { inputs: { force: false } },
		},
	];
	for (const context of eligible) {
		assert.equal(evaluate(jobGuard, context), true);
		assert.equal(resolvedGroup(context), "nightly-documentation");
	}
	assert.match(concurrency, / {2}cancel-in-progress: false\s/);
});

test("job-skipped CI completions cannot replace either active or pending publication", () => {
	const active = resolvedGroup(completedRun(1));
	const pending = resolvedGroup(completedRun(2));
	assert.equal(
		active,
		pending,
		"eligible publication has one serialized group",
	);
	const ineligible = [
		{ head_branch: "codex/release-osc-subscription-boundary" },
		{ conclusion: "failure" },
		{ conclusion: "cancelled" },
		{ event: "workflow_dispatch" },
		{ event: "pull_request" },
		{ head_branch: undefined },
	];
	const ignoredGroups = new Set();
	for (const [index, fields] of ineligible.entries()) {
		const context = completedRun(100 + index, fields);
		assert.equal(evaluate(jobGuard, context), false);
		const ignored = resolvedGroup(context);
		assert.notEqual(ignored, active);
		assert.notEqual(ignored, pending);
		assert.equal(typeof ignored, "string");
		ignoredGroups.add(ignored);
	}
	assert.equal(ignoredGroups.size, ineligible.length);
});

test("repeated ignored invocations have independent groups even on the same branch", () => {
	const fields = { head_branch: "codex/release-osc-subscription-boundary" };
	assert.notEqual(
		resolvedGroup(completedRun(8, fields)),
		resolvedGroup(completedRun(9, fields)),
	);
});

test("handbook consumes the complete fresh gallery before rendering", () => {
	const manual = source.match(/^ {2}manual:\n([\s\S]*?)(?=^ {2}[\w-]+:)/m)?.[1];
	assert.ok(manual);
	assert.match(
		manual,
		/needs: \[release-metadata, release-performance, help-screenshots\]/,
	);
	assert.ok(
		manual.indexOf("name: help-screenshots") <
			manual.indexOf("run: npm run manual"),
	);
	for (const name of ["help-screenshots-storybook", "help-screenshots-live"]) {
		assert.match(
			source,
			new RegExp(`name: ${name}\\n          path: \\.artifacts/tmp/${name}`),
		);
	}
});

test("current manual publication verifies source-bound captures and preserves the site", async () => {
	const workflow = await readFile(
		new URL("../.github/workflows/manual-pages.yml", import.meta.url),
		"utf8",
	);
	assert.ok(
		workflow.indexOf("manual-captures.py restore") <
			workflow.indexOf("run: npm run manual"),
	);
	assert.match(workflow, /sha256sum --check/);
	assert.match(workflow, /group: nightly-documentation/);
	assert.match(workflow, /manual-publication\.json/);
	assert.match(
		workflow,
		/cp -R \.artifacts\/generated\/manual\/html\/tosklight-manual \.artifacts\/generated\/pages\/manual/,
	);
	assert.doesNotMatch(workflow, /rm -rf \.artifacts\/generated\/pages\n/);
});

test("marketing publication verifies current captures and preserves existing documentation", async () => {
	const { load } = createRequire(import.meta.url)("js-yaml");
	const workflow = load(
		await readFile(
			new URL("../.github/workflows/marketing-pages.yml", import.meta.url),
			"utf8",
		),
	);
	const steps = workflow.jobs.build.steps;
	const step = (name) => steps.find((entry) => entry.name === name);
	assert.deepEqual(Object.keys(workflow.on.workflow_dispatch.inputs), [
		"capture_release",
		"capture_asset",
		"capture_sha256",
	]);
	for (const input of Object.values(workflow.on.workflow_dispatch.inputs))
		assert.equal(input.required, true);
	assert.equal(workflow.concurrency.group, "nightly-documentation");
	assert.equal(workflow.concurrency["cancel-in-progress"], false);
	assert.deepEqual(workflow.jobs.build.permissions, {
		actions: "read",
		contents: "read",
	});
	assert.deepEqual(workflow.jobs.deploy.permissions, {
		pages: "write",
		"id-token": "write",
	});
	const capture = step("Download and verify source-bound captures");
	assert.match(capture.run, /sha256sum --check/);
	assert.match(
		capture.run,
		/python3 tools\/manual-captures\.py restore "\$archive"/,
	);
	assert.doesNotMatch(capture.run, /restore-native/);
	const render = step("Render current marketing pages and contact details");
	assert.ok(steps.indexOf(capture) < steps.indexOf(render));
	assert.match(
		render.run,
		/cp -R docs\/site\/\. \.artifacts\/generated\/pages\//,
	);
	assert.match(render.run, /node tools\/render-landing-page\.mjs/);
	assert.match(render.run, /site-legal\/cli\.mjs write/);
	assert.equal(
		render.env.LIGHT_PERFORMANCE_STATUS_FILE,
		".artifacts/generated/pages/performance/status.json",
	);
	assert.equal(
		render.env.TOSKLIGHT_IMPRINT_CONTACT,
		"${{ secrets.TOSKLIGHT_IMPRINT_CONTACT }}",
	);
	assert.match(
		step("Record published image provenance").run,
		/marketing-publication\.json/,
	);
	const publish = step("Publish refreshed marketing PNGs to preview and live");
	assert.match(publish.run, /upload-marketing-screenshots\.sh preview/);
	assert.match(publish.run, /upload-marketing-screenshots\.sh ''/);
	for (const entry of steps.filter((entry) => entry.run)) {
		assert.equal(
			spawnSync("bash", ["-n"], { input: entry.run, encoding: "utf8" }).status,
			0,
			entry.name,
		);
		assert.doesNotMatch(entry.run, /rm -rf/);
	}
	const reuse = step("Reuse the last successful published site");
	for (const path of [
		"manual/index.html",
		"storybook/index.html",
		"safari/index.html",
	])
		assert.ok(reuse.run.includes(path));
	assert.match(reuse.run, /LIGHT_RELEASE_VERSION=\$\{version#v\}/);
	// Execute the actual selection loop with a newer failed artifact ahead of successful ones.
	// This guards against overwriting the live site's documentation from a failed publication.
	mkdirSync(artifactPaths.tmp, { recursive: true });
	const scratch = mkdtempSync(join(artifactPaths.tmp, "marketing-workflow-"));
	try {
		const gh = join(scratch, "gh");
		writeFileSync(
			gh,
			`#!/bin/bash
if [[ "$1" == api ]]; then printf '101\n100\n99\n'; elif [[ "$1 $2 $3" == 'run view 101' ]]; then echo failure; elif [[ "$1 $2 $3" == 'run view 100' ]]; then echo success; else exit 1; fi
`,
		);
		chmodSync(gh, 0o755);
		const selection =
			reuse.run.slice(0, reuse.run.indexOf("mkdir -p")) +
			'printf "%s" "$base_run"';
		const selected = spawnSync("bash", ["-c", selection], {
			env: {
				...process.env,
				PATH: `${scratch}:${process.env.PATH}`,
				GITHUB_REPOSITORY: "example/repo",
			},
			encoding: "utf8",
		});
		assert.equal(selected.status, 0, selected.stderr);
		assert.equal(selected.stdout, "100");
	} finally {
		rmSync(scratch, { recursive: true, force: true });
	}
});

test("manual renderer and browser dependencies are reviewed exact lockfile inputs", async () => {
	const tooling = JSON.parse(
		await readFile(
			new URL("../docs/help/.tooling/package.json", import.meta.url),
			"utf8",
		),
	);
	const lock = JSON.parse(
		await readFile(
			new URL("../docs/help/.tooling/package-lock.json", import.meta.url),
			"utf8",
		),
	);
	assert.equal(tooling.private, true);
	assert.equal(tooling.dependencies["@tobisk/markdown-manuals"], "1.3.4");
	assert.deepEqual(tooling.overrides, {
		puppeteer: "25.7.0",
		"puppeteer-core": "25.7.0",
	});
	assert.deepEqual(lock.packages[""].dependencies, tooling.dependencies);
	assert.equal(
		lock.packages["node_modules/@tobisk/markdown-manuals"].version,
		"1.3.4",
	);
	for (const name of ["puppeteer", "puppeteer-core"]) {
		const packages = Object.entries(lock.packages).filter(
			([key]) =>
				key.endsWith(`/node_modules/${name}`) || key === `node_modules/${name}`,
		);
		assert.ok(packages.length, `locked ${name} must exist`);
		for (const [, entry] of packages) {
			assert.equal(entry.version, "25.7.0");
			assert.match(entry.integrity, /^sha512-/);
		}
	}
});

test("manual runner installs its isolated lock and uses its binary, preserving explicit overrides", async () => {
	const build = await readFile(new URL("./build.sh", import.meta.url), "utf8");
	const runner = build.match(/^run_manual_renderer\(\) \{[\s\S]*?^\}/m)?.[0];
	assert.ok(runner);
	mkdirSync(artifactPaths.tmp, { recursive: true });
	const cwd = mkdtempSync(join(artifactPaths.tmp, "manual-runner-"));
	try {
		const bin = join(cwd, "node_modules/.bin");
		mkdirSync(bin, { recursive: true });
		const executable = join(bin, "markdown-manual");
		writeFileSync(
			executable,
			'#!/usr/bin/env bash\nprintf "manual:%s|%s\\n" "$*" "$LIGHT_MANUAL_VERSION" >> "$TRACE"\n',
		);
		chmodSync(executable, 0o755);
		for (const override of [null, "@example/reviewed-renderer@2.0.0"]) {
			const trace = join(cwd, override ? "override" : "default");
			const env = {
				...process.env,
				MANUAL_TOOLING: cwd,
				LIGHT_MANUAL_VERSION: "1.0.0",
				TRACE: trace,
			};
			delete env.LIGHT_MANUAL_RENDERER_PACKAGE;
			if (override) env.LIGHT_MANUAL_RENDERER_PACKAGE = override;
			const result = spawnSync(
				"bash",
				[
					"-c",
					`set -euo pipefail
require(){ :; }
npm(){ printf 'npm:%s\\n' "$*" >> "$TRACE"; }
npx(){ printf 'npx:%s\\n' "$*" >> "$TRACE"; }
${runner}
run_manual_renderer build --source example`,
				],
				{ env, encoding: "utf8" },
			);
			assert.equal(result.status, 0, result.stderr);
			const calls = await readFile(trace, "utf8");
			assert.equal(
				calls,
				override
					? `npx:--yes --package ${override} markdown-manual build --source example\n`
					: `npm:ci --prefix ${cwd} --no-audit --no-fund\nmanual:build --source example|1.0.0\n`,
			);
		}
	} finally {
		rmSync(cwd, { recursive: true, force: true });
	}
});

test("reporting can remeasure a pinned release with separately identified current tooling", () => {
	const performance = source.match(
		/^ {2}release-performance:\n([\s\S]*?)(?=^ {2}[\w-]+:)/m,
	)?.[1];
	const pages = source.match(
		/^ {2}pages-build:\n([\s\S]*?)(?=^ {2}[\w-]+:)/m,
	)?.[1];
	assert.ok(performance && pages);
	assert.match(performance, /ref: \$\{\{ github.sha \}\}/);
	assert.match(pages, /ref: \$\{\{ github.sha \}\}/);
	assert.match(performance, /--commit "\$RELEASE_SHA"/);
	assert.match(performance, /--harness-commit "\$HARNESS_SHA"/);
	assert.match(
		performance,
		/sha256sum --check --ignore-missing report-checksums.txt/,
	);
});
