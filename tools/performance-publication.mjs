const PERFORMANCE_STATES = new Set(["healthy", "degraded", "unknown"]);

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

function row(label, value_, unit = "") {
	return `<tr><th>${escapePerformanceText(label)}</th><td>${value(value_, unit)}</td></tr>`;
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
	const version = performance.release?.version;
	if (!releaseUrl || typeof version !== "string" || version.length === 0)
		return null;
	const parsed = new URL(releaseUrl);
	const tagMarker = "/releases/tag/";
	const marker = parsed.pathname.indexOf(tagMarker);
	if (marker < 0) return null;
	const tag = parsed.pathname.slice(marker + tagMarker.length);
	if (!tag) return null;
	parsed.pathname =
		`${parsed.pathname.slice(0, marker)}/releases/download/${tag}/` +
		`tosklight-performance-report-${version}.zip`;
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
		row("Gate", gate(doubled.met)) +
		`</tbody></table>` +
		reason
	);
}

export function renderPerformancePage(performance) {
	const required = performance.required_floor ?? {};
	const frameRate = required.frame_rate ?? {};
	const deadline = required.deadline ?? {};
	const mutation = performance.show_mutation ?? performance.mutation ?? {};
	const patchServer = performance.patch?.server;
	const doubled = performance.doubled_density ?? {};
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
		`<title>ToskLight release performance</title><style>body{font:16px system-ui;max-width:960px;margin:3rem auto;padding:0 1rem;background:#101318;color:#eef2f6}` +
		`a{color:#72c7ff}table{border-collapse:collapse;width:100%;margin:1rem 0 2rem}th,td{border:1px solid #44505c;padding:.7rem;text-align:left}` +
		`code{background:#20262d;padding:.15rem .35rem}</style><main><p><a href="../">← ToskLight</a></p>` +
		`<h1>Release performance</h1><p><strong>${escapePerformanceText(performance.status.toUpperCase())}</strong> — ${escapePerformanceText(performance.summary)}</p>` +
		`<h2>Evidence</h2><table><tbody>` +
		row("Release version", performance.release?.version) +
		row("Tested commit", performance.release?.commit) +
		row("Generated", performance.generated_at) +
		row("Runner", runner) +
		row("Workload", workloadLabel(performance.workload)) +
		`</tbody></table>` +
		`<h2>Required output floor</h2><table><tbody>` +
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
		row("Gate", gate(required.met)) +
		`</tbody></table><p>— means the metric was unavailable in the normalized evidence.</p>` +
		`<h2>Large-show mutation</h2><table><thead><tr><th>Fixture batch</th><th>p50</th><th>p95</th></tr></thead><tbody>` +
		`<tr><th>${value(mutation.small?.fixture_count ?? mutation.small_fixture_count, " fixtures")}</th><td>${value(mutation.small?.p50_microseconds, " µs")}</td><td>${value(mutation.small?.p95_microseconds, " µs")}</td></tr>` +
		`<tr><th>${value(mutation.large?.fixture_count ?? mutation.large_fixture_count, " fixtures")}</th><td>${value(mutation.large?.p50_microseconds, " µs")}</td><td>${value(mutation.large?.p95_microseconds, " µs")}</td></tr>` +
		`</tbody></table><p>Gate: <strong>${gate(mutation.gate_met ?? performance.mutation_gate_met)}</strong></p>` +
		`<h2>Persisted Patch transaction</h2><table><thead><tr><th>Batch</th><th>p50</th><th>p95</th><th>p95 budget</th><th>Gate</th></tr></thead><tbody>` +
		renderPatchRows(patchServer) +
		`</tbody></table><p>UI action-to-visible latency is informational and is not yet a release gate.</p>` +
		`<h2>Doubled-density probe</h2>${renderDoubledDensity(doubled)}` +
		`<h2>Failed or unavailable gates</h2><p>${failedGates.length > 0 ? failedGates.map(escapePerformanceText).join(", ") : "None reported."}</p>` +
		reportLink +
		`</main></html>`
	);
}
