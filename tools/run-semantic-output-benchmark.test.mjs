import assert from "node:assert/strict";
import test from "node:test";
import {
	allowedP99RegressionMs,
	evaluateGates,
	median,
	medianSummary,
	summarizeScenario,
	workloadEvidence,
} from "./run-semantic-output-benchmark.mjs";

const distribution = (p50, p95, p99) => ({ p50_microseconds: p50 * 1_000, p95_microseconds: p95 * 1_000, p99_microseconds: p99 * 1_000, maximum_microseconds: p99 * 1_000 });
const counts = (p50, p95 = p50, p99 = p95, frames = 10) => ({ frames, total: p50 * frames, p50, p95, p99, maximum: p99 });

function benchmark({ p99 = 5, misses = 0, met = true, semantic } = {}) {
	return {
		required_floor_met: null,
		process_resources: { resident_bytes: 1 },
		scenarios: [
			{
				profile: "semantic_workload",
				fixture_count: 112,
				configured_rate_hz: 60,
				met_configured_rate: met,
				completed_ticks: 240,
				warmup_elapsed_seconds: 1,
				elapsed_seconds: 4,
				frame_rate: { average_completed_hz: 60, minimum_one_second_completed_hz: 60 },
				deadline: { deadline_misses: misses, dropped_ticks: 0, deferred_ticks: 0 },
				phases: { total_pipeline: distribution(p99 / 2, p99, p99) },
				semantic,
			},
		],
	};
}

function semantic({ fits = 0, dirty = 6, expected = 6, color = 66, generation = 0, compiles = 0, tracking = true } = {}) {
	return {
		family_engaged: true,
		frames: 10,
		hybrid_frames: 10,
		phases: { prepared_capture: distribution(0.1, 0.1, 0.1), dynamics_and_family_transaction: distribution(3, 4, 5), publication: distribution(0.1, 0.1, 0.1) },
		tracking: tracking ? { installed_samples: 300, tracking_to_output: distribution(8, 20, 24) } : null,
		work: {
			position_fits: counts(fits),
			position_fit_cache_hits: counts(47),
			color_fits_and_refits: counts(color),
			optics_resolves: counts(12),
			tracking_changed_points: counts(1),
			tracking_dirty_instances: counts(dirty),
			descriptor_and_fitter_compiles_in_window: compiles,
			generation_changes_in_window: generation,
		},
		workload: { expected_dirty_targets: expected },
		readout_consumers: null,
	};
}

test("medians and the paired allowance follow the documented method", () => {
	assert.equal(median([3, 1, 2]), 2);
	assert.equal(median([4, 1, 2, 3]), 2.5);
	assert.equal(median([]), null);
	assert.equal(allowedP99RegressionMs(10), 1);
	assert.equal(allowedP99RegressionMs(40), 2);
});

test("paired legacy gates compare round medians and keep the hard floor", () => {
	const run = (p99, met = true) => summarizeScenario(benchmark({ p99, met }));
	const summary = {
		legacy: {
			"stress-2000": { baseline: [run(6), run(7), run(6.5)], candidate: [run(6.6), run(7.2), run(6.9)] },
			"hard-floor-4148-125hz": { baseline: [run(4)], candidate: [run(9, false)] },
		},
	};
	const gates = evaluateGates(summary);
	assert.equal(gates.find((row) => row.gate.startsWith("stress-2000")).status, "pass");
	assert.equal(gates.find((row) => row.gate.includes("hard-floor-4148-125hz: paired")).status, "fail");
	assert.equal(gates.find((row) => row.gate.includes("rate held every round")).status, "fail");
	assert.equal(medianSummary(summary.legacy["stress-2000"].candidate).pipelineP99Ms, 6.9);
});

test("dirty-work gates compare exactly against the manifest", () => {
	const staticRun = (scenario, options) => ({ scenario, staticBases: true, summary: summarizeScenario(benchmark({ semantic: semantic(options) })) });
	const gates = evaluateGates({
		workload: {
			staticBases: [
				staticRun("static-points", { fits: 0, dirty: 0, expected: 0, color: 0 }),
				staticRun("small-subset", { fits: 7, dirty: 6, expected: 6 }),
			],
			matrix: [{ scenario: "small-subset", trackingHz: 60, outputHz: 60, summary: summarizeScenario(benchmark({ misses: 2, semantic: semantic({ generation: 1 }) })) }],
		},
	});
	const status = (prefix) => gates.find((row) => row.gate.startsWith(prefix)).status;
	assert.equal(status("TL-564 static bases static-points: 0 Position fits"), "pass");
	assert.equal(status("TL-564 static bases static-points: unchanged Color"), "pass");
	assert.equal(status("TL-564 static bases small-subset: dirty"), "pass");
	assert.equal(status("TL-564 static bases small-subset: Position fits"), "fail");
	assert.equal(status("TL-564 small-subset tracking 60 Hz / output 60 Hz: rate held"), "fail");
	assert.equal(status("TL-564 small-subset tracking 60 Hz / output 60 Hz: no generation"), "fail");
});

test("consumer gates need a zero-consumer reference and equal per-frame solves", () => {
	const run = (consumers, color) => ({ consumers, holdMs: 0, summary: summarizeScenario(benchmark({ semantic: semantic({ color }) })) });
	const gates = evaluateGates({ workload: { consumers: [run(0, 66), run(4, 66), run(8, 70)] } });
	assert.deepEqual(gates.map((row) => row.status), ["pass", "fail"]);
	assert.equal(evaluateGates({ workload: { consumers: [run(4, 66)] } })[0].status, "unavailable");
});

test("workload evidence never turns missing data into zero", () => {
	const raw = benchmark({ semantic: semantic({ tracking: false }) });
	const run = { scenario: "static-points", staticBases: false, summary: summarizeScenario(raw) };
	const evidence = workloadEvidence(run, raw);
	assert.equal(evidence.output.trackingToOutputP95Ms.status, "unavailable");
	assert.equal(evidence.output.outputSendErrors.status, "unavailable");
	assert.equal(evidence.semantic.portableShowWritesFromMotion.status, "unavailable");
	assert.equal(evidence.semantic.redundantStaticAimSolves.status, "unavailable");
	assert.equal(evidence.semantic.generationRebuildsFromMotion.value, 0);
	assert.equal(evidence.productionSemanticSupport.status, "active");
	const staticRaw = benchmark({ semantic: semantic({ fits: 0 }) });
	const staticEvidence = workloadEvidence({ scenario: "static-points", staticBases: true, summary: summarizeScenario(staticRaw) }, staticRaw);
	assert.equal(staticEvidence.semantic.redundantStaticAimSolves.value, 0);
	assert.equal(staticEvidence.output.trackingSampleHz.value, 60);
});
