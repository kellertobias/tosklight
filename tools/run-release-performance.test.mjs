import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
	chmodSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { artifactPaths } from "./artifact-paths.mjs";
import {
	classifyPerformance,
	statusDocument,
} from "./run-release-performance.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const options = {
	version: "1.2.3",
	commit: "0123456789abcdef",
	"release-url":
		"https://github.com/kellertobias/tosklight/releases/tag/v1.2.3",
};

function distribution(p50 = 0, p95 = 0) {
	return { p50_microseconds: p50, p95_microseconds: p95 };
}

function report({
	floor = true,
	showMutation = true,
	patchMutation = true,
	achieved = 100,
	minimum = achieved,
	deadlineMisses = 0,
} = {}) {
	return {
		schema_version: 6,
		benchmark: "tosklight_render_to_protocol_encoding_pipeline",
		reference: {
			hardware_label: "<CI runner>",
			cpu_model: "Test CPU",
			logical_cpus: 4,
			operating_system: "linux",
			architecture: "x86_64",
		},
		scenarios: [
			{
				profile: "hard_floor",
				expectation: "required_floor",
				release_blocking: true,
				universes: 32,
				fixture_count: 1024,
				fixtures_per_universe: 32,
				configured_rate_hz: 100,
				achieved_ticks_per_second: achieved,
				frame_rate: { minimum_one_second_completed_hz: minimum },
				deadline: {
					deadline_misses: deadlineMisses,
					dropped_ticks: 0,
					deferred_ticks: 0,
				},
				phases: {
					total_pipeline: distribution(500, 900),
					engine_render_combined: distribution(400, 800),
					protocol_encoding: distribution(50, 75),
					loopback_datagram_delivery: null,
					benchmark_validation_overhead: distribution(10, 20),
				},
			},
		],
		required_floor_met: floor,
		show_mutation: {
			gate_met: showMutation,
			small_fixture_count: 120,
			large_fixture_count: 1200,
			small: distribution(10, 20),
			large: distribution(30, 40),
		},
		patch_mutation: {
			gate_met: patchMutation,
			single_fixture: {
				total_server: distribution(0, 0),
				gate_p95_microseconds: 250_000,
				gate_met: patchMutation,
			},
			hundred_fixtures: {
				total_server: distribution(300, 400),
				gate_p95_microseconds: 500_000,
				gate_met: patchMutation,
			},
		},
	};
}

function stage(value, exitCode = 0) {
	const scenario = value?.scenarios?.[0];
	return {
		exit_code: exitCode,
		signal: null,
		error: null,
		report: value,
		fixtures_per_universe: scenario?.fixtures_per_universe,
		fixture_count: scenario?.fixture_count,
	};
}

function canonicalDemo() {
	return {
		schema_version: 1,
		measurement_surface: "browser_playwright_product_demo",
		scene: {
			fixture_records: 295,
			physical_instances: 343,
			stage_visible: true,
		},
		window: { elapsed_ms: 60_000 },
		stage: {
			presentation_rate_hz: 30,
			source_to_settled_canvas_ms: { p95: 61 },
			render_duration_ms: { p95: 3 },
		},
	};
}

test("invalid or inconsistent benchmark evidence is unknown", () => {
	assert.equal(
		classifyPerformance({ valid: false, reason: "invalid" }, null).status,
		"unknown",
	);
	const status = statusDocument(
		options,
		stage({ required_floor_met: false }, 1),
		null,
	);
	assert.equal(status.status, "unknown");
	assert.equal(status.evidence.kind, "unknown");
	assert.match(status.evidence.baseline.error, /missing required measured/u);
});

test("the 1,024-fixture indicator uses 60 Hz green and 40 Hz yellow thresholds", () => {
	const status = statusDocument(
		options,
		stage(
			report({ floor: false, achieved: 63.25, minimum: 63, deadlineMisses: 3 }),
			1,
		),
		null,
	);
	assert.equal(status.status, "healthy");
	assert.equal(status.evidence.kind, "measured");
	assert.equal(status.evidence.baseline.exit_code, 1);
	assert.deepEqual(status.evidence.failed_gates, []);
	assert.deepEqual(status.evidence.warnings, []);
	assert.equal(status.required_floor.achieved_ticks_per_second, 63.25);
	assert.equal(status.required_floor.met, true);
	assert.equal(status.required_floor.configured_target_met, false);
	assert.equal(status.required_floor.deadline_misses, 3);
	assert.equal(
		status.required_floor.limiting_phase.name,
		"Engine render and fixture projection",
	);
	assert.equal(status.show_mutation.large.p95_microseconds, 40);
	assert.equal(status.patch.server.single_fixture.p95_microseconds, 0);
	assert.match(
		status.report_url,
		/report-performance\.zip$/u,
	);

	const yellow = statusDocument(
		options,
		stage(report({ floor: false, achieved: 52, minimum: 49 }), 1),
		null,
	);
	assert.equal(yellow.status, "warning");
	assert.deepEqual(yellow.evidence.warnings, ["interactive_output_yellow"]);

	const red = statusDocument(
		options,
		stage(report({ floor: false, achieved: 39, minimum: 38 }), 1),
		null,
	);
	assert.equal(red.status, "degraded");
	assert.deepEqual(red.evidence.failed_gates, ["interactive_output_red"]);

	assert.equal(
		statusDocument(
			options,
			stage(report({ floor: false, achieved: 60, minimum: 60 }), 1),
			null,
		).status,
		"healthy",
	);
	assert.equal(
		statusDocument(
			options,
			stage(report({ floor: false, achieved: 40, minimum: 40 }), 1),
			null,
		).status,
		"warning",
	);
});

test("the canonical 306-instance product demo is retained as separate measured evidence", () => {
	const status = statusDocument(
		options,
		stage(report(), 0),
		null,
		canonicalDemo(),
	);
	assert.equal(status.canonical_demo.attempted, true);
	assert.equal(status.canonical_demo.scene.physical_instances, 343);
	assert.equal(status.canonical_demo.stage.presentation_rate_hz, 30);
	assert.equal(status.canonical_demo.stage.render_duration_ms.p95, 3);

	const unavailable = statusDocument(options, stage(report(), 0), null, {});
	assert.equal(unavailable.canonical_demo.attempted, true);
	assert.match(unavailable.canonical_demo.reason, /did not produce valid/u);
});

test("passing baseline remains healthy when the optional density probe degrades", () => {
	const baseline = stage(report(), 0);
	const doubledReport = report({
		floor: false,
		achieved: 82,
		deadlineMisses: 2,
	});
	doubledReport.scenarios[0].fixtures_per_universe = 64;
	doubledReport.scenarios[0].fixture_count = 2048;
	delete doubledReport.show_mutation;
	delete doubledReport.patch_mutation;
	const status = statusDocument(options, baseline, stage(doubledReport, 1));
	assert.equal(status.status, "healthy");
	assert.equal(status.doubled_density.attempted, true);
	assert.equal(status.doubled_density.met, null);
	assert.equal(status.doubled_density.configured_target_met, false);
	assert.equal(status.doubled_density.achieved_ticks_per_second, 82);
	assert.equal(status.doubled_density.deadline_misses, 2);
});

test("CLI accepts a parsed measured failure but rejects invalid JSON", () => {
	const temporary = mkdtempSync(
		resolve(artifactPaths.tmp, "release-performance-test-"),
	);
	const executable = resolve(temporary, "fake-benchmark.mjs");
	const canonicalDemoPath = resolve(
		temporary,
		"canonical-demo-performance.json",
	);
	const output = resolve(temporary, "output");
	mkdirSync(output, { recursive: true });
	writeFileSync(
		executable,
		`#!/usr/bin/env node\nconsole.log(${JSON.stringify(
			JSON.stringify(report({ floor: false, achieved: 88, deadlineMisses: 4 })),
		)}); process.exit(1);\n`,
	);
	chmodSync(executable, 0o755);
	writeFileSync(canonicalDemoPath, JSON.stringify(canonicalDemo()));
	const measured = spawnSync(
		process.execPath,
		[
			resolve(ROOT, "tools/run-release-performance.mjs"),
			"--binary",
			executable,
			"--output-dir",
			output,
			"--version",
			options.version,
			"--commit",
			options.commit,
			"--release-url",
			options["release-url"],
			"--canonical-demo-performance",
			canonicalDemoPath,
		],
		{ encoding: "utf8" },
	);
	assert.equal(measured.status, 0, measured.stderr);
	assert.equal(
		JSON.parse(readFileSync(resolve(output, "status.json"))).status,
		"healthy",
	);
	assert.equal(
		JSON.parse(readFileSync(resolve(output, "status.json"))).canonical_demo
			.scene.physical_instances,
		343,
	);
	assert.equal(
		JSON.parse(readFileSync(resolve(output, "hard-floor.json")))
			.required_floor_met,
		false,
	);

	writeFileSync(
		executable,
		'#!/usr/bin/env node\nconsole.log("not json"); process.exit(1);\n',
	);
	const invalid = spawnSync(
		process.execPath,
		[
			resolve(ROOT, "tools/run-release-performance.mjs"),
			"--binary",
			executable,
			"--output-dir",
			output,
			"--version",
			options.version,
			"--commit",
			options.commit,
			"--release-url",
			options["release-url"],
		],
		{ encoding: "utf8" },
	);
	assert.equal(invalid.status, 1);
	assert.equal(
		JSON.parse(readFileSync(resolve(output, "status.json"))).status,
		"unknown",
	);
});

test("scheduled publication separates release delivery from performance and Pages", () => {
	const releaseWorkflow = readFileSync(
		resolve(ROOT, ".github/workflows/release.yml"),
		"utf8",
	);
	const workflow = readFileSync(
		resolve(ROOT, ".github/workflows/documentation.yml"),
		"utf8",
	);
	const release =
		/^ {2}release:\n([\s\S]*?)(?=^ {2}[\w-]+:\n|(?![\s\S]))/mu.exec(
			releaseWorkflow,
		)?.[1] ?? "";
	const performance =
		/^ {2}release-performance:\n([\s\S]*?)(?=^ {2}[\w-]+:\n)/mu.exec(
			workflow,
		)?.[1] ?? "";
	const pages =
		/^ {2}pages-build:\n([\s\S]*?)(?=^ {2}[\w-]+:\n)/mu.exec(workflow)?.[1] ??
		"";

	assert.match(release, /needs:[\s\S]*?- build/u);
	assert.doesNotMatch(release, /- benchmark|- pages-build/u);
	assert.match(workflow, /schedule:[\s\S]*?cron:/u);
	assert.match(performance, /needs: release-metadata/u);
	assert.doesNotMatch(performance, /product-demo|--canonical-demo-performance/u);
	assert.match(performance, /gh release download/u);
	assert.match(performance, /tools\/run-release-performance\.mjs/u);
	assert.doesNotMatch(performance, /continue-on-error: true/u);
	assert.match(workflow, /Check for an undocumented release/u);
	assert.match(workflow, /report-documentation\.json/u);
	assert.match(pages, /release-performance/u);
	assert.match(pages, /manual/u);
	assert.match(pages, /storybook-screenshots/u);
	assert.match(pages, /help-screenshots/u);
	assert.match(pages, /LIGHT_PERFORMANCE_STATUS_FILE/u);
	assert.match(pages, /name: storybook-static/u);
	assert.match(pages, /\.artifacts\/build\/storybook\/ui/u);
	assert.doesNotMatch(release, /storybook|manual|pages|performance/u);
});
