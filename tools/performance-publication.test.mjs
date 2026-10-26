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
				measurement_surface: "released-tauri-desk-fixture-sheet",
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
					server: {
						cpu_average_percent: 61,
						cpu_max_percent: 72,
						peak_resident_bytes: 500 * 1024 ** 2,
					},
					desktop_webview: {
						cpu_average_percent: 22,
						cpu_max_percent: 25,
						peak_resident_bytes: 200 * 1024 ** 2,
					},
					playwright: { launched: false },
				},
			},
			...["unrestricted", "one_core"].map((execution_mode) => ({
				case_name: "Demo show — 4,096 parameters / 295 fixtures",
				execution_mode,
				measurement_surface: "released-tauri-desk-fixture-sheet",
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
					server: {
						cpu_average_percent: 18,
						cpu_max_percent: 22,
						peak_resident_bytes: 192 * 1024 ** 2,
					},
					desktop_webview: {
						cpu_average_percent: 7,
						cpu_max_percent: 9,
						peak_resident_bytes: 64 * 1024 ** 2,
					},
					playwright: { launched: false },
				},
			})),
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
	assert.match(page, /Separate Stage demonstration/u);
	assert.match(page, /Physical Stage instances<\/th><td>343/u);
	assert.match(page, /Stage source-to-canvas p95<\/th><td>61 ms/u);
	assert.match(page, /Measured shows/u);
	assert.equal(page.match(/class="performance-row /gu)?.length, 3);
	assert.match(page, /37,720 parameters \/ 2,000 fixtures/u);
	assert.match(page, /Demo show — 4,096 parameters \/ 295 fixtures/u);
	assert.doesNotMatch(page, /100-fixture show|Not measured/u);
	assert.match(page, /same measured-scenario table shown on the main/u);
	assert.match(page, /How the test runs/u);
	assert.match(page, /Released Linux bundle/u);
	assert.match(page, /Tauri Desk under Xvfb/u);
	assert.match(page, /WebKit Fixture Sheet/u);
	assert.match(page, /Bundled Light server/u);
	assert.match(page, /40 Hz scheduler/u);
	assert.match(page, /Node and Xvfb are excluded/u);
	assert.match(page, /Playwright is not launched/u);
	assert.match(page, /Application process breakdown/u);
	assert.match(page, /Complete Desk app tree/u);
	assert.match(page, /Bundled Light server/u);
	assert.match(page, /WebKit<\/td>/u);
	assert.match(page, /61%<\/td><td>72%<\/td><td>0\.49 GiB/u);
	assert.match(page, /Runner configuration/u);
	assert.match(page, /Logical cores<\/th><td>4/u);
	assert.match(page, /RAM<\/th><td>16\.0 GiB/u);
});

test("compact landing summary includes only measured runs and explains row colors", () => {
	const summary = renderCompactPerformanceSummary(measuredStatus("degraded"));
	assert.equal(summary.match(/class="performance-row /gu)?.length, 3);
	assert.match(summary, /Measured release: 2\.4\.6/);
	assert.match(
		summary,
		/<th>Test set<\/th><th>Load<\/th><th>Output cadence<\/th><th>Output intervals<\/th><th>CPU and RAM<\/th>/u,
	);
	assert.match(summary, /37,720 parameters \/ 2,000 fixtures/u);
	assert.match(summary, /unrestricted<br>74 patched DMX universes/u);
	assert.match(summary, /6,480 dyn\. attr\./u);
	assert.match(summary, /20 master lanes/u);
	assert.match(summary, /<strong>44 Hz · 95% floor<\/strong>/u);
	// The frame-time column carries the histogram and the window length it covers.
	assert.match(summary, /Frame times not measured<\/small><small>15\.04 s nominal<\/small>/u);
	assert.match(
		summary,
		/97% max<\/strong><small>83% avg · 97% max Desk app CPU<br>0\.68 GiB max RAM/u,
	);
	assert.match(summary, /real WebKit Fixture Sheet/u);
	assert.match(summary, /Playwright is not launched/u);
	assert.match(summary, /58 \/ 59\.5 \/ 60 Hz<br>min \/ avg \/ max/u);
	assert.match(summary, /100% means one fully used core/u);
	// Every one of these runs holds its floor at the 40 Hz target: the slowest second of the
	// hardest row is 42 Hz, so all three grade healthy rather than one being a warning.
	assert.equal(summary.match(/performance-row-healthy/gu)?.length, 3);
	assert.doesNotMatch(summary, /performance-row-warning|performance-row-degraded/u);
	assert.match(summary, /Yellow<\/span>: slowest above 30 Hz/u);
	assert.ok(
		summary.indexOf("Demo show") < summary.indexOf("37,720 parameters"),
	);
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

test("a scenario that stalled below 40 Hz is never presented as healthy", () => {
	const stalled = measuredStatus("warning");
	stalled.benchmark_scenarios = [
		{
			case_id: "required_1024",
			case_name: "16,384 parameters / 1,024 fixtures",
			execution_mode: "one_core",
			fixture_count: 1_024,
			parameter_count: 16_384,
			universes: 32,
			requested_rate_hz: 60,
			below_target_hz: 44,
			windows_below_target: 14,
			measurement_seconds: 15,
			// A comfortable typical rate that hides seconds the operator saw stall.
			average_completed_hz: 25,
			minimum_one_second_completed_hz: 12,
			p95_one_second_completed_hz: 81,
			maximum_one_second_completed_hz: 81,
			animated_attribute_count: 8_578,
			master_lane_count: 20,
			thresholds: { red_below_hz: 40, yellow_below_hz: 44 },
			resources: { application_cpu_average_percent: 100 },
		},
	];
	const summary = renderCompactPerformanceSummary(stalled);
	// A slowest second of 12 Hz clears none of the floors, whatever the p95 looks like.
	assert.match(summary, /class="performance-row performance-row-degraded"/u);
	assert.doesNotMatch(summary, /performance-row-healthy/u);
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
	assert.doesNotMatch(page, /<script>|javascript:/u);
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

test("README clearly warns that beta builds have not been tested at a show", () => {
	const readme = readFileSync(resolve(ROOT, "README.md"), "utf8");
	assert.match(readme, /> \[!WARNING\]/u);
	assert.match(readme, /ToskLight is beta software and has not yet been tested at a show/u);
	assert.doesNotMatch(readme, /> \[!CAUTION\]/u);
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
		const screenshotFixture = Buffer.alloc(33);
		Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).copy(screenshotFixture);
		screenshotFixture.writeUInt32BE(13, 8);
		screenshotFixture.write("IHDR", 12, "ascii");
		screenshotFixture.writeUInt32BE(1552, 16);
		screenshotFixture.writeUInt32BE(265, 20);
		for (const entry of manifest.entries) {
			writeFileSync(resolve(screenshots, entry.file), screenshotFixture);
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
		assert.match(detailPage, /<h1>Development<\/h1>/u);
		assert.match(detailPage, /href="\.\.\/storybook\/"/u);
		assert.match(detailPage, /href="\.\.\/safari\/"/u);
		assert.match(detailPage, /semantic-test-catalog\.html/u);
		assert.doesNotMatch(detailPage, /href="status\.json"/u);
		assert.doesNotMatch(detailPage, /Performance methodology|Stage acceptance contract/u);
		const downloadsPage = readFileSync(
			resolve(directory, "downloads/index.html"),
			"utf8",
		);
		assert.match(downloadsPage, /<h1>Download ToskLight\.<\/h1>/u);
		assert.match(downloadsPage, /Currently in development/u);
		assert.match(downloadsPage, /Release candidate/u);
		assert.match(downloadsPage, /Published with Pixel/u);
		assert.equal(downloadsPage.match(/class="download-button"/gu)?.length, 3);
		assert.equal(downloadsPage.match(/id="platform-download"/gu)?.length, 1);
		assert.equal(downloadsPage.match(/<option value=/gu)?.length, 4);
		assert.match(downloadsPage, /Useful alongside the suite/u);
		assert.match(downloadsPage, /href="\.\.\/license\/"/u);
		assert.match(downloadsPage, /href="\.\.\/impressum\/"/u);
		assert.match(downloadsPage, /href="\.\.\/datenschutz\/"/u);
        // Pages must use the exact capture artifact, including native surfaces. The tracked
        // still may predate the reviewed capture restored by CI.
        assert.deepEqual(readFileSync(resolve(directory, "screenshots/architect-renderer-ultra.png")), screenshotFixture);
        for (const product of ["control", "pixel", "architect"]) {
            const html = readFileSync(resolve(directory, product, "index.html"), "utf8");
            const shots = [...html.matchAll(/src="(\.\.\/screenshots\/[^"]+)"/gu)].map(match => match[1]);
            assert.equal(shots.length, new Set(shots).size, `${product} repeats a screenshot`);
            assert.ok(shots.length >= 3, `${product} needs representative screenshots`);
            assert.match(html, /<dialog class="lightbox" id="image-lightbox"/u, `${product} enlarges screenshots in place`);
            assert.match(html, /<script src="\.\.\/site\.js"><\/script>/u);
            const screenshotTags = [...html.matchAll(/<img\b[^>]*src="\.\.\/screenshots\/[^"]+"[^>]*>/gu)].map(match => match[0]);
            for (const image of screenshotTags) assert.match(image, /width="1552" height="265"/u, `${product} must reserve the screenshot's actual dimensions`);
        }
        const controlPage = readFileSync(resolve(directory, "control/index.html"), "utf8");
        const firstControlImage = [...controlPage.matchAll(/<img\b[^>]*src="(\.\.\/screenshots\/[^"]+)"/gu)][0]?.[1];
        assert.equal(firstControlImage, "../screenshots/application-overview.png", "Control detail must open with the actual complete application view");
        assert.match(controlPage, /screenshots\/dynamics\.png/u);
        const dynamicsSection = controlPage.split('<p class="eyebrow">Dynamics</p>')[1]?.split('</article>')[0];
        assert.ok(dynamicsSection, "Control detail page needs its Dynamics section");
        assert.match(dynamicsSection, /class="product-detail-section-gallery"/u);
        assert.equal(dynamicsSection.match(/<figure class="shot">/gu)?.length, 3);
        const dynamicsImages = [...dynamicsSection.matchAll(/<img src="(\.\.\/screenshots\/[^"]+)"/gu)].map(match => match[1]);
        assert.deepEqual(dynamicsImages, ["../screenshots/dynamics.png", "../screenshots/dynamics-projection.png", "../screenshots/dynamics-phase.png"]);
        for (const image of dynamicsImages) assert.ok(dynamicsSection.includes(`href="${image}"`), `${image} needs a full-size link`);
        assert.match(dynamicsSection, /five Speed Groups/u);
        assert.match(dynamicsSection, /joins the running sync phase/u);
        assert.match(dynamicsSection, /next beat or four-beat bar/u);
        assert.match(controlPage, /screenshots\/control-media-pane\.png/u);
        const hardwareSection = controlPage.split('<p class="eyebrow">Hardware</p>')[1]?.split('</article>')[0];
        assert.ok(hardwareSection, "Control needs its hardware and software playback workflow");
        assert.match(hardwareSection, /OSC control surface/u);
        assert.match(hardwareSection, /Custom hardware bridges/u);
        assert.match(hardwareSection, /UI adapts when hardware connects/u);
        assert.match(hardwareSection, /compact playback view frees room/u);
        assert.match(hardwareSection, /no longer need at full touchscreen size/u);
        assert.match(hardwareSection, /--gallery-columns:1/u, "playback comparisons must stay full width");
        assert.deepEqual([...hardwareSection.matchAll(/<img src="(\.\.\/screenshots\/[^"]+)"/gu)].map(match => match[1]), ["../screenshots/control-playbacks-software.png", "../screenshots/control-playbacks-hardware.png"]);


        assert.doesNotMatch(controlPage, /A stage view built in/u);
        assert.match(controlPage, /lightweight plan and wireframe Stage views/u);
        assert.match(controlPage, /Use Architect for rendered lighting/u);
        assert.doesNotMatch(controlPage, /a built-in visualizer/u);
        assert.match(controlPage, /Art-Net, sACN, or supported USB DMX/u);
        assert.match(controlPage, /functions or editable keyframes/u);
        assert.match(controlPage, /Distribute them across the rig in 3D/u);
        assert.match(controlPage, /Keep your media server close/u);
        assert.ok(controlPage.indexOf("Position / PSN support") < controlPage.indexOf("Timecode and timeline editor"));
        const pixelPage = readFileSync(resolve(directory, "pixel/index.html"), "utf8");
        const pixelSections = [...pixelPage.matchAll(/<p class="eyebrow">([^<]+)<\/p>/gu)].map(match => match[1]);
        assert.deepEqual(pixelSections.slice(0, 3), ["Library", "Visualizers", "Text"]);
        assert.match(pixelPage, /Doors open in/u);
        assert.match(pixelPage, /including HDMI/u);
        assert.match(pixelPage, /DMX output and multiple outputs<\/strong><p>Public beta/u);
        const architectPage = readFileSync(resolve(directory, "architect/index.html"), "utf8");
        assert.match(architectPage, /export clear PDF plans/u);
        const architectSections = [...architectPage.matchAll(/<p class="eyebrow">([^<]+)<\/p>/gu)].map(match => match[1]);
        assert.deepEqual(architectSections.slice(0, 3), ["Visualization", "Interchange", "CAD"]);
        const firstArchitectImage = [...architectPage.matchAll(/<img\b[^>]*src="(\.\.\/screenshots\/[^"]+)"/gu)][0]?.[1];
        assert.equal(firstArchitectImage, "../screenshots/architect-media-stage-main.png");
        const visualizationSection = architectPage.split('<p class="eyebrow">Visualization</p>')[1]?.split('</article>')[0];
        assert.match(visualizationSection, /architect-renderer-white-ultra\.png/u, "preserve the original white venue capture");
        assert.match(visualizationSection, /architect-renderer-warm-ultra\.png/u, "preserve the original warm venue capture");
        assert.match(visualizationSection, /--gallery-columns:2/u, "extra venue views sit below the large main image");
        assert.match(architectPage, /Simulate your venue and rig\./u);
        assert.match(architectPage, /Build the rig\. Export the PDF plan\./u);
        const interchangeSection = architectPage.split('<p class="eyebrow">Interchange</p>')[1]?.split('</article>')[0];
        assert.ok(interchangeSection);
        assert.match(interchangeSection, /Bring the rig in\. Take the rig out\./u);
        assert.match(interchangeSection, /screenshots\/architect-show-transfer\.png/u);
        assert.doesNotMatch(interchangeSection, /architect-renderer-/u);

        assert.match(architectPage, /screenshots\/architect-patch\.png/u);
        assert.doesNotMatch(architectPage, /screenshots\/architect-elevation\.png/u);
        const lasersSection = architectPage.split('<p class="eyebrow">Lasers</p>')[1]?.split('</article>')[0];
        assert.ok(lasersSection, "Architect needs its actual laser authoring and output views");
        assert.equal(lasersSection.match(/<figure class="shot">/gu)?.length, 2);
        assert.match(lasersSection, /--gallery-columns:2/u);
        assert.match(dynamicsSection, /--gallery-columns:3/u);
        const patchSection = architectPage.split('<p class="eyebrow">Patch</p>')[1]?.split('</article>')[0];
        assert.ok(patchSection);
        assert.match(patchSection, /screenshots\/architect-patch\.png/u);
        const movingSection = architectPage.split('<p class="eyebrow">Moving scenes</p>')[1]?.split('</article>')[0];
        assert.ok(movingSection);
        assert.match(movingSection, /screenshots\/control-position-reference\.png/u);
        assert.match(movingSection, /Control’s Patch sheet for the same portable Show/u);
        assert.match(movingSection, /relative to a shared 3D point/u);

        assert.match(lasersSection, /screenshots\/architect-laser-code\.png/u);
        assert.match(lasersSection, /screenshots\/architect-laser-output\.png/u);
        assert.match(architectPage, /Mostly complete, with further improvements in progress/u);
        assert.match(architectPage, /Crowd simulation<\/strong><p>Being improved/u);
        assert.match(controlPage, /Plugin support<\/strong><p>In planning/u);
        assert.match(controlPage, /Position \/ PSN support<\/strong><p>Public beta/u);
        assert.match(architectPage, /Ultra Rendering Quality/u);
        assert.match(architectPage, /Adjust the rendering quality to suit your computer’s graphics performance/u);
        assert.doesNotMatch(architectPage, /A warm look for the venue|venue and DJ rig/u);
		const landingPage = readFileSync(target, "utf8");
        const homeCardTitles = [...landingPage.matchAll(/<figcaption><strong>([^<]+)<\/strong>/gu)].map(match => match[1]);
        assert.deepEqual(homeCardTitles, ["Dynamic Screen Layouts", "Track every change", "Desk-style Effect Engine", "Up to Eight Layers", "Library Management Built In", "Control via DMX or in the Browser", "Live 3D Visualizer", "Generate Your Venue Plans", "Patch Your Fixtures"]);
        assert.match(landingPage, /screenshots\/media-server-library\.png/u);
        assert.doesNotMatch(landingPage, /screenshots\/media-server-visualizers\.png/u);
        assert.doesNotMatch(pixelPage, /Library Management Built In/u, "homepage copy must not rewrite Pixel detail captions");

        const landingImages = [...landingPage.matchAll(/<img\b[^>]*src="screenshots\/[^"]+"[^>]*>/gu)].map(match => match[0]);
        assert.ok(landingImages.length > 3);
        for (const image of landingImages) assert.match(image, /width="1552" height="265"/u, "landing screenshots including hero images need intrinsic dimensions");
		assert.doesNotMatch(landingPage, /class="performance-compact"|class="downloads"/u);
		assert.match(landingPage, /Open source lighting control desk/u);
        assert.deepEqual([...landingPage.matchAll(/<li><span>0[1-4]<\/span><strong>([^<]+)<\/strong>/gu)].map(match => match[1]), ["Plan", "Program", "Visualize", "Perform"]);
		assert.match(landingPage, /Open source live media server/u);
		assert.match(landingPage, /Open source visualizer and show CAD/u);
		assert.match(landingPage, /screenshots\/application-overview\.png/u);
		assert.match(landingPage, /screenshots\/tracked-programming\.png/u);
		assert.match(landingPage, /screenshots\/media-server-playback\.png/u);
		assert.match(landingPage, /screenshots\/architect-media-stage-main\.png/u);
        assert.equal([...landingPage.matchAll(/src="screenshots\/architect-media-stage-main\.png"/gu)].length, 1, "new media venue render is the top hero; original white view stays in the product gallery");
        assert.match(landingPage, /screenshots\/architect-renderer-white-ultra\.png/u);
        assert.doesNotMatch(architectPage, /screenshots\/architect-renderer-ultra\.png/u, "use the new main view and keep the distinct original white/warm views");

		assert.match(landingPage, /screenshots\/architect-cad\.png/u);
		assert.doesNotMatch(landingPage, /screenshots\/media-server-dashboard\.png/u);
		assert.match(landingPage, /href="downloads\/"/u);
		assert.match(landingPage, /href="impressum\/"/u);
		assert.match(landingPage, /href="datenschutz\/"/u);
		assert.doesNotMatch(landingPage, /__[A-Z_]+__/u);
	} finally {
		rmSync(directory, { recursive: true, force: true });
	}
});

test("the frame-time histogram counts every frame and colours each band by its rate", () => {
	const measured = measuredStatus("healthy");
	const scenario = measured.benchmark_scenarios[0];
	scenario.frame_rate_band_bounds_hz = [10, 30, 38, 44, 60, 120];
	// One frame under 10 Hz, then bands rising to the fastest.
	scenario.frame_rate_band_counts = [1, 2, 4, 8, 16, 32];
	const summary = renderCompactPerformanceSummary(measured);

	assert.match(summary, /Frame time distribution across 63 frames/u);
	// Each band is drawn once, coloured by where its rates sit.
	assert.match(summary, /performance-tone-failing[^>]*--bar-height:3%/u);
	assert.match(summary, /performance-tone-spare[^>]*--bar-height:100%/u);
	// The band a bar covers and its share are readable without a legend.
	assert.match(summary, /title="10–30 Hz: 2 frames \(3\.2% of all frames\)"/u);
	// The open ends of the chart say so instead of naming a bound they do not have.
	assert.match(summary, /title="Under 10 Hz: 1 frame \(/u);
	assert.match(summary, /title="60 Hz and above: 32 frames/u);
});

test("a run without measured frame times says so instead of drawing an empty chart", () => {
	const summary = renderCompactPerformanceSummary(measuredStatus("healthy"));
	assert.match(summary, /Frame times not measured/u);
	assert.doesNotMatch(summary, /performance-histogram-bar/u);
});

test("the headline CPU number is the p95 busiest second when one was measured", () => {
	const measured = measuredStatus("healthy");
	measured.benchmark_scenarios[0].resources.application_cpu_p95_percent = 64;
	const summary = renderCompactPerformanceSummary(measured);

	assert.match(summary, /<strong>64% p95<\/strong>/u);
});

test("cadence labels distinguish desktop targets, slow-tail floor, and separate engine probe", () => {
	const page = renderPerformancePage(measuredStatus());
	assert.match(page, /Output cadence/);
	assert.match(page, /95% floor/);
	assert.match(page, /target 60 Hz/);
	assert.match(page, /Separate engine-only probe/);
	assert.match(page, /Average engine-only output cadence/);
	assert.match(page, /not directly comparable/);
	assert.match(page, /not the Fixture Sheet paint rate or 3D renderer FPS/);
	assert.doesNotMatch(page, /60 hertz output scheduler/);
});

test("legacy broken zero-slot counters are unavailable rather than claimed as a measured zero", () => {
	const status = measuredStatus();
	status.benchmark_scenarios[0].parameter_count = 0;
	assert.match(renderCompactPerformanceSummary(status), /Not measured/);
	assert.doesNotMatch(
		renderCompactPerformanceSummary(status),
		/<strong>0 DMX slots/,
	);
});

test("Stage mode and native renderer CPU remain visible instead of hidden in the total", () => {
	const status = measuredStatus();
	status.benchmark_scenarios[0].stage_enabled = false;
	status.benchmark_scenarios[0].measurement_duration_source =
		"server-monotonic";
	status.benchmark_scenarios[0].resources.native_renderer = {
		cpu_average_percent: 189,
		cpu_max_percent: 220,
		peak_resident_bytes: 800 * 1024 ** 2,
	};
	const page = renderPerformancePage(status);
	assert.match(page, /Stage: off/);
	assert.match(page, /15.04 s measured/);
	assert.match(page, /Native Stage renderer/);
	assert.match(page, /189%/);
	assert.match(page, /Older raw reports included native Stage rendering/);
});
