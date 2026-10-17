import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import {
	assertNumericOnly,
	workloadSummary,
} from "./desktop-performance-cpu-stages.mjs";

const runner = await readFile(
	new URL("./run-desktop-performance.mjs", import.meta.url),
	"utf8",
);
function productionFunction(name, async = false) {
	const signature = `${async ? "async " : ""}function ${name}(`;
	assert.equal(
		runner.split(signature).length,
		2,
		`exactly one canonical ${name}`,
	);
	const start = runner.indexOf(signature);
	const end = runner.indexOf("\n}", start);
	assert.ok(end > start, `${name} has a top-level closing brace`);
	return runner.slice(start, end + 2);
}
const canonical = [
	productionFunction("average"),
	productionFunction("percentile"),
	productionFunction("measureWindow", true),
].join("\n");
const fields = [
	"minimum_one_second_completed_microhz",
	"average_completed_microhz",
	"p95_one_second_completed_microhz",
	"maximum_one_second_completed_microhz",
	"frame_rate_band_bounds_microhz",
	"frame_rate_band_counts",
	"windows_below_38hz",
];

// Execute the actual runner's 60-poll source with deterministic counter/read/I/O clocks.
// No copied expected aggregation formula and no real sleep, app, process or network I/O.
async function canonicalWindow({
	increments = [0, 37, 38, 39, 80, 42],
	failure,
	banded = true,
} = {}) {
	let clock = 10,
		reads = 0,
		frames = 100,
		deadline = 2;
	const samples = [];
	const bounds = banded ? Array.from({ length: 34 }, (_, i) => 10 + i * 3) : [];
	const counts = bounds.map(() => 5);
	const measure = vm.runInNewContext(`${canonical}\nmeasureWindow`, {
		DURATION_SECONDS: 60,
		performance: { now: () => clock },
		setTimeout: (callback) => {
			clock += 1000 + (reads % 3) * 17;
			callback();
		},
		readLinuxProcessTree: async () => {
			clock += 4 + (reads % 2);
			return [];
		},
		processTreeResourceDelta: () => ({ cpuPercent: 0, residentBytes: 0 }),
		LINUX_PROCESS_OPTIONS: {},
	});
	const api = {
		request: async () => {
			clock += 5 + (reads % 5);
			if (reads) {
				frames += increments[(reads - 1) % increments.length];
				if (counts.length) counts[reads % counts.length]++;
				if (reads % 9 === 0) deadline++;
			}
			reads++;
			return {
				output: {
					frames_sent: frames,
					deadline_misses: deadline,
					send_errors: 0,
					frame_rate_band_bounds_hz: [...bounds],
					frame_rate_band_counts: [...counts],
				},
			};
		},
	};
	const cpu = failure
		? {
				requestStart() {},
				requestEnd() {},
				async observe() {},
				headlessStacks: {
					async run(_cpu, measurement) {
						await measurement();
						throw failure;
					},
				},
			}
		: undefined;
	let result;
	if (failure)
		await assert.rejects(
			measure(api, 123, "unrestricted", samples, cpu),
			(error) => error === failure,
		);
	else result = await measure(api, 123, "unrestricted", samples);
	assert.equal(reads, 61);
	assert.equal(samples.length, 61);
	return {
		samples: structuredClone(samples),
		result: result && JSON.parse(JSON.stringify(result)),
	};
}
function completeResult(measured) {
	return {
		...measured,
		case_id: "sixteen_universe",
		execution_mode: "unrestricted",
		fixture_count: 576,
		requested_rate_hz: 40,
		measurement_seconds: 60,
	};
}
function cadence(value) {
	return Object.fromEntries(fields.map((field) => [field, value[field]]));
}
function unavailable(value) {
	for (const field of fields.filter(
		(field) => !field.startsWith("frame_rate_band"),
	))
		assert.equal(value[field], null, field);
	assert.deepEqual(value.frame_rate_band_bounds_microhz, []);
	assert.deepEqual(value.frame_rate_band_counts, []);
	assert.equal(value.rate_gate_met, null);
}

test("partial cadence is exactly canonical measured cadence while complete path and negative gates remain unchanged", async () => {
	const { samples, result } = await canonicalWindow();
	const complete = workloadSummary(completeResult(result), samples, {
		phase: "complete",
	});
	const fallback = workloadSummary(undefined, samples, {
		phase: "measurement",
	});
	assert.deepEqual(cadence(fallback), cadence(complete));
	assert.equal(fallback.rate_gate_met, 0);
	assert.equal(fallback.workload_complete, 0);
	assert.equal(fallback.phase, 4);
	assert.equal(fallback.fixture_sheet_liveness_gate_met, null);
	assert.equal(fallback.application_cpu_average_percent_microunits, null);
	assert.ok(
		fallback.process_resources.every(
			(row) => row.cpu_average_percent_microunits === null,
		),
	);
	assert.equal(fallback.deadline_gate_met, 0);
	assert.equal(fallback.send_error_gate_met, null);
	assert.equal(fallback.mutation_p95_ns, null);
	assert.equal(fallback.requested_rate_microhz, 40000000);
	assert.equal(fallback.minimum_rate_gate_microhz, 38000000);
	assertNumericOnly(fallback);
	// A complete result remains authoritative even if retained samples are short.
	assert.deepEqual(
		cadence(workloadSummary(completeResult(result), [])),
		cadence(complete),
	);
});

test("full fast or zero cadence is observed without promoting a partial workload to PASS", async () => {
	for (const increments of [[50], [0]]) {
		const { samples, result } = await canonicalWindow({ increments });
		const fallback = workloadSummary(undefined, samples);
		assert.deepEqual(
			cadence(fallback),
			cadence(workloadSummary(completeResult(result), samples)),
		);
		assert.equal(fallback.rate_gate_met, increments[0] ? null : 0);
		assert.equal(fallback.workload_complete, 0);
		assert.equal(fallback.send_error_gate_met, null);
		assert.equal(fallback.fixture_sheet_liveness_gate_met, null);
	}
});

test("short, unordered, regressed and nonfinite windows never synthesize cadence", async (t) => {
	const { samples } = await canonicalWindow();
	const corruptions = {
		nullSample: (s) => {
			s[30] = null;
		},
		short: (s) => s.pop(),
		extra: (s) => s.push(structuredClone(s.at(-1))),
		opening: (s) => (s[0].phase = "measurement"),
		phase: (s) => (s[30].phase = "opening"),
		duplicate: (s) => (s[30] = structuredClone(s[29])),
		second: (s) => (s[30].second = 10),
		elapsedZero: (s) => (s[30].elapsed = 0),
		elapsedNegative: (s) => (s[30].elapsed = -1),
		elapsedInfinite: (s) => (s[30].elapsed = Infinity),
		elapsedMissing: (s) => delete s[30].elapsed,
		elapsedMismatch: (s) => (s[30].elapsed += 0.01),
		clockInfinite: (s) => (s[30].diagnosticsMonotonicEndMillis = Infinity),
		clockBackwards: (s) =>
			(s[30].diagnosticsMonotonicStartMillis =
				s[29].diagnosticsMonotonicEndMillis - 1),
		reversedBounds: (s) =>
			(s[30].diagnosticsMonotonicEndMillis =
				s[30].diagnosticsMonotonicStartMillis - 1),
		framesRegressed: (s) => (s[30].diagnostics.output.frames_sent = 0),
		framesFractional: (s) => (s[30].diagnostics.output.frames_sent += 0.1),
		framesNaN: (s) => (s[30].diagnostics.output.frames_sent = NaN),
		framesMissing: (s) => delete s[30].diagnostics.output.frames_sent,
		firstCounterInvalid: (s) => (s[0].diagnostics.output.frames_sent = NaN),
		lastCounterInvalid: (s) =>
			(s[60].diagnostics.output.frames_sent = Infinity),
		unsafeRate: (s) => {
			for (let i = 0; i < s.length; i++)
				s[i].diagnostics.output.frames_sent = i * 1e13;
		},
		deadlineRegressed: (s) => (s[30].diagnostics.output.deadline_misses = 0),
		errorsRegressed: (s) => {
			s[29].diagnostics.output.send_errors = 1;
		},
		bandRegressed: (s) =>
			(s[30].diagnostics.output.frame_rate_band_counts[0] = 0),
		bandMissing: (s) => s[30].diagnostics.output.frame_rate_band_counts.pop(),
		bandInfinite: (s) =>
			(s[30].diagnostics.output.frame_rate_band_counts[0] = Infinity),
		boundsChanged: (s) =>
			(s[30].diagnostics.output.frame_rate_band_bounds_hz[1] += 1),
		boundsString: (s) =>
			(s[30].diagnostics.output.frame_rate_band_bounds_hz[0] = "private/path"),
	};
	for (const [name, corrupt] of Object.entries(corruptions))
		await t.test(name, () => {
			const invalid = structuredClone(samples);
			corrupt(invalid);
			unavailable(workloadSummary(undefined, invalid));
		});
	unavailable(workloadSummary(undefined));
});

test("post-measurement capture rejection preserves original error and still exports observed failure-only cadence", async () => {
	const original = new Error("private capture rejection unchanged");
	const { samples } = await canonicalWindow({ failure: original });
	const fallback = workloadSummary(undefined, samples, {
		phase: "measurement",
		backendReady: true,
		prepared: true,
	});
	assert.notEqual(fallback.average_completed_microhz, null);
	assert.equal(fallback.workload_complete, 0);
	assert.equal(fallback.observed_poll_iterations, 60);
	assert.equal(fallback.rate_gate_met, 0);
	assert.equal(fallback.fixture_sheet_liveness_gate_met, null);
	assert.ok(!JSON.stringify(fallback).includes("private"));
});

test("an explicitly empty histogram stays canonical without inventing bands", async () => {
	const { samples, result } = await canonicalWindow({ banded: false });
	const fallback = workloadSummary(undefined, samples);
	assert.deepEqual(
		cadence(fallback),
		cadence(workloadSummary(completeResult(result), samples)),
	);
	assert.deepEqual(fallback.frame_rate_band_counts, []);
	assert.notEqual(fallback.average_completed_microhz, null);
});
