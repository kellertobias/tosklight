import assert from "node:assert/strict";
import test from "node:test";
import {
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
