import assert from "node:assert/strict";
import test from "node:test";
import { createOutputWindowAccumulator } from "./output-histogram.mjs";
import { queueStageOutputSample } from "./stage-output-sampling.mjs";

const bounds = [...Array.from({ length: 128 }, (_, index) => (index + 1) * 250),
	48_000, 64_000, 100_000, 1_000_000, Number.MAX_SAFE_INTEGER];
function output(count) {
	const fine = Array(bounds.length).fill(0);
	fine[32] = count;
	return {
		frames_sent: count, packets_sent: count * 2, send_errors: 0,
		deadline_misses: 0, maximum_lateness_micros: 0, last_tick_micros: 8_100,
		maximum_tick_micros: 8_100, scheduler_utilization: 0.4,
		tick_duration_bucket_bounds_micros: [250, 10_000],
		tick_duration_bucket_counts: [0, count],
		tick_duration_fine_histogram: { schema_version: 1, bounds_micros: bounds,
			counts: fine, sample_count: count },
		frame_rate_band_counts: [count], recent_send_errors: 0,
		change_lead: { admission_note: "preserve complete typed health" },
	};
}

test("counter endpoint forwards the complete paired health and fine counters without filtering", async () => {
	const health = output(20);
	const session = { token: "owned benchmark session" };
	const seen = [];
	const actual = await queueStageOutputSample(Promise.resolve(), async (...args) => {
		seen.push(args);
		return { output: health, output_sample: { monotonic_micros: 12 },
			visualization: {}, programmer_action_timing: {} };
	}, session, (snapshot) => snapshot);
	assert.equal(actual, health, "forward the complete health object, including future fields");
	assert.deepEqual(seen, [["GET", "/api/v2/diagnostics/performance", undefined, { session }]]);
	assert.deepEqual(actual.tick_duration_fine_histogram, health.tick_duration_fine_histogram);
});

test("delayed observations and a window close stay FIFO across a scheduler reset", async () => {
	const snapshots = [output(10), output(15), output(2), output(7)];
	const accumulator = createOutputWindowAccumulator(snapshots[0]);
	const oldFullDiagnostics = createOutputWindowAccumulator(snapshots[0]);
	let unblock;
	const gate = new Promise((resolve) => { unblock = resolve; });
	let reads = 0;
	const read = async () => {
		reads++;
		if (reads === 1) await gate;
		return { output: snapshots[reads], output_sample: {} };
	};
	let pending = queueStageOutputSample(Promise.resolve(), read, "session", (health) => accumulator.observe(health, 0));
	pending = queueStageOutputSample(pending, read, "session", (health) => accumulator.observe(health, 0));
	const closed = queueStageOutputSample(pending, read, "session", (health) => accumulator.take(health));
	await Promise.resolve();
	assert.equal(reads, 1, "a queued close cannot overtake the outstanding HTTP read");
	unblock();
	const actual = await closed;
	oldFullDiagnostics.observe(snapshots[1], 0);
	oldFullDiagnostics.observe(snapshots[2], 0);
	const expected = oldFullDiagnostics.take(snapshots[3]);
	assert.deepEqual(actual, expected);
	assert.equal(reads, 3);
	assert.equal(actual.frames_sent, 12);
	assert.equal(actual.packets_sent, 24);
	assert.equal(actual.counter_resets, 1);
	assert.equal(actual.tick_duration_fine_histogram.sample_count, 12);
	assert.equal(actual.fine_histogram_coverage.status, "complete");
	const next = await queueStageOutputSample(closed, async () => ({ output: output(9) }), "session", (health) => accumulator.take(health));
	assert.equal(next.frames_sent, 2, "the next paired window starts at the previous exact close");
});

test("a failed read rejects while caller recovery preserves later observations", async () => {
	const error = new Error("owned server disconnected");
	const failed = queueStageOutputSample(Promise.resolve(), async () => { throw error; }, "session", () => assert.fail("no observation on failed read"));
	await assert.rejects(failed, (actual) => actual === error);
	const later = output(5);
	const result = await queueStageOutputSample(failed.catch(() => undefined), async () => ({ output: later }), "session", (health) => health);
	assert.equal(result, later);
});
