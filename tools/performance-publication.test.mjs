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
	renderCompactPerformanceSummary,
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
			"https://github.com/kellertobias/tosklight/releases/download/v2.4.6/report-performance.zip",
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
			total_memory_bytes: 16 * 1024 ** 3,
			operating_system: "linux",
			architecture: "x86_64",
			package_version: "2.4.6",
			rustc_version: "rustc 1.88.0",
			build_profile: "release",
		},
		workload: {
			benchmark: "tosklight_render_to_protocol_encoding_pipeline",
			profile: "hard_floor",
			universes: 32,
			fixture_count: 1024,
			parameter_count: 16_384,
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
			average_completed_hz: status === "healthy" ? 100.1 : 87.5,
			p95_one_second_completed_hz: status === "healthy" ? 101 : 91,
			maximum_one_second_completed_hz: status === "healthy" ? 102 : 94,
			windows_below_minimum: status === "healthy" ? 0 : 3,
			dynamic_definition_count: 4,
			dynamic_lane_attributes: ["intensity"],
			dynamic_excluded_fixture_count: 0,
			deadline_misses: status === "healthy" ? 0 : 3,
			dropped_ticks: 0,
			deferred_ticks: 0,
			resources: { peak_resident_bytes: 512 * 1024 ** 2 },
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
		two_thousand_show: {
			attempted: true,
			fixture_count: 2_000,
			parameter_count: 37_720,
			universes: 74,
			minimum_one_second_completed_hz: 57,
			average_completed_hz: 60,
			p95_one_second_completed_hz: 62,
			maximum_one_second_completed_hz: 64,
			windows_below_minimum: 1,
			requested_rate_hz: 100,
			dynamic_definition_count: 20,
			dynamic_lane_attributes: [
				"intensity",
				"color.red",
				"color.green",
				"color.blue",
				"pan",
				"tilt",
			],
			dynamic_excluded_fixture_count: 920,
			resources: { peak_resident_bytes: 700 * 1024 ** 2 },
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
			parameter_count: 16_384,
			universes: 32,
			met: false,
			configured_target_met: false,
			achieved_ticks_per_second: 72.25,
			minimum_one_second_completed_hz: 69,
			average_completed_hz: 72,
			p95_one_second_completed_hz: 74,
			maximum_one_second_completed_hz: 76,
			windows_below_minimum: 2,
			requested_rate_hz: 100,
			dynamic_definition_count: 4,
			dynamic_lane_attributes: ["intensity"],
			dynamic_excluded_fixture_count: 0,
			deadline_misses: 8,
			dropped_ticks: 0,
			deferred_ticks: 1,
			limiting_phase: {
				name: "Protocol encoding",
				p50_microseconds: 900,
				p95_microseconds: 1200,
			},
		},
		benchmark_scenarios: [
			{
				case_name: "37,720 parameters / 2,000 fixtures",
				execution_mode: "unrestricted",
				fixture_count: 2_000,
				parameter_count: 37_720,
				universes: 74,
				requested_rate_hz: 60,
				minimum_one_second_completed_hz: 42,
				average_completed_hz: 51,
				p95_one_second_completed_hz: 44,
				maximum_one_second_completed_hz: 58,
				below_target_hz: 44,
				windows_below_target: 1,
				measurement_seconds: 15.04,
				animated_attribute_count: 6_480,
				master_lane_count: 20,
				thresholds: { red_below_hz: 30, yellow_below_hz: 45 },
				resources: {
					application_cpu_average_percent: 83,
					application_cpu_max_percent: 97,
					application_peak_resident_bytes: 700 * 1024 ** 2,
				},
			},
			...(["unrestricted", "one_core"].map((execution_mode) => ({
				case_name: "Demo show — 4,096 parameters / 295 fixtures",
				execution_mode,
				fixture_count: 295,
				parameter_count: 4_096,
				universes: 8,
				requested_rate_hz: 60,
				minimum_one_second_completed_hz: 58,
				average_completed_hz: 59.5,
				p95_one_second_completed_hz: 59,
				maximum_one_second_completed_hz: 60,
				below_target_hz: 44,
				windows_below_target: 0,
				measurement_seconds: 15.01,
				animated_attribute_count: 295,
				master_lane_count: 5,
				thresholds: {
					red_below_hz: 44,
					critical_below_hz: 40,
					yellow_below_hz: 59,
				},
				resources: {
					application_cpu_average_percent: 25,
					application_cpu_max_percent: 31,
					application_peak_resident_bytes: 256 * 1024 ** 2,
				},
			}))),
		],
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
	assert.match(page, /Show statistics/u);
	assert.match(page, /<strong>295<\/strong><small>fixture records/u);
	assert.match(page, /100-fixture show/u);
	assert.match(page, /2,000-fixture mixed shipped-mode show/u);
	assert.match(page, /<strong>37,?720<\/strong>|<strong>37720<\/strong>/u);
	assert.match(page, /2,048-fixture capacity diagnostic/u);
	assert.match(page, /<strong>87\.5 Hz<\/strong>/u);
	assert.match(page, /warning: 40 Hz ≤ x &lt; 60 Hz/u);
	assert.match(page, /<strong>Not measured<\/strong><small>not collected/u);
	assert.match(page, /Runner configuration/u);
	assert.match(page, /Logical cores<\/th><td>4/u);
	assert.match(page, /RAM<\/th><td>16\.0 GiB/u);
});

test("compact landing summary includes only measured runs and explains row colors", () => {
	const summary = renderCompactPerformanceSummary(measuredStatus("degraded"));
	assert.equal(summary.match(/class="performance-row /gu)?.length, 3);
	assert.match(
		summary,
		/<th>Test set<\/th><th>Load<\/th><th>Statistics<\/th>/u,
	);
	assert.match(summary, /37,720 parameters \/ 2,000 fixtures/u);
	assert.match(summary, /unrestricted<br>74 DMX universes/u);
	assert.match(summary, /<strong>6,480 dyn\. attr\.<\/strong>/u);
	assert.match(summary, /20 master lanes/u);
	assert.match(summary, /<strong>44 Hz p95<\/strong>/u);
	assert.match(
		summary,
		/<strong>1 \/ 15\.04 s<\/strong><small>one-second windows<br>below 44 Hz<br>total test time/u,
	);
	assert.match(
		summary,
		/97% max<\/strong><small>83% avg Light CPU<br>0\.68 GiB max RAM/u,
	);
	assert.match(summary, /58 \/ 59\.5 \/ 60 Hz<br>min \/ avg \/ max/u);
	assert.match(summary, /This workflow does not launch Playwright/u);
	assert.match(summary, /100% means one fully used core/u);
	assert.match(summary, /performance-row-warning/u);
	assert.match(summary, /Yellow<\/span>: warning/u);
	assert.ok(summary.indexOf("Demo show") < summary.indexOf("37,720 parameters"));
	assert.match(summary, /locked to 1 core/u);
	assert.match(summary, /unrestricted/u);
	assert.match(summary, /Detailed tests, run information, and raw report/u);
	assert.doesNotMatch(summary, /100 fixtures/u);
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
		"https://github.com/kellertobias/tosklight/releases/download/v2.4.6/report-performance.zip",
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
		const screenshots = resolve(directory, "source-screenshots");
		const status = measuredStatus("degraded");
		copyFileSync(resolve(ROOT, "docs/site/index.html"), target);
		writeFileSync(statusFile, `${JSON.stringify(status, null, 2)}\n`);
		mkdirSync(screenshots, { recursive: true });
		const manifest = JSON.parse(
			readFileSync(
				resolve(ROOT, "docs/marketing/screenshot-manifest.json"),
				"utf8",
			),
		);
		for (const entry of manifest.entries) {
			writeFileSync(resolve(screenshots, entry.file), "fixture image");
		}

		const result = spawnSync(
			process.execPath,
			[resolve(ROOT, "tools/render-landing-page.mjs"), target],
			{
				cwd: ROOT,
				encoding: "utf8",
				env: {
					...process.env,
					LIGHT_MARKETING_SCREENSHOTS_DIR: screenshots,
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
		const detailPage = readFileSync(
			resolve(directory, "performance/index.html"),
			"utf8",
		);
		assert.match(detailPage, /87\.5 Hz/u);
		assert.match(detailPage, /report-performance\.zip/u);
		const landingPage = readFileSync(target, "utf8");
		assert.match(landingPage, /class="performance-compact"/u);
		assert.match(landingPage, /37,720 parameters \/ 2,000 fixtures/u);
		assert.match(landingPage, /6,480 dyn\. attr\./u);
		assert.match(landingPage, /Yellow<\/span>: warning/u);
		assert.match(landingPage, /table-layout: fixed/u);
		assert.doesNotMatch(landingPage, /min-width: 58rem/u);
	} finally {
		rmSync(directory, { recursive: true, force: true });
	}
});
