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
	PERFORMANCE_MEASUREMENT_SECONDS,
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
		schema_version: 8,
		benchmark: "tosklight_render_to_protocol_encoding_pipeline",
		reference: {
			hardware_label: "<CI runner>",
			cpu_model: "Test CPU",
			logical_cpus: 4,
			total_memory_bytes: 16 * 1024 ** 3,
			operating_system: "linux",
			architecture: "x86_64",
			rustc_version: "rustc test",
			package_version: "1.2.3",
			build_profile: "release",
		},
		scenarios: [
			{
				profile: "hard_floor",
				expectation: "required_floor",
				release_blocking: true,
				universes: 32,
				fixture_count: 1024,
				fixtures_per_universe: 32,
				fixture_footprint: 16,
				configured_rate_hz: 60,
				achieved_ticks_per_second: achieved,
				elapsed_seconds: 15.04,
				frame_rate: {
					average_completed_hz: achieved,
					minimum_one_second_completed_hz: minimum,
					p95_one_second_completed_hz: achieved + 1,
					maximum_one_second_completed_hz: achieved + 2,
					windows_below_minimum: deadlineMisses,
					reporting_target_hz: 44,
					windows_below_reporting_target: Math.min(deadlineMisses, 3),
				},
				dynamic_definition_count: 4,
				animated_attribute_count: 1024,
				master_lane_count: 4,
				dynamic_lane_attributes: ["intensity"],
				dynamic_excluded_fixture_count: 0,
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
				measurement_resources: {
					application_cpu_average_percent: 72,
					application_cpu_max_percent: 91,
					application_peak_resident_bytes: 512 * 1024 ** 2,
				},
			},
		],
		process_resources: {
			peak_resident_bytes: 512 * 1024 ** 2,
		},
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
		execution_mode: "unrestricted",
		harness_resources: { cpu_user_milliseconds: 2, max_resident_bytes: 10 },
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
	assert.equal(PERFORMANCE_MEASUREMENT_SECONDS, 15);
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
	assert.equal(status.required_floor.parameter_count, 16_384);
	assert.equal(status.required_floor.average_completed_hz, 63.25);
	assert.equal(status.required_floor.p95_one_second_completed_hz, 64.25);
	assert.equal(status.required_floor.maximum_one_second_completed_hz, 65.25);
	assert.equal(status.required_floor.dynamic_definition_count, 4);
	assert.deepEqual(status.required_floor.dynamic_lane_attributes, [
		"intensity",
	]);
	assert.equal(
		status.required_floor.resources.peak_resident_bytes,
		512 * 1024 ** 2,
	);
	assert.equal(status.runner.total_memory_bytes, 16 * 1024 ** 3);
	assert.equal(status.runner.package_version, "1.2.3");
	assert.equal(status.required_floor.met, true);
	assert.equal(status.required_floor.configured_target_met, false);
	assert.equal(status.required_floor.deadline_misses, 3);
	assert.equal(
		status.required_floor.limiting_phase.name,
		"Engine render and fixture projection",
	);
	assert.equal(status.show_mutation.large.p95_microseconds, 40);
	assert.equal(status.patch.server.single_fixture.p95_microseconds, 0);
	assert.match(status.report_url, /report-performance\.zip$/u);

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
	assert.equal(status.doubled_density.windows_below_minimum, 2);
	assert.equal(status.doubled_density.maximum_one_second_completed_hz, 84);
	assert.equal(status.doubled_density.dynamic_definition_count, 4);
});

test("the exact 2,000-fixture shipped-mode workload is retained separately", () => {
	const stressReport = report();
	const scenario = stressReport.scenarios[0];
	scenario.profile = "headless_stress";
	scenario.expectation = "informational_capacity";
	scenario.release_blocking = false;
	scenario.fixture_count = 2_000;
	scenario.fixture_footprint = null;
	scenario.universes = 74;
	scenario.fixture_inventory = { total_slots: 37_720 };
	stressReport.required_floor_met = null;
	delete stressReport.show_mutation;
	delete stressReport.patch_mutation;
	const status = statusDocument(
		options,
		stage(report(), 0),
		null,
		null,
		stage(stressReport, 0),
	);
	assert.equal(status.two_thousand_show.attempted, true);
	assert.equal(status.two_thousand_show.fixture_count, 2_000);
	assert.equal(status.two_thousand_show.parameter_count, 37_720);
	assert.equal(status.two_thousand_show.universes, 74);
	assert.equal(status.two_thousand_show.average_completed_hz, 100);
	assert.equal(status.two_thousand_show.maximum_one_second_completed_hz, 102);
	assert.equal(status.two_thousand_show.dynamic_definition_count, 4);
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
	const measuredStatus = JSON.parse(
		readFileSync(resolve(output, "status.json")),
	);
	assert.equal(measuredStatus.benchmark_scenarios.length, 8);
	assert.deepEqual(
		new Set(measuredStatus.benchmark_scenarios.map((scenario) => scenario.case_id)),
		new Set(["demo", "sixteen_universe", "required_1024", "doubled_2048"]),
	);
	assert.ok(
		measuredStatus.benchmark_scenarios.every(
			(scenario) =>
				scenario.requested_rate_hz === 60 &&
				scenario.below_target_hz === 44 &&
				Number.isFinite(scenario.resources.application_cpu_average_percent),
		),
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
	const manual =
		/^ {2}manual:\n([\s\S]*?)(?=^ {2}[\w-]+:\n)/mu.exec(workflow)?.[1] ?? "";
	const storybook =
		/^ {2}storybook-screenshots:\n([\s\S]*?)(?=^ {2}[\w-]+:\n)/mu.exec(
			workflow,
		)?.[1] ?? "";
	const liveBuild =
		/^ {2}help-live-build:\n([\s\S]*?)(?=^ {2}[\w-]+:\n)/mu.exec(
			workflow,
		)?.[1] ?? "";
	const pages =
		/^ {2}pages-build:\n([\s\S]*?)(?=^ {2}[\w-]+:\n)/mu.exec(workflow)?.[1] ??
		"";

	assert.match(release, /needs:[\s\S]*?- build/u);
	assert.doesNotMatch(release, /- benchmark|- pages-build/u);
	assert.match(workflow, /schedule:[\s\S]*?cron:/u);
	assert.match(workflow, /workflow_dispatch:/u);
	assert.match(performance, /needs: release-metadata/u);
	for (const expensiveJob of [manual, storybook, liveBuild]) {
		assert.match(
			expensiveJob,
			/needs: \[release-metadata, release-performance\]/u,
		);
	}
	assert.doesNotMatch(
		performance,
		/product-demo|--canonical-demo-performance/u,
	);
	assert.match(performance, /gh release download/u);
	assert.match(performance, /tools\/run-release-performance\.mjs/u);
	assert.doesNotMatch(performance, /continue-on-error: true/u);
	assert.match(workflow, /Check for an undocumented release/u);
	assert.match(workflow, /report-documentation\.json/u);
	assert.match(
		workflow,
		/gh release upload "\$RELEASE_TAG" --repo "\$GITHUB_REPOSITORY"/u,
	);
	assert.match(pages, /release-performance/u);
	assert.match(pages, /manual/u);
	assert.match(pages, /storybook-screenshots/u);
	assert.match(pages, /help-screenshots/u);
	assert.match(pages, /LIGHT_PERFORMANCE_STATUS_FILE/u);
	assert.match(pages, /name: storybook-static/u);
	assert.match(pages, /\.artifacts\/build\/storybook\/ui/u);
	assert.doesNotMatch(release, /storybook|manual|pages|performance/u);
	assert.match(releaseWorkflow, /SCCACHE_GHA_ENABLED: "true"/u);
	assert.match(releaseWorkflow, /RUSTC_WRAPPER: sccache/u);
	assert.equal(
		releaseWorkflow.match(
			/mozilla-actions\/sccache-action@7d986dd989559c6ecdb630a3fd2557667be217ad/g,
		)?.length,
		7,
	);
	assert.equal(releaseWorkflow.match(/version: v0\.17\.0/g)?.length, 7);
});
