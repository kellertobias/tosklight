const PERFORMANCE_STATES = new Set([
	"healthy",
	"warning",
	"degraded",
	"unknown",
]);

const isObject = (value) =>
	value !== null && typeof value === "object" && !Array.isArray(value);

export const escapePerformanceText = (value) =>
	String(value)
		.replace(/&/g, "&amp;")
		.replace(/</g, "&lt;")
		.replace(/>/g, "&gt;")
		.replace(/"/g, "&quot;")
		.replace(/'/g, "&#39;");

function measuredEvidence(status) {
	const evidenceState =
		status.evidence?.kind ?? status.evidence?.state ?? status.evidence?.status;
	const baseline =
		status.evidence?.baseline ??
		status.execution?.baseline ??
		status.baseline_execution;
	return (
		evidenceState === "measured" &&
		Number.isInteger(baseline?.exit_code) &&
		typeof status.release?.commit === "string" &&
		status.release.commit.length > 0 &&
		typeof status.required_floor?.met === "boolean"
	);
}

export function unknownPerformanceStatus({
	version,
	releaseUrl,
	summary = "No valid performance result is available for this release.",
} = {}) {
	return {
		schema_version: 2,
		status: "unknown",
		summary,
		release: {
			version: version ?? null,
			commit: null,
			url: releaseUrl ?? null,
		},
		evidence: {
			kind: "unknown",
			reason: "missing_or_invalid_public_status",
		},
	};
}

export function normalizePublicPerformanceStatus(candidate, fallback = {}) {
	const unknown = unknownPerformanceStatus(fallback);
	if (
		!isObject(candidate) ||
		!PERFORMANCE_STATES.has(candidate.status) ||
		typeof candidate.summary !== "string"
	) {
		return unknown;
	}
	if (candidate.status === "unknown") {
		return candidate;
	}
	if (
		!Number.isInteger(candidate.schema_version) ||
		candidate.schema_version < 2 ||
		!measuredEvidence(candidate)
	) {
		return unknown;
	}
	return candidate;
}

const present = (value) => value !== null && value !== undefined;

function value(value, unit = "") {
	return present(value) ? `${escapePerformanceText(value)}${unit}` : "—";
}

function gate(value_) {
	return value_ == null ? "unknown" : value_ ? "pass" : "degraded";
}

function reference(value_) {
	return value_ == null ? "unknown" : value_ ? "met" : "not met";
}

function row(label, value_, unit = "") {
	return `<tr><th>${escapePerformanceText(label)}</th><td>${value(value_, unit)}</td></tr>`;
}

function bytes(value_) {
	if (!present(value_)) return null;
	const gibibytes = Number(value_) / 1024 ** 3;
	return `${gibibytes.toFixed(gibibytes >= 10 ? 1 : 2)} GiB`;
}

function compactNumber(value_) {
	if (!present(value_)) return "—";
	const numeric = Number(value_);
	if (!Number.isFinite(numeric)) return "—";
	return new Intl.NumberFormat("en-US", {
		maximumFractionDigits: Number.isInteger(numeric) ? 0 : 1,
	}).format(numeric);
}

function compactDuration(value_) {
	if (!Number.isFinite(value_)) return "—";
	return new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 }).format(
		value_,
	);
}

/**
 * The slowest second an operator lived through. A row that dipped below this is never green,
 * however comfortable its p95: the desk visibly stalled, and a good typical rate does not undo
 * a second the operator saw drop.
 */
const MINIMUM_GREEN_HZ = 40;

/**
 * How a run is graded, worst second first then typical second.
 *
 * A desk is judged by the second the operator saw drop, so every grade names a floor the slowest
 * second has to clear before the mean is even considered. A comfortable average does not undo a
 * visible stall.
 */
const CADENCE_GRADES = [
	{ status: "healthy", minimumHz: 38, meanHz: 39 },
	{ status: "warning", minimumHz: 30, meanHz: 35 },
	{ status: "caution", minimumHz: 26, meanHz: 30 },
];

function compactScenarioStatus(scenario, thresholds) {
	const worst = scenario.minimum_one_second_completed_hz;
	const mean = scenario.average_completed_hz;
	if (!Number.isFinite(worst) || !Number.isFinite(mean)) {
		// A legacy run without a measured floor is graded on what it does report.
		const cadence = scenario.p95_one_second_completed_hz;
		if (!Number.isFinite(cadence)) return "unknown";
		if (cadence < (thresholds?.red_below_hz ?? 30)) return "degraded";
		if (cadence < (thresholds?.yellow_below_hz ?? 38)) return "warning";
		return "healthy";
	}
	for (const grade of CADENCE_GRADES)
		if (worst > grade.minimumHz && mean > grade.meanHz) return grade.status;
	return "degraded";
}

/**
 * The colour a frame rate is drawn in, as an operator reads it.
 *
 * Below 30 Hz the desk is failing, to 38 Hz it is struggling, to 44 Hz it is holding, to 60 Hz it
 * is comfortable, and above that it has room to spare.
 */
const CADENCE_BANDS = [
	{ belowHz: 30, tone: "failing" },
	{ belowHz: 38, tone: "struggling" },
	{ belowHz: 44, tone: "holding" },
	{ belowHz: 60, tone: "comfortable" },
];

export function cadenceTone(frameHz) {
	if (!Number.isFinite(frameHz)) return "unknown";
	for (const band of CADENCE_BANDS) if (frameHz < band.belowHz) return band.tone;
	return "spare";
}

function compactScenarioRows(performance) {
	if (Array.isArray(performance.benchmark_scenarios)) {
		return performance.benchmark_scenarios
			.filter(
				(scenario) =>
					Number.isFinite(scenario.fixture_count) &&
					Number.isFinite(scenario.minimum_one_second_completed_hz) &&
					Number.isFinite(scenario.average_completed_hz) &&
					Number.isFinite(scenario.p95_one_second_completed_hz),
			)
			.map((scenario) => ({
				...scenario,
				compact_status: compactScenarioStatus(scenario, scenario.thresholds),
				output_target_hz: scenario.requested_rate_hz,
			}))
			.sort(
				(left, right) =>
					(left.animated_attribute_count ?? Number.MAX_SAFE_INTEGER) -
						(right.animated_attribute_count ?? Number.MAX_SAFE_INTEGER) ||
					String(left.execution_mode).localeCompare(
						String(right.execution_mode),
					),
			);
	}
	const required = performance.required_floor ?? {};
	const thresholds = {
		red_below_hz: required.yellow_threshold_hz ?? 40,
		yellow_below_hz: required.green_threshold_hz ?? 60,
	};
	return [
		performance.two_thousand_show?.attempted === true &&
			performance.two_thousand_show,
		required,
		performance.doubled_density?.attempted === true &&
			performance.doubled_density,
	]
		.filter(Boolean)
		.filter(
			(scenario) =>
				Number.isFinite(scenario.fixture_count) &&
				Number.isFinite(scenario.minimum_one_second_completed_hz) &&
				Number.isFinite(scenario.average_completed_hz) &&
				Number.isFinite(scenario.p95_one_second_completed_hz),
		)
		.map((scenario) => ({
			...scenario,
			compact_status: compactScenarioStatus(scenario, thresholds),
			output_target_hz:
				scenario.requested_rate_hz ??
				scenario.configured_rate_hz ??
				scenario.rate_hz ??
				required.rate_hz,
		}));
}

/**
 * The frame-time histogram of one run, drawn as one bar per rate band.
 *
 * Every delivered frame is counted, so the shape says how often the desk missed its budget rather
 * than only how often a whole second did. Each bar is coloured by the rate it represents.
 */
function cadenceHistogram(scenario) {
	const counts = scenario.frame_rate_band_counts ?? [];
	const bounds = scenario.frame_rate_band_bounds_hz ?? [];
	const total = counts.reduce((sum, count) => sum + count, 0);
	if (!total || counts.length !== bounds.length)
		return `<small class="performance-histogram-empty">Frame times not measured</small>`;
	const tallest = Math.max(...counts);
	const bars = counts
		.map((count, index) => {
			const upper = bounds[index];
			const lower = index === 0 ? 0 : bounds[index - 1];
			// A band is named by the rates it holds, and coloured by where those rates sit.
			const tone = cadenceTone(index === 0 ? lower : (lower + upper) / 2);
			const height = tallest ? Math.round((count / tallest) * 100) : 0;
			const share = ((count / total) * 100).toFixed(1);
			return (
				`<span class="performance-histogram-bar performance-tone-${tone}"` +
				` style="--bar-height:${height}%"` +
				` title="${compactNumber(lower)}–${compactNumber(upper)} Hz: ${compactNumber(count)} frames (${share}%)"></span>`
			);
		})
		.join("");
	return `<div class="performance-histogram" role="img" aria-label="Frame time distribution across ${compactNumber(total)} frames">${bars}</div>`;
}

function compactCpu(resources) {
	const maximum =
		resources?.application_cpu_max_percent ?? resources?.application_cpu_max;
	const average =
		resources?.application_cpu_average_percent ??
		resources?.application_cpu_average;
	if (!Number.isFinite(maximum) || !Number.isFinite(average)) {
		return `<strong>Not measured</strong><small>application CPU unavailable</small>`;
	}
	const ram =
		resources?.application_peak_resident_bytes ??
		resources?.peak_resident_bytes;
	// The headline is the p95 busiest second, which describes the load the desk actually carries
	// rather than the single worst spike a run happened to catch.
	const headline = Number.isFinite(resources?.application_cpu_p95_percent)
		? resources.application_cpu_p95_percent
		: maximum;
	const headlineLabel = Number.isFinite(resources?.application_cpu_p95_percent)
		? "p95"
		: "max";
	return (
		`<strong>${compactNumber(headline)}% ${headlineLabel}</strong>` +
		`<small>${compactNumber(average)}% avg · ${compactNumber(maximum)}% max Desk app CPU<br>${bytes(ram)} max RAM</small>`
	);
}

function compactScenarioRow(scenario) {
	const lanes = Number.isFinite(scenario.master_lane_count)
		? scenario.master_lane_count
		: scenario.dynamic_definition_count;
	const dynamics = Number.isFinite(scenario.animated_attribute_count)
		? `${compactNumber(scenario.animated_attribute_count)} dyn. attr.`
		: Number.isFinite(scenario.dynamic_definition_count)
			? `${compactNumber(scenario.dynamic_definition_count)} dyn.`
			: "Not measured";
	const legacyAttributes = Array.isArray(scenario.dynamic_lane_attributes)
		? scenario.dynamic_lane_attributes.length
		: null;
	const dynamicsDetail =
		Number.isFinite(scenario.animated_attribute_count) && Number.isFinite(lanes)
			? `${compactNumber(lanes)} master lane${lanes === 1 ? "" : "s"}`
			: Number.isFinite(legacyAttributes)
				? `${compactNumber(legacyAttributes)} Dynamic lane attribute${legacyAttributes === 1 ? "" : "s"}`
				: "Dynamic workload unavailable";
	const maximum = compactNumber(scenario.maximum_one_second_completed_hz);
	const elapsed = scenario.measurement_seconds;
	const parameters = Number.isFinite(scenario.parameter_count)
		? `${compactNumber(scenario.parameter_count)} param.`
		: "Not measured";
	const critical =
		scenario.thresholds?.critical_below_hz != null &&
		scenario.p95_one_second_completed_hz < scenario.thresholds.critical_below_hz
			? `<small>Critical: below ${compactNumber(scenario.thresholds.critical_below_hz)} Hz</small>`
			: "";
	const mode =
		scenario.execution_mode === "one_core"
			? "locked to 1 core"
			: "unrestricted";
	return (
		`<tr class="performance-row performance-row-${escapePerformanceText(scenario.compact_status)}">` +
		`<th scope="row"><strong>${escapePerformanceText(scenario.case_name ?? `${compactNumber(scenario.fixture_count)} fixtures`)}</strong>` +
		`<small>${mode}<br>${compactNumber(scenario.universes)} DMX universes</small></th>` +
		`<td><strong>${parameters}</strong><small>${dynamics}<br>${escapePerformanceText(dynamicsDetail)}</small></td>` +
		`<td class="performance-tone-${cadenceTone(scenario.p95_one_second_completed_hz)}">` +
		`<strong>${compactNumber(scenario.p95_one_second_completed_hz)} Hz p95</strong>` +
		`<small>${compactNumber(scenario.minimum_one_second_completed_hz)} / ${compactNumber(scenario.average_completed_hz)} / ${maximum} Hz<br>min / avg / max</small>${critical}</td>` +
		`<td>${cadenceHistogram(scenario)}<small>over ${compactDuration(elapsed)} s</small></td>` +
		`<td>${compactCpu(scenario.resources)}</td></tr>`
	);
}

function scenarioTable(performance) {
	const scenarios = compactScenarioRows(performance);
	const rows = scenarios.map(compactScenarioRow).join("");
	return {
		scenarios,
		html: scenarios.length
			? `<div class="performance-table-scroll"><table class="performance-compact"><thead><tr>` +
				`<th>Test set</th><th>Load</th><th>Frame rate</th><th>Frame times</th><th>CPU and RAM</th>` +
				`</tr></thead><tbody>${rows}</tbody></table></div>`
			: `<p class="performance-empty">No measured output-cadence run is available for this release.</p>`,
	};
}

function componentResourceRows(scenarios) {
	return scenarios
		.flatMap((scenario) => {
			const resources = scenario.resources ?? {};
			const application = {
				cpu_average_percent: resources.application_cpu_average_percent,
				cpu_max_percent: resources.application_cpu_max_percent,
				peak_resident_bytes: resources.application_peak_resident_bytes,
			};
			return [
				["Complete Desk app tree", application],
				["Bundled Light server", resources.server],
				["Tauri host + WebKit", resources.desktop_webview],
			]
				.filter(([, component]) =>
					Number.isFinite(component?.cpu_average_percent),
				)
				.map(
					([componentName, component]) =>
						`<tr><th>${escapePerformanceText(scenario.case_name)}</th>` +
						`<td>${scenario.execution_mode === "one_core" ? "locked to 1 core" : "unrestricted"}</td>` +
						`<td>${escapePerformanceText(componentName)}</td>` +
						`<td>${compactNumber(component.cpu_average_percent)}%</td>` +
						`<td>${compactNumber(component.cpu_max_percent)}%</td>` +
						`<td>${bytes(component.peak_resident_bytes) ?? "—"}</td></tr>`,
				);
		})
		.join("");
}

function renderTestSetup(scenarios) {
	const desktopMeasurement = scenarios.some(
		(scenario) =>
			scenario.measurement_surface === "released-tauri-desk-fixture-sheet",
	);
	if (!desktopMeasurement) {
		return `<p>These legacy results were produced by the released Linux engine benchmark. The next publication uses the released Desk application setup shown on the main page.</p>`;
	}
	return (
		`<p>Each row is a 60-second timed run of the released Linux Desk bundle. Every show runs unrestricted; the demo show runs a second time with the complete application tree locked to one CPU core, which is the case that says whether a modest machine still carries a real show. The output scheduler requests 40 Hz throughout, so the rows compare directly. The frame rate column reports the p95 slowest second beside the slowest, mean, and fastest; the histogram counts every delivered frame by the rate it was delivered at.</p>` +
		`<figure class="test-setup"><div class="test-flow" role="img" aria-label="Released Linux bundle starts the Tauri Desk under Xvfb. Its real WebKit Fixture Sheet connects to the bundled Light server, which runs the 60 hertz output scheduler.">` +
		`<div><strong>Released Linux bundle</strong><small>exact published AppImage</small></div><span aria-hidden="true">→</span>` +
		`<div><strong>Tauri Desk under Xvfb</strong><small>unrestricted, and one core for the demo show</small></div><span aria-hidden="true">→</span>` +
		`<div><strong>WebKit Fixture Sheet</strong><small>real production UI, kept active</small></div><span aria-hidden="true">→</span>` +
		`<div><strong>Bundled Light server</strong><small>show, Dynamics, and DMX output</small></div><span aria-hidden="true">→</span>` +
		`<div><strong>40 Hz scheduler</strong><small>cadence diagnostics</small></div></div>` +
		`<figcaption>The API coordinator loads each real show and confirms the Fixture Sheet remains responsive. Linux process sampling measures the Desk application tree and separates its Light server from the Tauri/WebKit processes. Node and Xvfb are excluded. Playwright is not launched.</figcaption></figure>`
	);
}

function renderDetailedScenarioEvidence(performance) {
	const { scenarios, html } = scenarioTable(performance);
	const componentRows = componentResourceRows(scenarios);
	const components = componentRows
		? `<h3>Application process breakdown</h3><div class="table-scroll"><table><thead><tr><th>Test set</th><th>Mode</th><th>Process scope</th><th>Average CPU</th><th>Maximum CPU</th><th>Maximum RAM</th></tr></thead><tbody>${componentRows}</tbody></table></div>`
		: "";
	return (
		`${html}<p class="performance-legend">This is the same measured-scenario table shown on the main GitHub Pages page. A row is graded by its slowest second first and then its mean; every run requests 40 Hz.</p>` +
		`<h2>How the test runs</h2>${renderTestSetup(scenarios)}${components}`
	);
}

export function renderCompactPerformanceSummary(performance) {
	const { html } = scenarioTable(performance);
	return (
		`<div class="performance-summary">${html}` +
		`<p class="performance-legend">A row is graded by its slowest second first, then its mean. <span class="legend-healthy">Green</span>: slowest above 38 Hz and mean above 39 Hz · ` +
		`<span class="legend-warning">Yellow</span>: slowest above 30 Hz and mean above 35 Hz · ` +
		`<span class="legend-caution">Orange</span>: slowest above 26 Hz and mean above 30 Hz · ` +
		`<span class="legend-degraded">Red</span>: below that · ` +
		`<span class="legend-unknown">Gray</span>: incomplete evidence. Every run requests 40 Hz.</p>` +
		`<p class="performance-legend">CPU and RAM cover the released Desk application tree during the timed window: the Tauri host, its real WebKit Fixture Sheet, and the bundled Light server. The detailed report separates the server and desktop/WebView processes. Node, Xvfb, and Playwright are excluded; Playwright is not launched. CPU is expressed per logical core and bounded by the application affinity; 100% means one fully used core.</p>` +
		`<p class="performance-details"><a href="performance/">Detailed tests, run information, and raw report →</a></p></div>`
	);
}

function safePublicUrl(value_) {
	if (typeof value_ !== "string") return null;
	try {
		const parsed = new URL(value_);
		return parsed.protocol === "https:" || parsed.protocol === "http:"
			? parsed.href
			: null;
	} catch {
		return null;
	}
}

export function performanceReportUrl(performance) {
	const explicit = safePublicUrl(
		performance.report_url ??
			performance.direct_report_url ??
			performance.release?.report_url,
	);
	if (explicit) return explicit;
	const releaseUrl = safePublicUrl(performance.release?.url);
	if (!releaseUrl) return null;
	const parsed = new URL(releaseUrl);
	const tagMarker = "/releases/tag/";
	const marker = parsed.pathname.indexOf(tagMarker);
	if (marker < 0) return null;
	const tag = parsed.pathname.slice(marker + tagMarker.length);
	if (!tag) return null;
	parsed.pathname =
		`${parsed.pathname.slice(0, marker)}/releases/download/${tag}/` +
		"report-performance.zip";
	parsed.search = "";
	parsed.hash = "";
	return parsed.href;
}

function workloadLabel(workload) {
	if (typeof workload === "string") return workload;
	if (!isObject(workload)) return null;
	const parts = [
		workload.label ?? workload.benchmark,
		workload.profile,
		workload.tier ?? workload.workload_tier,
	].filter(present);
	return parts.length > 0 ? parts.join(" · ") : null;
}

function renderPatchRows(patchServer) {
	const metric = (scenario, label) =>
		`<tr><th>${escapePerformanceText(label)}</th>` +
		`<td>${value(scenario?.p50_microseconds, " µs")}</td>` +
		`<td>${value(scenario?.p95_microseconds, " µs")}</td>` +
		`<td>${value(scenario?.gate_p95_microseconds, " µs")}</td>` +
		`<td>${gate(scenario?.gate_met)}</td></tr>`;
	return (
		metric(patchServer?.single_fixture, "1 fixture") +
		metric(patchServer?.hundred_fixtures, "100 fixtures")
	);
}

function renderDoubledDensity(doubled) {
	if (!doubled?.attempted) {
		return (
			`<p><strong>Not attempted.</strong> ` +
			`${escapePerformanceText(doubled?.reason ?? doubled?.not_attempted_reason ?? "No reason was recorded.")}</p>`
		);
	}
	const deadline = doubled.deadline ?? {};
	const reason = present(doubled.reason)
		? `<p>${escapePerformanceText(doubled.reason)}</p>`
		: "";
	return (
		`<table><tbody>` +
		row("Fixtures", doubled.fixture_count) +
		row("Fixtures per universe", doubled.fixtures_per_universe) +
		row(
			"Requested output cadence",
			doubled.requested_rate_hz ??
				doubled.configured_rate_hz ??
				doubled.rate_hz,
			" Hz",
		) +
		row(
			"Achieved output cadence",
			doubled.achieved_rate_hz ?? doubled.achieved_ticks_per_second,
			" Hz",
		) +
		row(
			"Minimum one-second cadence",
			doubled.minimum_one_second_completed_hz,
			" Hz",
		) +
		row(
			"Deadline misses",
			deadline.deadline_misses ?? doubled.deadline_misses,
		) +
		row("Dropped ticks", deadline.dropped_ticks ?? doubled.dropped_ticks) +
		row("Deferred ticks", deadline.deferred_ticks ?? doubled.deferred_ticks) +
		row(
			"100 Hz reference (diagnostic)",
			reference(doubled.configured_target_met),
		) +
		row("Limiting measured phase", doubled.limiting_phase?.name) +
		row("Limiting phase p95", doubled.limiting_phase?.p95_microseconds, " µs") +
		`</tbody></table>` +
		reason +
		`<p>This 2,048-fixture capacity probe is diagnostic only and never changes the release indicator.</p>`
	);
}

function renderCanonicalDemo(canonical) {
	if (canonical?.reason || canonical?.attempted !== true) {
		return `<p>${escapePerformanceText(canonical?.reason ?? "No current-release canonical demo measurement is available.")}</p>`;
	}
	return (
		`<p>This is the exact product-demo show with 231 controllable lighting fixtures, 33 visual-only Venue records, 264 total patch records, 306 physical Stage instances, and the 3D Stage visible. CI measures it in Chromium; the separate packaged Tauri acceptance remains authoritative for WebView behavior.</p><table><tbody>` +
		row("Controllable fixture records", canonical.scene?.fixture_records) +
		row("Physical Stage instances", canonical.scene?.physical_instances) +
		row("Measurement duration", canonical.window?.elapsed_ms, " ms") +
		row(
			"Stage presentation cadence",
			canonical.stage?.presentation_rate_hz,
			" Hz",
		) +
		row(
			"Stage source-to-canvas p95",
			canonical.stage?.source_to_settled_canvas_ms?.p95,
			" ms",
		) +
		row(
			"Stage render-duration p95",
			canonical.stage?.render_duration_ms?.p95,
			" ms",
		) +
		row("Maximum draw calls", canonical.stage?.max_draw_calls) +
		row("Maximum triangles", canonical.stage?.max_triangles) +
		`</tbody></table>`
	);
}

export function renderPerformancePage(performance) {
	const required = performance.required_floor ?? {};
	const frameRate = required.frame_rate ?? {};
	const deadline = required.deadline ?? {};
	const mutation = performance.show_mutation ?? performance.mutation ?? {};
	const patchServer = performance.patch?.server;
	const doubled = performance.doubled_density ?? {};
	const canonical = performance.canonical_demo ?? {};
	const runner =
		performance.runner?.label ??
		performance.runner?.hardware_label ??
		performance.evidence?.runner?.label ??
		performance.evidence?.runner?.hardware_label;
	const failedGates =
		performance.failed_gates ??
		performance.evidence?.failed_gates ??
		performance.evidence?.failed_gate_ids ??
		[];
	const reportUrl = performanceReportUrl(performance);
	const reportLink = reportUrl
		? `<p><a href="${escapePerformanceText(reportUrl)}">Download the detailed benchmark report →</a></p>`
		: "<p>The detailed benchmark report is unavailable.</p>";

	return (
		`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">` +
		`<title>ToskLight release performance</title><style>body{font:16px system-ui;max-width:1500px;margin:3rem auto;padding:0 1rem;background:#101318;color:#eef2f6}` +
		`a{color:#72c7ff}.table-scroll,.performance-table-scroll{overflow-x:auto}table{border-collapse:collapse;width:100%;margin:1rem 0 2rem}th,td{border:1px solid #44505c;padding:.7rem;text-align:left;vertical-align:top}.performance-compact td strong,.performance-compact th strong{display:block}.performance-compact td small,.performance-compact th small{display:block;margin-top:.25rem;color:#aab4bf;font-size:.72rem;line-height:1.3}.performance-row-healthy{background:#123326}.performance-row-warning{background:#3a3014}.performance-row-caution{background:#3a2414}.performance-row-degraded{background:#3b1d20}.performance-row-unknown{background:#252b32}.performance-legend{color:#c5ced8}` +
		`.performance-histogram{display:flex;align-items:flex-end;gap:1px;height:54px;min-width:220px;padding:2px;border:1px solid #44505c;border-radius:3px;background:#181d23}` +
		`.performance-histogram-bar{flex:1;min-width:2px;height:var(--bar-height,0%);min-height:1px;border-radius:1px 1px 0 0;background:#6b7784}` +
		`.performance-histogram-empty{color:#9aa5b8}` +
		`.performance-tone-failing{--tone:#ff6b6b}.performance-tone-struggling{--tone:#ff9f45}.performance-tone-holding{--tone:#ffd166}.performance-tone-comfortable{--tone:#39d98a}.performance-tone-spare{--tone:#72c7ff}` +
		`.performance-histogram-bar[class*="performance-tone-"]{background:var(--tone)}` +
		`td[class*="performance-tone-"] strong{color:var(--tone)}` +
		`.test-setup{margin:1.25rem 0 2rem}.test-flow{display:grid;grid-template-columns:repeat(9,auto);align-items:stretch;gap:.6rem}.test-flow div{border:1px solid #526170;border-radius:.5rem;padding:.8rem;background:#1a2027;min-width:0}.test-flow strong,.test-flow small{display:block}.test-flow small,.test-setup figcaption{color:#aab4bf}.test-flow span{align-self:center;color:#72c7ff;font-size:1.4rem}.test-setup figcaption{margin-top:.8rem;line-height:1.5}@media(max-width:900px){.test-flow{display:flex;flex-direction:column}.test-flow span{transform:rotate(90deg);align-self:flex-start;margin-left:1rem}}` +
		`code{background:#20262d;padding:.15rem .35rem}.healthy{color:#39d98a}.warning{color:#ffd166}.caution{color:#ff9f45}.degraded{color:#ff6b6b}.unknown{color:#9aa5b8}</style><main><p><a href="../">← ToskLight</a></p>` +
		`<h1>Release performance</h1><p><strong class="${escapePerformanceText(performance.status)}">${escapePerformanceText(performance.status.toUpperCase())}</strong> — ${escapePerformanceText(performance.summary)}</p>` +
		`<h2>Evidence</h2><table><tbody>` +
		row("Release version", performance.release?.version) +
		row("Tested commit", performance.release?.commit) +
		row("Generated", performance.generated_at) +
		row("Runner", runner) +
		row("Workload", workloadLabel(performance.workload)) +
		`</tbody></table>` +
		`<h2>Measured shows</h2>${renderDetailedScenarioEvidence(performance)}` +
		`<h2>Runner configuration</h2><table><tbody>` +
		row("CPU", performance.runner?.cpu_model) +
		row("Logical cores", performance.runner?.logical_cpus) +
		row("RAM", bytes(performance.runner?.total_memory_bytes)) +
		row("Operating system", performance.runner?.operating_system) +
		row("Architecture", performance.runner?.architecture) +
		row("ToskLight release", performance.release?.version) +
		row("Benchmark package version", performance.runner?.package_version) +
		row("Rust toolchain", performance.runner?.rustc_version) +
		row("Build profile", performance.runner?.build_profile) +
		`</tbody></table>` +
		`<h2>Canonical 306-instance demo show</h2>${renderCanonicalDemo(canonical)}` +
		`<h2>1,024-fixture released-engine workload</h2><p>Green means at least ${value(required.green_threshold_hz, " Hz")}; yellow means at least ${value(required.yellow_threshold_hz, " Hz")}; below that is red. This released Linux probe measures engine rendering and output encoding without opening a UI. Separate packaged acceptance keeps the 306-instance demo at 100 Hz and proves the exact 1,000-instance show with Stage and Fixture Sheet open while the rest of the desk remains responsive.</p><table><tbody>` +
		row("Fixtures", required.fixture_count) +
		row("Universes", required.universes) +
		row("Fixtures per universe", required.fixtures_per_universe) +
		row(
			"Requested output cadence",
			required.requested_rate_hz ??
				required.configured_rate_hz ??
				required.rate_hz,
			" Hz",
		) +
		row(
			"Achieved output cadence",
			required.achieved_rate_hz ?? required.achieved_ticks_per_second,
			" Hz",
		) +
		row(
			"Average completed cadence",
			frameRate.average_completed_hz ?? required.average_completed_hz,
			" Hz",
		) +
		row(
			"Minimum one-second cadence",
			frameRate.minimum_one_second_completed_hz ??
				required.minimum_one_second_completed_hz,
			" Hz",
		) +
		row("Windows below minimum", frameRate.windows_below_minimum) +
		row(
			"Deadline misses",
			deadline.deadline_misses ?? required.deadline_misses,
		) +
		row("Dropped ticks", deadline.dropped_ticks ?? required.dropped_ticks) +
		row("Deferred ticks", deadline.deferred_ticks ?? required.deferred_ticks) +
		row("Acceptance tier", performance.status) +
		row(
			"Configured 100 Hz reference",
			reference(required.configured_target_met),
		) +
		row("Limiting measured phase", required.limiting_phase?.name) +
		row(
			"Limiting phase p95",
			required.limiting_phase?.p95_microseconds,
			" µs",
		) +
		`</tbody></table><p>— means the metric was unavailable in the normalized evidence.</p>` +
		`<h2>Large-show mutation</h2><table><thead><tr><th>Fixture batch</th><th>p50</th><th>p95</th></tr></thead><tbody>` +
		`<tr><th>${value(mutation.small?.fixture_count ?? mutation.small_fixture_count, " fixtures")}</th><td>${value(mutation.small?.p50_microseconds, " µs")}</td><td>${value(mutation.small?.p95_microseconds, " µs")}</td></tr>` +
		`<tr><th>${value(mutation.large?.fixture_count ?? mutation.large_fixture_count, " fixtures")}</th><td>${value(mutation.large?.p50_microseconds, " µs")}</td><td>${value(mutation.large?.p95_microseconds, " µs")}</td></tr>` +
		`</tbody></table><p>Gate: <strong>${gate(mutation.gate_met ?? performance.mutation_gate_met)}</strong></p>` +
		`<h2>Persisted Patch transaction</h2><table><thead><tr><th>Batch</th><th>p50</th><th>p95</th><th>p95 budget</th><th>Gate</th></tr></thead><tbody>` +
		renderPatchRows(patchServer) +
		`</tbody></table><p>UI action-to-visible latency is informational and is not yet a release gate.</p>` +
		`<h2>2,048-fixture limit diagnostic</h2>${renderDoubledDensity(doubled)}` +
		`<h2>Failed or unavailable gates</h2><p>${failedGates.length > 0 ? failedGates.map(escapePerformanceText).join(", ") : "None reported."}</p>` +
		reportLink +
		`</main></html>`
	);
}
