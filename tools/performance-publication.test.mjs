import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
	copyFileSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { artifactPaths } from "./artifact-paths.mjs";
import {
	normalizePublicPerformanceStatus,
	performanceReportUrl,
	renderPerformancePage,
} from "./performance-publication.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function measuredStatus(status = "healthy") {
	return {
		schema_version: 3,
		status,
		summary:
			status === "healthy"
				? "All required gates passed."
				: "The output floor was missed.",
		generated_at: "2026-07-30T12:34:56.000Z",
		release: {
			version: "2.4.6",
			commit: "0123456789abcdef",
			url: "https://github.com/kellertobias/tosklight/releases/tag/v2.4.6",
		},
		report_url:
			"https://github.com/kellertobias/tosklight/releases/download/v2.4.6/tosklight-performance-report-2.4.6.zip",
		evidence: {
			kind: "measured",
			baseline: {
				exit_code: status === "healthy" ? 0 : 1,
				signal: null,
				error: null,
			},
			failed_gates: status === "healthy" ? [] : ["required_floor"],
		},
		runner: {
			hardware_label: "GitHub Actions ubuntu-22.04",
			cpu_model: "Test CPU",
			logical_cpus: 4,
			operating_system: "linux",
			architecture: "x86_64",
		},
		workload: {
			benchmark: "tosklight_render_to_protocol_encoding_pipeline",
			profile: "hard_floor",
			universes: 32,
			fixture_count: 1024,
			requested_rate_hz: 100,
		},
		required_floor: {
			universes: 32,
			rate_hz: 100,
			fixtures_per_universe: 32,
			fixture_count: 1024,
			met: status === "healthy",
			configured_target_met: status === "healthy",
			green_threshold_hz: 60,
			yellow_threshold_hz: 40,
			achieved_ticks_per_second: status === "healthy" ? 100.2 : 87.5,
			minimum_one_second_completed_hz: status === "healthy" ? 100 : 82,
			deadline_misses: status === "healthy" ? 0 : 3,
			dropped_ticks: 0,
			deferred_ticks: 0,
			limiting_phase: {
				name: "Engine render and fixture projection",
				p50_microseconds: 400,
				p95_microseconds: 800,
			},
		},
		canonical_demo: {
			attempted: true,
			reason: null,
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
				max_draw_calls: 402,
				max_triangles: 120_000,
			},
		},
		show_mutation: {
			gate_met: true,
			small: {
				fixture_count: 120,
				p50_microseconds: 110,
				p95_microseconds: 170,
			},
			large: {
				fixture_count: 1200,
				p50_microseconds: 130,
				p95_microseconds: 190,
			},
		},
		patch: {
			server: {
				single_fixture: {
					p50_microseconds: 0,
					p95_microseconds: 250,
					gate_p95_microseconds: 250_000,
					gate_met: true,
				},
				hundred_fixtures: {
					p50_microseconds: 500,
					p95_microseconds: 900,
					gate_p95_microseconds: 500_000,
					gate_met: true,
				},
				gate_met: true,
			},
		},
		doubled_density: {
			attempted: true,
			reason: null,
			fixtures_per_universe: 64,
			fixture_count: 2048,
			met: false,
			configured_target_met: false,
			achieved_ticks_per_second: 72.25,
			minimum_one_second_completed_hz: 69,
			deadline_misses: 8,
			dropped_ticks: 0,
			deferred_ticks: 1,
			limiting_phase: {
				name: "Protocol encoding",
				p50_microseconds: 900,
				p95_microseconds: 1200,
			},
		},
	};
}

test("healthy and degraded measured statuses retain their public evidence", () => {
	const healthy = measuredStatus("healthy");
	const degraded = measuredStatus("degraded");
	assert.strictEqual(normalizePublicPerformanceStatus(healthy), healthy);
	assert.strictEqual(normalizePublicPerformanceStatus(degraded), degraded);

	const page = renderPerformancePage(degraded);
	assert.match(page, /87\.5 Hz/u);
	assert.match(page, /Deadline misses<\/th><td>3<\/td>/u);
	assert.match(page, /required_floor/u);
	assert.match(page, /1200 fixtures<\/th><td>130 µs<\/td><td>190 µs/u);
	assert.match(page, /72\.25 Hz/u);
	assert.match(
		page,
		/Green means at least 60 Hz; yellow means at least 40 Hz/u,
	);
	assert.match(page, /Engine render and fixture projection/u);
	assert.match(page, /Protocol encoding/u);
	assert.match(page, /diagnostic only/u);
	assert.match(page, /Canonical 306-instance demo show/u);
	assert.match(page, /Physical Stage instances<\/th><td>343/u);
	assert.match(page, /Stage source-to-canvas p95<\/th><td>61 ms/u);
});

test("warning is a valid measured public state", () => {
	const warning = measuredStatus("warning");
	warning.summary = "The 1,024-fixture workload is between 40 and 60 Hz.";
	assert.strictEqual(normalizePublicPerformanceStatus(warning), warning);
	const page = renderPerformancePage(warning);
	assert.match(page, /class="warning">WARNING/u);
	assert.match(page, /<th>Acceptance tier<\/th><td>warning<\/td>/u);
});

test("unknown and invalid measured classifications cannot masquerade as evidence", () => {
	const unknown = {
		schema_version: 3,
		status: "unknown",
		summary: "The benchmark executable crashed.",
		evidence: { kind: "unknown", reason: "process_crash" },
	};
	assert.strictEqual(normalizePublicPerformanceStatus(unknown), unknown);

	const invalid = normalizePublicPerformanceStatus(
		{ schema_version: 3, status: "degraded", summary: "Trust me." },
		{
			version: "2.4.6",
			releaseUrl:
				"https://github.com/kellertobias/tosklight/releases/tag/v2.4.6",
		},
	);
	assert.equal(invalid.status, "unknown");
	assert.equal(invalid.evidence.kind, "unknown");
	assert.doesNotMatch(renderPerformancePage(invalid), /Trust me/u);
});

test("zero values remain visible and partial optional evidence is tolerated", () => {
	const status = measuredStatus();
	status.required_floor.achieved_ticks_per_second = 0;
	status.required_floor.minimum_one_second_completed_hz = 0;
	status.required_floor.deadline_misses = 0;
	status.show_mutation = null;
	status.patch.server.hundred_fixtures = null;
	status.doubled_density = {
		attempted: false,
		reason: "The required floor did not pass.",
		fixtures_per_universe: 64,
		fixture_count: 2048,
		met: null,
	};

	const page = renderPerformancePage(status);
	assert.match(page, /Achieved output cadence<\/th><td>0 Hz<\/td>/u);
	assert.match(page, /Minimum one-second cadence<\/th><td>0 Hz<\/td>/u);
	assert.match(page, /Deadline misses<\/th><td>0<\/td>/u);
	assert.match(page, /The required floor did not pass\./u);
	assert.match(page, /100 fixtures<\/th><td>—<\/td>/u);
});

test("report-controlled text and URLs are escaped or rejected", () => {
	const status = measuredStatus("degraded");
	status.summary = '<script>alert("summary")</script>';
	status.runner.hardware_label = '<img src=x onerror="runner">';
	status.workload.profile = "<unsafe>";
	status.evidence.failed_gates = ["<gate>"];
	status.report_url = "javascript:alert(1)";
	status.release.url = "javascript:alert(2)";

	const page = renderPerformancePage(status);
	assert.doesNotMatch(page, /<script>|<img|javascript:/u);
	assert.match(
		page,
		/&lt;script&gt;alert\(&quot;summary&quot;\)&lt;\/script&gt;/u,
	);
	assert.match(page, /&lt;img src=x onerror=&quot;runner&quot;&gt;/u);
	assert.match(page, /&lt;unsafe&gt;/u);
	assert.match(page, /&lt;gate&gt;/u);
	assert.match(page, /detailed benchmark report is unavailable/u);
});

test("the performance report link is a direct release asset URL", () => {
	const status = measuredStatus();
	assert.equal(performanceReportUrl(status), status.report_url);

	delete status.report_url;
	assert.equal(
		performanceReportUrl(status),
		"https://github.com/kellertobias/tosklight/releases/download/v2.4.6/tosklight-performance-report-2.4.6.zip",
	);
});

test("landing-page assembly writes the same normalized object used by the HTML", () => {
	mkdirSync(artifactPaths.tmp, { recursive: true });
	const directory = mkdtempSync(
		resolve(artifactPaths.tmp, "performance-publication-"),
	);
	try {
		const target = resolve(directory, "index.html");
		const statusFile = resolve(directory, "input-status.json");
		const status = measuredStatus("degraded");
		copyFileSync(resolve(ROOT, "docs/site/index.html"), target);
		writeFileSync(statusFile, `${JSON.stringify(status, null, 2)}\n`);

		const result = spawnSync(
			process.execPath,
			[resolve(ROOT, "tools/render-landing-page.mjs"), target],
			{
				cwd: ROOT,
				encoding: "utf8",
				env: {
					...process.env,
					LIGHT_RELEASE_VERSION: status.release.version,
					LIGHT_PERFORMANCE_STATUS_FILE: statusFile,
				},
			},
		);
		assert.equal(result.status, 0, result.stderr);
		assert.deepEqual(
			JSON.parse(
				readFileSync(resolve(directory, "performance/status.json"), "utf8"),
			),
			status,
		);
		const page = readFileSync(
			resolve(directory, "performance/index.html"),
			"utf8",
		);
		assert.match(page, /87\.5 Hz/u);
		assert.match(page, /tosklight-performance-report-2\.4\.6\.zip/u);
	} finally {
		rmSync(directory, { recursive: true, force: true });
	}
});
