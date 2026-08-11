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

function summaryMetric(value_, unit = "", context = "measured") {
	const actual = present(value_)
		? `${escapePerformanceText(value_)}${unit}`
		: "Not measured";
	const detail = present(value_) ? context : "not collected by this benchmark";
	return `<td><strong>${actual}</strong><small>${escapePerformanceText(detail)}</small></td>`;
}

function rateContext(workload) {
	const yellow = workload?.yellow_threshold_hz;
	const green = workload?.green_threshold_hz;
	return present(yellow) && present(green)
		? `critical: x < ${yellow} Hz; warning: ${yellow} Hz ≤ x < ${green} Hz; healthy: x ≥ ${green} Hz`
		: "informational; no acceptance range configured";
}

function bytes(value_) {
	if (!present(value_)) return null;
	const gibibytes = Number(value_) / 1024 ** 3;
	return `${gibibytes.toFixed(gibibytes >= 10 ? 1 : 2)} GiB`;
}

function summaryRow({
	name,
	fixtures,
	parameters,
	universes,
	workload,
	rate,
	resources,
}) {
	const context = rateContext(workload);
	return (
		`<tr><th>${escapePerformanceText(name)}</th>` +
		summaryMetric(fixtures, "", "fixture records") +
		summaryMetric(parameters, "", "controllable parameter slots") +
		summaryMetric(universes, "", "logical output universes") +
		summaryMetric(rate?.minimum, " Hz", context) +
		summaryMetric(rate?.average, " Hz", context) +
		summaryMetric(rate?.p95, " Hz", context) +
		summaryMetric(rate?.outliers, "", "one-second windows below the minimum") +
		summaryMetric(resources?.application_cpu_max, "%") +
		summaryMetric(resources?.application_cpu_average, "%") +
		summaryMetric(resources?.system_cpu_max, "%") +
		summaryMetric(resources?.system_cpu_average, "%") +
		summaryMetric(
			bytes(resources?.peak_resident_bytes),
			"",
			"peak application resident memory",
		) +
		`</tr>`
	);
}

function renderStatisticsSummary(performance) {
	const canonical = performance.canonical_demo ?? {};
	const twoThousand = performance.two_thousand_show ?? {};
	const required = performance.required_floor ?? {};
	const doubled = performance.doubled_density ?? {};
	const unavailable = (name, fixtures) => summaryRow({ name, fixtures });
	return (
		`<div class="table-scroll"><table class="statistics"><thead><tr>` +
		`<th>Case / show</th><th>Fixtures</th><th>Parameters</th><th>Universes</th>` +
		`<th>Minimum rate</th><th>Average rate</th><th>P95 rate</th><th>Outliers</th>` +
		`<th>Application CPU max</th><th>Application CPU average</th>` +
		`<th>General CPU max</th><th>General CPU average</th><th>Maximum RAM</th>` +
		`</tr></thead><tbody>` +
		summaryRow({
			name: "Product demo (~300 fixtures)",
			fixtures: canonical.scene?.fixture_records ?? 295,
			rate: { average: canonical.stage?.presentation_rate_hz },
		}) +
		unavailable("100-fixture show", 100) +
		summaryRow({
			name: "2,000-fixture mixed shipped-mode show",
			fixtures: twoThousand.fixture_count ?? 2_000,
			parameters: twoThousand.parameter_count,
			universes: twoThousand.universes,
			rate: {
				minimum: twoThousand.minimum_one_second_completed_hz,
				average: twoThousand.average_completed_hz,
				p95: twoThousand.p95_one_second_completed_hz,
				outliers: twoThousand.windows_below_minimum,
			},
			resources: twoThousand.resources,
		}) +
		summaryRow({
			name: "1,024-fixture released-engine workload",
			fixtures: required.fixture_count,
			parameters: required.parameter_count,
			universes: required.universes,
			workload: required,
			rate: {
				minimum: required.minimum_one_second_completed_hz,
				average: required.average_completed_hz,
				p95: required.p95_one_second_completed_hz,
				outliers: required.windows_below_minimum,
			},
			resources: required.resources,
		}) +
		summaryRow({
			name: "2,048-fixture capacity diagnostic",
			fixtures: doubled.fixture_count,
			parameters: doubled.parameter_count,
			universes: doubled.universes,
			workload: required,
			rate: {
				minimum: doubled.minimum_one_second_completed_hz,
				average: doubled.average_completed_hz,
				p95: doubled.p95_one_second_completed_hz,
				outliers: doubled.windows_below_minimum,
			},
			resources: doubled.resources,
		}) +
		`</tbody></table></div><p><small>The exact 100-fixture sustained show case is listed explicitly but is not yet collected. The 2,000-fixture mixed shipped-mode workload and 2,048-fixture synthetic capacity probe are different measurements and are reported separately.</small></p>`
	);
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
	return new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 }).format(value_);
}

function compactScenarioStatus(scenario, thresholds) {
	const cadence = scenario.p95_one_second_completed_hz;
	if (!Number.isFinite(cadence)) return "unknown";
	if (cadence < thresholds.red_below_hz) return "degraded";
	if (cadence < thresholds.yellow_below_hz) return "warning";
	return "healthy";
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
					String(left.execution_mode).localeCompare(String(right.execution_mode)),
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
		resources?.application_peak_resident_bytes ?? resources?.peak_resident_bytes;
	return (
		`<strong>${compactNumber(maximum)}% max</strong>` +
		`<small>${compactNumber(average)}% avg Light CPU<br>${bytes(ram)} max RAM</small>`
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
	const dynamicsDetail = Number.isFinite(scenario.animated_attribute_count) && Number.isFinite(lanes)
		? `${compactNumber(lanes)} master lane${lanes === 1 ? "" : "s"}`
		: Number.isFinite(legacyAttributes)
			? `${compactNumber(legacyAttributes)} Dynamic lane attribute${legacyAttributes === 1 ? "" : "s"}`
			: "Dynamic workload unavailable";
	const maximum = compactNumber(scenario.maximum_one_second_completed_hz);
	const target = compactNumber(scenario.below_target_hz ?? 44);
	const below = scenario.windows_below_target ?? scenario.windows_below_minimum;
	const elapsed = scenario.measurement_seconds;
	const critical =
		scenario.thresholds?.critical_below_hz != null &&
		scenario.p95_one_second_completed_hz < scenario.thresholds.critical_below_hz
			? `<small>Critical: below ${compactNumber(scenario.thresholds.critical_below_hz)} Hz</small>`
			: "";
	const mode = scenario.execution_mode === "one_core" ? "locked to 1 core" : "unrestricted";
	return (
		`<tr class="performance-row performance-row-${escapePerformanceText(scenario.compact_status)}">` +
		`<th scope="row"><strong>${escapePerformanceText(scenario.case_name ?? `${compactNumber(scenario.fixture_count)} fixtures`)}</strong>` +
		`<small>${mode}<br>${compactNumber(scenario.universes)} DMX universes</small></th>` +
		`<td><strong>${dynamics}</strong><small>${escapePerformanceText(dynamicsDetail)}</small></td>` +
		`<td><strong>${compactNumber(scenario.p95_one_second_completed_hz)} Hz p95</strong>` +
		`<small>${compactNumber(scenario.minimum_one_second_completed_hz)} / ${compactNumber(scenario.average_completed_hz)} / ${maximum} Hz<br>min / avg / max</small>${critical}</td>` +
		`<td><strong>${compactNumber(below)} / ${compactDuration(elapsed)} s</strong>` +
		`<small>one-second windows<br>below ${target} Hz<br>total test time</small></td>` +
		`<td>${compactCpu(scenario.resources)}</td></tr>`
	);
}

export function renderCompactPerformanceSummary(performance) {
	const scenarios = compactScenarioRows(performance);
	const rows = scenarios.map(compactScenarioRow).join("");
	const table = scenarios.length
		? `<div class="performance-table-scroll"><table class="performance-compact"><thead><tr>` +
			`<th>Test set</th><th>Load</th><th>Statistics</th><th>Below target</th><th>CPU</th>` +
			`</tr></thead><tbody>${rows}</tbody></table></div>`
		: `<p class="performance-empty">No measured output-cadence run is available for this release.</p>`;
	return (
		`<div class="performance-summary">${table}` +
		`<p class="performance-legend">Rows use the scenario-specific p95 thresholds. <span class="legend-healthy">Green</span>: target met · ` +
		`<span class="legend-warning">Yellow</span>: warning · ` +
		`<span class="legend-degraded">Red</span>: below the scenario floor · ` +
		`<span class="legend-unknown">Gray</span>: incomplete evidence. Every run requests 60 Hz; “below target” always counts one-second windows below 44 Hz.</p>` +
		`<p class="performance-legend">CPU and RAM cover only the Light benchmark process (engine, rendering, and protocol pipeline) during the timed window—not the Node coordinator, browser, Desk UI, or Playwright. This workflow does not launch Playwright. CPU is expressed per logical core and bounded by the process CPU affinity; 100% means one fully used core.</p>` +
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
		`a{color:#72c7ff}.table-scroll{overflow-x:auto}table{border-collapse:collapse;width:100%;margin:1rem 0 2rem}th,td{border:1px solid #44505c;padding:.7rem;text-align:left;vertical-align:top}.statistics{min-width:1500px}.statistics th{white-space:nowrap}.statistics td strong{display:block}.statistics td small{display:block;margin-top:.25rem;color:#aab4bf;font-size:.7rem;line-height:1.2}` +
		`code{background:#20262d;padding:.15rem .35rem}.healthy{color:#39d98a}.warning{color:#ffd166}.degraded{color:#ff6b6b}.unknown{color:#9aa5b8}</style><main><p><a href="../">← ToskLight</a></p>` +
		`<h1>Release performance</h1><p><strong class="${escapePerformanceText(performance.status)}">${escapePerformanceText(performance.status.toUpperCase())}</strong> — ${escapePerformanceText(performance.summary)}</p>` +
		`<h2>Evidence</h2><table><tbody>` +
		row("Release version", performance.release?.version) +
		row("Tested commit", performance.release?.commit) +
		row("Generated", performance.generated_at) +
		row("Runner", runner) +
		row("Workload", workloadLabel(performance.workload)) +
		`</tbody></table>` +
		`<h2>Show statistics</h2>${renderStatisticsSummary(performance)}` +
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
