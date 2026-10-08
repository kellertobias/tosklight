import assert from "node:assert/strict";
import test from "node:test";
import {
	createOutputWindowAccumulator,
	histogramPercentileMicros,
	outputWindow,
} from "./output-histogram.mjs";

const output = (counts) => ({
	frames_sent: counts.reduce((sum, count) => sum + count, 0),
	packets_sent: 0,
	send_errors: 0,
	deadline_misses: 0,
	maximum_lateness_micros: 0,
	last_tick_micros: 100,
	maximum_tick_micros: 200,
	scheduler_utilization: 0.1,
	tick_duration_bucket_bounds_micros: [250, 500, 1_000, 2_000],
	tick_duration_bucket_counts: counts,
});

test("subtracts fixed-bucket snapshots and computes their percentile", () => {
	const window = outputWindow(output([1, 2, 3, 4]), output([2, 4, 6, 14]));

	assert.deepEqual(window.tick_duration_bucket_counts, [1, 2, 3, 10]);
	assert.equal(histogramPercentileMicros(window, 50), 2_000);
	assert.equal(histogramPercentileMicros(window, 99), 2_000);
});

test("returns null for an empty bounded window", () => {
	const snapshot = output([1, 2, 3, 4]);
	assert.equal(
		histogramPercentileMicros(outputWindow(snapshot, snapshot), 99),
		null,
	);
});

test("p99 excludes one isolated outlier only after the window exceeds 100 samples", () => {
	assert.equal(histogramPercentileMicros(output([43, 0, 0, 1]), 99), 2_000);
	assert.equal(histogramPercentileMicros(output([100, 0, 0, 1]), 99), 250);
});

test("an accumulated window keeps the ticks on both sides of a scheduler counter reset", () => {
	const accumulator = createOutputWindowAccumulator(output([1, 0, 0, 0]));
	accumulator.observe(output([3, 2, 0, 0]));
	// A show switch restarts the scheduler: its counters begin again from zero.
	accumulator.observe(output([0, 1, 0, 0]));
	const window = accumulator.take(output([0, 2, 0, 1]));

	assert.deepEqual(window.tick_duration_bucket_counts, [2, 4, 0, 1]);
	assert.equal(window.frames_sent, 7);
	assert.equal(window.counter_resets, 1);
	const next = accumulator.take(output([0, 3, 0, 1]));
	assert.deepEqual(next.tick_duration_bucket_counts, [0, 1, 0, 0]);
	assert.equal(next.counter_resets, 0);
});
