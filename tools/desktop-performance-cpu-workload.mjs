function require_(condition, message) {
	if (!condition) throw new Error(`Typed CPU diagnostic STOP: ${message}`);
}

function uint(value, maximum = Number.MAX_SAFE_INTEGER) {
	require_(
		Number.isSafeInteger(value) && value >= 0 && value <= maximum,
		"invalid unsigned numeric field",
	);
	return value;
}

function scaled(value, factor = 1000000) {
	if (value === undefined || value === null) return null;

	require_(
		typeof value === "number" && Number.isFinite(value) && value >= 0,
		"invalid workload measurement",
	);
	return uint(Math.floor(value * factor));
}

// Failure categories: 0 none, 1 prior failure, 2 partial capture only, 3 prior failure + partial capture.
// Capture stage 5: server drain/parse; 6: server stream validated (native remains unavailable).
// Workload phases: 0 unavailable, 1 backend, 2 prepare, 3 Sheet ready, 4 measure, 5 live, 6 complete.
const WORKLOAD_PHASES = [
	"backend-readiness",
	"show-preparation",
	"fixture-sheet-readiness",
	"measurement",
	"fixture-sheet-liveness",
	"complete",
];

function observedBoolean(value) {
	if (value === undefined || value === null) return null;

	require_(typeof value === "boolean", "invalid observed boolean");
	return value ? 1 : 0;
}

function observedEvent(value) {
	if (value === undefined) return null;

	if (value === null) return 0;

	require_(
		typeof value === "object" && !Array.isArray(value),
		"invalid observed event",
	);
	return 1;
}

function sheetSummary(sheet = {}) {
	sheet ??= {};
	const readiness = sheet.readiness;
	const count = (value) =>
		value === undefined || value === null ? null : uint(value);
	const config = readiness?.configState;
	const configState =
		config === undefined
			? null
			: ["loading", "unavailable", "loaded"].indexOf(config);
	require_(
		configState === null || configState >= 0,
		"unknown Sheet configuration state",
	);
	const surface = sheet.surface?.kind;

	require_(
		surface === undefined ||
			[
				"fixture-sheet-container-mounted",
				"fixture-sheet-container-unmounted",
			].includes(surface),
		"unknown Sheet surface state",
	);

	return {
		ready: observedBoolean(sheet.ready),
		readiness_timed_out: observedBoolean(sheet.readinessTimedOut),
		report_present: observedBoolean(sheet.reportPresent),
		expected_fixture_records: count(sheet.expectedFixtureRecords),
		ready_fixture_records: count(sheet.readyFixtureRecords),
		surface_mounted:
			surface === undefined
				? null
				: surface === "fixture-sheet-container-mounted"
					? 1
					: 0,
		readiness_observed: observedEvent(readiness),
		configuration_state: configState,
		active: observedBoolean(readiness?.active),
		fixture_sheet_enabled: observedBoolean(readiness?.fixtureSheetEnabled),
		reported_fixture_records: count(readiness?.fixtureRecords),
		row_count: count(readiness?.rowCount),
		active_values_loading: observedBoolean(readiness?.activeValuesLoading),
		group_runtime_loading: observedBoolean(readiness?.groupRuntimeLoading),
		render_failed: observedEvent(sheet.renderFailure),
		lifecycle_failed: observedEvent(sheet.lifecycleFailure),
		ready_append_failed: observedEvent(sheet.readyAppendFailure),
		heartbeat_count: count(sheet.heartbeatCount),
		widest_silence_ms: count(sheet.widestSilenceMillis),
		tolerated_silence_ms: count(sheet.toleratedSilenceMillis),
	};
}

// Same arithmetic as measureWindow; only complete, ordered retained observations qualify.
// The exact production function is executed by cadence conformance tests, not copied there.
function retainedCadence(samples) {
	const valid = (value) => Number.isSafeInteger(value) && value >= 0;
	try {
		require_(
			samples.length === 61 && samples[0].phase === "opening",
			"incomplete cadence window",
		);
		const rates = [],
			opening = samples[0].diagnostics.output;
		const bounds = opening.frame_rate_band_bounds_hz ?? [];
		require_(
			Array.isArray(bounds) && [0, 34].includes(bounds.length),
			"cadence bands unavailable",
		);
		require_(
			bounds.every(
				(value, i) =>
					Number.isFinite(value) && value >= 0 && (!i || value > bounds[i - 1]),
			),
			"invalid cadence bounds",
		);
		let previous;
		for (const [index, sample] of samples.entries()) {
			const start = sample.diagnosticsMonotonicStartMillis;
			const end = sample.diagnosticsMonotonicEndMillis;
			const output = sample.diagnostics.output;
			const bands = output.frame_rate_band_counts ?? [];
			require_(
				Number.isFinite(start) &&
					start >= 0 &&
					Number.isFinite(end) &&
					end >= start,
				"invalid cadence request bounds",
			);
			require_(
				JSON.stringify(output.frame_rate_band_bounds_hz ?? []) ===
					JSON.stringify(bounds),
				"cadence bounds changed",
			);
			require_(
				Array.isArray(bands) &&
					bands.length === bounds.length &&
					bands.every(valid),
				"invalid cadence bands",
			);
			for (const key of ["frames_sent", "deadline_misses", "send_errors"]) {
				require_(
					valid(output[key]) &&
						(!previous || output[key] >= previous.diagnostics.output[key]),
					"cadence counter regression",
				);
			}
			if (previous) {
				require_(
					sample.phase === "measurement" &&
						sample.second === index - 1 &&
						start >= previous.diagnosticsMonotonicEndMillis,
					"unordered cadence observations",
				);
				const elapsed =
					((start + end) / 2 -
						(previous.diagnosticsMonotonicStartMillis +
							previous.diagnosticsMonotonicEndMillis) /
							2) /
					1000;
				require_(
					Number.isFinite(elapsed) && elapsed > 0 && sample.elapsed === elapsed,
					"cadence elapsed differs from measured interval",
				);
				require_(
					bands.every(
						(count, i) =>
							count >= previous.diagnostics.output.frame_rate_band_counts[i],
					),
					"cadence band counter regression",
				);
				rates.push(
					(output.frames_sent - previous.diagnostics.output.frames_sent) /
						elapsed,
				);
			}
			previous = sample;
		}
		const sorted = [...rates].sort((left, right) => left - right);
		const result = {
			minimum_one_second_completed_hz: sorted[0],
			average_completed_hz:
				rates.reduce((total, value) => total + value, 0) / rates.length,
			p95_one_second_completed_hz:
				sorted[
					Math.min(sorted.length - 1, Math.ceil((5 / 100) * sorted.length) - 1)
				],
			maximum_one_second_completed_hz: sorted.at(-1),
			frame_rate_band_bounds_hz: bounds,
			frame_rate_band_counts: (opening.frame_rate_band_counts ?? []).map(
				(count, index) =>
					Math.max(
						0,
						(previous.diagnostics.output.frame_rate_band_counts?.[index] ?? 0) -
							count,
					),
			),
			windows_below_target: rates.filter((rate) => rate < 38).length,
		};
		for (const value of [
			sorted[0],
			result.average_completed_hz,
			result.p95_one_second_completed_hz,
			sorted.at(-1),
			...bounds,
		])
			scaled(value);
		return result;
	} catch {
		return undefined;
	}
}

export function workloadSummary(result, samples = [], state = {}) {
	const complete = result === undefined ? 0 : 1;

	if (complete)
		require_(
			result.case_id === "sixteen_universe" &&
				result.execution_mode === "unrestricted" &&
				result.fixture_count === 576 &&
				result.requested_rate_hz === 40 &&
				result.measurement_seconds === 60,
			"workload differs from existing576 case",
		);

	const first = samples[0]?.diagnostics?.output;
	const last = samples.at(-1)?.diagnostics?.output;

	const delta = (key) => {
		if (
			samples.length < 2 ||
			first?.[key] === undefined ||
			last?.[key] === undefined
		)
			return null;

		if (
			!complete &&
			![first[key], last[key]].every(
				(value) => Number.isSafeInteger(value) && value >= 0,
			)
		)
			return null;
		const before = uint(first[key]),
			after = uint(last[key]);
		return after < before ? null : after - before;
	};

	const cadence = complete ? result : retainedCadence(samples);
	const minimum = scaled(cadence?.minimum_one_second_completed_hz);
	const deadline = delta("deadline_misses"),
		errors = delta("send_errors");
	const resources = result?.resources;
	const bounds = cadence?.frame_rate_band_bounds_hz ?? [];
	const bands = cadence?.frame_rate_band_counts ?? [];

	require_(
		Array.isArray(bounds) &&
			Array.isArray(bands) &&
			bounds.length === bands.length &&
			[0, 34].includes(bounds.length),
		"invalid workload rate bands",
	);

	const component = (role, source) => ({
		role,
		cpu_average_percent_microunits: scaled(source?.cpu_average_percent),
		cpu_max_percent_microunits: scaled(source?.cpu_max_percent),
		peak_resident_bytes:
			source?.peak_resident_bytes === undefined
				? null
				: uint(source.peak_resident_bytes),
	});

	return {
		case: 1,
		execution_mode: 0,
		expected_fixture_records: 576,
		observed_fixture_records:
			result?.fixture_count === undefined ? null : uint(result.fixture_count),
		physical_instance_count:
			result?.physical_instance_count === undefined
				? null
				: uint(result.physical_instance_count),
		parameter_count:
			result?.parameter_count === undefined
				? null
				: uint(result.parameter_count),
		universe_count:
			result?.universes === undefined ? null : uint(result.universes),
		animated_attribute_count:
			result?.animated_attribute_count === undefined
				? null
				: uint(result.animated_attribute_count),
		master_lane_count:
			result?.master_lane_count === undefined
				? null
				: uint(result.master_lane_count),
		requested_rate_microhz: 40000000,
		minimum_rate_gate_microhz: 38000000,
		configured_poll_iterations: 60,
		observed_poll_iterations: samples.filter(
			(entry) => entry?.phase === "measurement",
		).length,
		workload_complete: complete,
		phase: WORKLOAD_PHASES.indexOf(state.phase) + 1,
		fixture_sheet: sheetSummary(state.sheet),
		backend_ready: state.backendReady === true ? 1 : 0,
		show_prepared: state.prepared === true ? 1 : 0,
		fixture_sheet_liveness_gate_met:
			observedBoolean(state.sheet?.livenessPassed) ?? (complete ? 1 : null),
		minimum_one_second_completed_microhz: minimum,
		average_completed_microhz: scaled(cadence?.average_completed_hz),
		p95_one_second_completed_microhz: scaled(
			cadence?.p95_one_second_completed_hz,
		),
		maximum_one_second_completed_microhz: scaled(
			cadence?.maximum_one_second_completed_hz,
		),
		frame_rate_band_bounds_microhz: bounds.map((value) => scaled(value)),
		frame_rate_band_counts: bands.map((value) => uint(value)),
		windows_below_38hz:
			cadence?.windows_below_target === undefined
				? null
				: uint(cadence.windows_below_target, 60),
		rate_gate_met:
			minimum === null ? null : minimum < 38000000 ? 0 : complete ? 1 : null,
		frames_sent_delta: delta("frames_sent"),
		output_deadline_misses_delta: deadline,
		output_send_errors_delta: errors,
		deadline_gate_met:
			deadline === null ? null : deadline > 0 ? 0 : complete ? 1 : null,
		send_error_gate_met:
			errors === null ? null : errors > 0 ? 0 : complete ? 1 : null,
		mutation_measurement_available: 0,
		mutation_budget_ns: 5000000,
		mutation_p95_ns: null,
		mutation_gate_met: null,
		application_cpu_average_percent_microunits: scaled(
			resources?.application_cpu_average_percent,
		),
		application_cpu_p95_percent_microunits: scaled(
			resources?.application_cpu_p95_percent,
		),
		application_cpu_max_percent_microunits: scaled(
			resources?.application_cpu_max_percent,
		),
		application_peak_resident_bytes:
			resources?.application_peak_resident_bytes === undefined
				? null
				: uint(resources.application_peak_resident_bytes),

		process_resources: [
			component(1, resources?.server),
			component(2, resources?.native_renderer),
			component(3, resources?.desktop_webview),
			component(4, resources?.desktop_host),
		],
	};
}
