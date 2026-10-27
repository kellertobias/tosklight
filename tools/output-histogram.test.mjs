import assert from "node:assert/strict";
import test from "node:test";
import {
	createOutputWindowAccumulator,
    compareOutputWindows,
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

const fineBounds = [...Array.from({length:128},(_,index)=>(index+1)*250),48_000,64_000,100_000,1_000_000,Number.MAX_SAFE_INTEGER];
function withFine(snapshot, bins) {
    const counts = Array.from({length:fineBounds.length},()=>0);
    for (const [bound,count] of bins) counts[fineBounds.indexOf(bound)] = count;
    return {...snapshot,tick_duration_fine_histogram:{schema_version:1,bounds_micros:[...fineBounds],counts,sample_count:counts.reduce((a,b)=>a+b,0)}};
}
function measured(bound,count=100) { return withFine(output([0,count,0,0]),[[bound,count]]); }

test("fine window subtracts complete cumulative counts without altering coarse fields",()=>{
    const first = withFine(output([0,10,0,0]),[[8250,10]]);
    const last = withFine(output([0,110,0,0]),[[8250,20],[8750,90]]);
    const window = outputWindow(first,last);
    assert.equal(window.fine_histogram_coverage.status,"complete");
    assert.equal(window.tick_duration_fine_histogram.sample_count,100);
    assert.equal(window.tick_duration_fine_histogram.counts[32],10);
    assert.equal(window.tick_duration_fine_histogram.counts[34],90);
    assert.deepEqual(window.tick_duration_bucket_counts,[0,100,0,0]);
});
test("a zero omitted baseline and zero reset remain complete when fine counts arrive",()=>{
    const accumulator = createOutputWindowAccumulator(output([0,0,0,0]));
    accumulator.observe(measured(8250,20));
    accumulator.observe(output([0,0,0,0]));
    accumulator.observe(measured(8750,10));
    const window = accumulator.take(measured(8750,20));
    assert.equal(window.counter_resets,1);
    assert.equal(window.fine_histogram_coverage.status,"complete");
    assert.equal(window.tick_duration_fine_histogram.sample_count,40);
    assert.equal(window.tick_duration_fine_histogram.counts[32],20);
    assert.equal(window.tick_duration_fine_histogram.counts[34],20);
    const next = accumulator.take(measured(8750,30));
    assert.equal(next.tick_duration_fine_histogram.sample_count,10);
});
test("complete fine counters accumulate across nonzero scheduler resets",()=>{
    const accumulator = createOutputWindowAccumulator(measured(8250,10));
    accumulator.observe(measured(8250,30));
    accumulator.observe(measured(8750,5));
    const window = accumulator.take(measured(8750,15));
    assert.equal(window.counter_resets,1);
    assert.equal(window.tick_duration_fine_histogram.sample_count,35);
    assert.equal(window.tick_duration_fine_histogram.counts[32],20);
    assert.equal(window.tick_duration_fine_histogram.counts[34],15);
});
test("missing nonempty intermediate fine coverage cannot disappear after later valid samples",()=>{
    const accumulator = createOutputWindowAccumulator(measured(8250,10));
    accumulator.observe(output([0,20,0,0]));
    accumulator.observe(measured(8250,30));
    const window = accumulator.take(measured(8250,40));
    assert.equal(window.fine_histogram_coverage.status,"invalid");
    assert.equal(window.tick_duration_fine_histogram,undefined);
    const comparison = compareOutputWindows(measured(8250),window);
    assert.equal(comparison.boundedWindowGatePassed,false);
    assert.equal(comparison.p99ComparisonSource,"fine_histogram_rejected");
});
test("malformed version, geometry, counts and sample coverage are rejected",()=>{
    const mutations = [
        fine=>fine.schema_version=2,
        fine=>fine.bounds_micros[1]=499,
        fine=>fine.bounds_micros.pop(),
        fine=>delete fine.bounds_micros[1],
        fine=>fine.counts[0]=-1,
        fine=>fine.counts[0]=0.5,
        fine=>fine.counts[0]=Number.NaN,
        fine=>delete fine.counts[0],
        fine=>fine.counts[0]=Number.MAX_SAFE_INTEGER+1,
        fine=>fine.counts=null,
        fine=>fine.counts="invalid",
        fine=>fine.sample_count=99,
        fine=>fine.sample_count=Number.POSITIVE_INFINITY,
        fine=>fine.counts.pop(),
    ];
    for (const mutate of mutations) {
        const last = measured(8250);
        mutate(last.tick_duration_fine_histogram);
        const window = outputWindow(output([0,0,0,0]),last);
        assert.equal(window.fine_histogram_coverage.status,"invalid");
        assert.equal(compareOutputWindows(measured(8250),window).boundedWindowGatePassed,false);
    }
    const wrongTotal = measured(8250);
    wrongTotal.tick_duration_bucket_counts=[0,101,0,0];
    assert.equal(outputWindow(output([0,0,0,0]),wrongTotal).fine_histogram_coverage.status,"invalid");
});
test("fine bucket movement without a coarse reset cannot be silently clamped",()=>{
    const first = measured(8250,10);
    const last = measured(8750,20);
    const window = outputWindow(first,last);
    assert.equal(window.fine_histogram_coverage.status,"invalid");
    assert.match(window.fine_histogram_coverage.reasons.join(" "),/without matching coarse/);
});
test("fine geometry drift across a reset is not normalized into a valid window",()=>{
    const accumulator = createOutputWindowAccumulator(measured(8250,10));
    accumulator.observe(measured(8250,20));
    const changed = measured(8750,5);
    changed.tick_duration_fine_histogram.bounds_micros[0]=249;
    const window = accumulator.take(changed);
    assert.equal(window.counter_resets,1);
    assert.equal(window.fine_histogram_coverage.status,"invalid");
});
test("fine interval gate distinguishes guaranteed pass, fail and boundary ambiguity",()=>{
    const control = measured(8250);
    const pass = compareOutputWindows(control,measured(8750));
    assert.deepEqual(pass.fineP99Comparison.noStageIntervalMicros,[8001,8250]);
    assert.deepEqual(pass.fineP99Comparison.regressionIntervalMicros,[251,749]);
    assert.deepEqual(pass.fineP99Comparison.allowedRegressionIntervalMicros,[1000,1000]);
    assert.equal(pass.fineP99Comparison.decision,"pass");
    assert.equal(pass.boundedWindowGatePassed,true);
    const uncertain = compareOutputWindows(control,measured(9250));
    assert.deepEqual(uncertain.fineP99Comparison.regressionIntervalMicros,[751,1249]);
    assert.equal(uncertain.fineP99Comparison.decision,"indeterminate");
    assert.equal(uncertain.boundedWindowGatePassed,false);
    const fail = compareOutputWindows(control,measured(9750));
    assert.equal(fail.fineP99Comparison.decision,"fail");
    assert.equal(fail.boundedWindowGatePassed,false);
});
test("five percent threshold is itself interval bounded rather than chosen optimistically",()=>{
    const comparison = compareOutputWindows(measured(25000),measured(26250));
    assert.deepEqual(comparison.fineP99Comparison.allowedRegressionIntervalMicros,[24751*0.05,25000*0.05]);
    assert.equal(comparison.fineP99Comparison.decision,"indeterminate");
    assert.equal(comparison.boundedWindowGatePassed,false);
});
test("legacy coarse metrics and gate remain explicit; mixed availability never falls back",()=>{
    const old = output([0,100,0,0]);
    const fallback = compareOutputWindows(old,old);
    assert.equal(fallback.p99ComparisonSource,"coarse_histogram_fallback");
    assert.equal(fallback.fineP99Comparison.status,"unavailable");
    assert.equal(fallback.boundedWindowGatePassed,true);
    const mixed = compareOutputWindows(old,measured(8250));
    assert.equal(mixed.coarseBoundedWindowGatePassed,true);
    assert.equal(mixed.boundedWindowGatePassed,false);
    assert.equal(mixed.fineP99Comparison.status,"invalid");
});
test("fine tails keep wide precision and overflow never invents a bounded quantile",()=>{
    const tail = compareOutputWindows(measured(32000),measured(48000));
    assert.deepEqual(tail.fineP99Comparison.stageIntervalMicros,[32001,48000]);
    assert.equal(tail.fineP99Comparison.decision,"indeterminate");
    const overflow = compareOutputWindows(measured(Number.MAX_SAFE_INTEGER),measured(Number.MAX_SAFE_INTEGER));
    assert.deepEqual(overflow.fineP99Comparison.stageIntervalMicros,[1000001,null]);
    assert.equal(overflow.fineP99Comparison.decision,"indeterminate");
    assert.equal(overflow.boundedWindowGatePassed,false);
    const empty = compareOutputWindows(measured(8250,0),measured(8250,0));
    assert.equal(empty.boundedWindowGatePassed,false);
});

test("first-bucket and nearly one-millisecond fine decisions use actual integer intervals",()=>{
    const first = compareOutputWindows(measured(250),measured(250));
    assert.deepEqual(first.fineP99Comparison.noStageIntervalMicros,[0,250]);
    const near = compareOutputWindows(measured(8250),measured(9000));
    assert.deepEqual(near.fineP99Comparison.regressionIntervalMicros,[501,999]);
    assert.equal(near.boundedWindowGatePassed,true);
});
test("malformed fine data on a reset and invalid retained coverage never become zero precision",()=>{
    const accumulator = createOutputWindowAccumulator(measured(8250,10));
    const malformed = measured(8250,1);
    malformed.tick_duration_fine_histogram.counts="bad";
    const result = accumulator.take(malformed);
    assert.equal(result.counter_resets,1);
    assert.equal(result.fine_histogram_coverage.status,"invalid");
    const invalid = {...measured(8250),fine_histogram_coverage:{status:"invalid"}};
    assert.equal(compareOutputWindows(measured(8250),invalid).boundedWindowGatePassed,false);
});

test("fine precision may resolve a coarse bucket crossing without rewriting historical failure",()=>{
    const control = withFine({...output([100,0]),tick_duration_bucket_bounds_micros:[8000,12000]},[[8000,100]]);
    const stage = withFine({...output([0,100]),tick_duration_bucket_bounds_micros:[8000,12000]},[[8250,100]]);
    const comparison = compareOutputWindows(control,stage);
    assert.equal(comparison.noStageP99TickMicros,8000);
    assert.equal(comparison.stageP99TickMicros,12000);
    assert.equal(comparison.p99RegressionMicros,4000);
    assert.equal(comparison.coarseBoundedWindowGatePassed,false);
    assert.deepEqual(comparison.fineP99Comparison.regressionIntervalMicros,[1,499]);
    assert.equal(comparison.boundedWindowGatePassed,true);
    assert.equal(comparison.p99ComparisonSource,"fine_histogram_v1");
});

test("diagnostic minutes preserve aggregate counts and gate, including the final partial", () => {
	const start = measured(8250, 0);
	const plain = createOutputWindowAccumulator(start);
	const timed = createOutputWindowAccumulator(start, { diagnosticWindows: true, observedAt: 0 });
	for (let second = 1; second < 125; second++) {
		const sample = measured(8250, second * 40);
		plain.observe(sample, second * 1000);
		timed.observe(sample, second * 1000);
	}
	const expected = plain.take(measured(8250, 5000), 125_000);
	const { diagnostic_windows: detail, ...actual } = timed.take(measured(8250, 5000), 125_000);
	assert.deepEqual(actual, expected);
	assert.deepEqual(compareOutputWindows(measured(8000), actual), compareOutputWindows(measured(8000), expected));
	assert.equal(detail.coverageComplete, true);
	assert.deepEqual(detail.windows.map((row) => [row.startedAtEpochMillis, row.endedAtEpochMillis, row.finalPartial]),
		[[0, 60_000, false], [60_000, 120_000, false], [120_000, 125_000, true]]);
	assert.deepEqual(detail.windows.map((row) => row.observationCount), [61, 61, 6]);
	const sums = Array(fineBounds.length).fill(0);
	for (const row of detail.windows) row.fineHistogram.counts.forEach((count, index) => { sums[index] += count; });
	assert.deepEqual(sums, expected.tick_duration_fine_histogram.counts);
	assert.equal(detail.windows.reduce((sum, row) => sum + row.framesSent, 0), expected.frames_sent);
	assert.equal(detail.windows[2].durationMillis, 5000);
	const next = timed.take(measured(8250, 5040), 126_000).diagnostic_windows;
	assert.equal(next.windows[0].startedAtEpochMillis, 125_000);
	assert.equal(next.windows[0].framesSent, 40);
});

test("an irregular crossing stays whole at its actual observation boundary", () => {
	const accumulator = createOutputWindowAccumulator(measured(8250, 0), { diagnosticWindows: true, observedAt: 0 });
	accumulator.observe(measured(8250, 10), 59_000);
	accumulator.observe(measured(8250, 90), 65_000);
	const { diagnostic_windows: detail } = accumulator.take(measured(8250, 100), 66_000);
	assert.deepEqual(detail.windows.map((row) => [row.startedAtEpochMillis, row.endedAtEpochMillis, row.framesSent]),
		[[0, 65_000, 90], [65_000, 66_000, 10]]);
	assert.equal(detail.windows[0].maximumObservationGapMillis, 59_000);
	assert.equal(detail.windows[0].gapsAboveFiveSeconds, 2);
	assert.equal(detail.coverageComplete, false);
	assert.match(detail.partition, /no interpolation/);
});

test("reset and failed sample preserve available counts and explicitly lose diagnostic coverage", () => {
	const accumulator = createOutputWindowAccumulator(measured(8250, 10), { diagnosticWindows: true, observedAt: 1000 });
	accumulator.observe(measured(8250, 20), 2000);
	accumulator.observationFailed();
	accumulator.observe(measured(8250, 5), 4000);
	const result = accumulator.take({ ...measured(8250, 15), deadline_misses: 1, send_errors: 2 }, 5000);
	assert.equal(result.frames_sent, 25);
	assert.equal(result.counter_resets, 1);
	const row = result.diagnostic_windows.windows[0];
	assert.equal(row.fineHistogram.sample_count, 25);
	assert.equal(row.counterResets, 1);
	assert.equal(row.samplingErrors, 1);
	assert.equal(row.deadlineMisses, 1);
	assert.equal(row.sendErrors, 2);
	assert.equal(row.coverage.status, "unavailable");
	assert.match(row.coverage.reasons.join(" "), /before reset/);
	assert.equal(result.diagnostic_windows.coverageComplete, false);
});

test("missing clocks and missing fine schema stay unavailable rather than inventing zeros", () => {
	const accumulator = createOutputWindowAccumulator(output([0, 0, 0, 0]), { diagnosticWindows: true });
	accumulator.observe(output([0, 10, 0, 0]), Number.NaN);
	const row = accumulator.take(output([0, 20, 0, 0]), 1000).diagnostic_windows.windows[0];
	assert.equal(row.startedAtEpochMillis, null);
	assert.equal(row.fineHistogram, undefined);
	assert.equal(row.durationMillis, null);
	assert.equal(row.framesSent, 20);
	assert.equal(row.coverage.status, "unavailable");
	assert.ok(row.invalidClockIntervals > 0);
	assert.match(row.coverage.reasons.join(" "), /clock/);
	assert.match(row.coverage.reasons.join(" "), /fine histogram/);
});

test("diagnostic capacity bounds rows with explicit overflow without changing the aggregate", () => {
	const accumulator = createOutputWindowAccumulator(measured(8250, 0), {
		diagnosticWindows: true, observedAt: 0, maximumDiagnosticWindows: 2,
	});
	for (let minute = 1; minute <= 3; minute++) accumulator.observe(measured(8250, minute * 100), minute * 60_000);
	const result = accumulator.take(measured(8250, 400), 240_000);
	assert.equal(result.frames_sent, 400);
	assert.equal(result.tick_duration_fine_histogram.sample_count, 400);
	assert.equal(result.diagnostic_windows.windows.length, 2);
	assert.equal(result.diagnostic_windows.overflowWindows, 2);
	assert.equal(result.diagnostic_windows.firstOverflowEpochMillis, 120_000);
	assert.equal(result.diagnostic_windows.coverageComplete, false);
	assert.throws(() => createOutputWindowAccumulator(measured(8250), { diagnosticWindows: true, maximumDiagnosticWindows: 257 }), /capacity/);
});

test("reversed clocks cannot be reported as complete diagnostic timing", () => {
	const accumulator = createOutputWindowAccumulator(measured(8250, 0), { diagnosticWindows: true, observedAt: 10_000 });
	accumulator.observe(measured(8250, 10), 9000);
	const result = accumulator.take(measured(8250, 20), 11_000);
	assert.equal(result.frames_sent, 20);
	assert.equal(result.diagnostic_windows.windows[0].invalidClockIntervals, 1);
	assert.equal(result.diagnostic_windows.coverageComplete, false);
});

test("malformed fine schema remains an invalid diagnostic witness without fabricated counts", () => {
	const accumulator = createOutputWindowAccumulator(measured(8250, 0), { diagnosticWindows: true, observedAt: 0 });
	const malformed = measured(8250, 20);
	malformed.tick_duration_fine_histogram.schema_version = 2;
	const result = accumulator.take(malformed, 1000);
	const row = result.diagnostic_windows.windows[0];
	assert.equal(row.fineHistogram, undefined);
	assert.equal(row.fineHistogramCoverage.status, "invalid");
	assert.equal(row.coverage.status, "unavailable");
	assert.match(row.coverage.reasons.join(" "), /version/);
	assert.equal(result.fine_histogram_coverage.status, "invalid");
});

test("an empty diagnostic window does not masquerade as measured timing", () => {
	const snapshot = measured(8250, 0);
	const accumulator = createOutputWindowAccumulator(snapshot, { diagnosticWindows: true, observedAt: 1000 });
	const result = accumulator.take(snapshot, 2000);
	assert.equal(result.diagnostic_windows.coverageComplete, false);
	assert.match(result.diagnostic_windows.windows[0].coverage.reasons.join(" "), /no output ticks/);
	assert.equal(result.frames_sent, 0);
});

test("coarse bound or count-length drift invalidates diagnostics even with valid fine counts", () => {
	for (const change of [
		(sample) => { sample.tick_duration_bucket_bounds_micros[0] = 251; },
		(sample) => {
			sample.tick_duration_bucket_bounds_micros = [250, 2000];
			sample.tick_duration_bucket_counts = [0, 20];
		},
	]) {
		const initial = measured(8250, 10);
		const timed = createOutputWindowAccumulator(initial, { diagnosticWindows: true, observedAt: 0 });
		const legacy = createOutputWindowAccumulator(initial);
		const changed = measured(8250, 20);
		change(changed);
		const { diagnostic_windows: detail, ...actual } = timed.take(changed, 1000);
		assert.deepEqual(actual, legacy.take(changed, 1000), "legacy aggregation stays unchanged");
		assert.equal(detail.windows[0].fineHistogramCoverage.status, "complete");
		assert.equal(detail.windows[0].coarseGeometryDriftIntervals, 1);
		assert.equal(detail.windows[0].coverage.status, "unavailable");
		assert.equal(detail.coverageComplete, false);
		assert.match(detail.windows[0].coverage.reasons.join(" "), /coarse histogram geometry/);
	}
});

test("invalid clock with misses and errors retains counters and an unavailable time witness", () => {
	const accumulator = createOutputWindowAccumulator(measured(8250, 0), { diagnosticWindows: true, observedAt: 0 });
	const missed = { ...measured(8250, 10), deadline_misses: 1, send_errors: 2 };
	assert.doesNotThrow(() => accumulator.observe(missed, Number.NaN));
	const result = accumulator.take({ ...measured(8250, 20), deadline_misses: 1, send_errors: 2 }, 1000);
	assert.equal(result.frames_sent, 20);
	assert.equal(result.deadline_misses, 1);
	assert.equal(result.send_errors, 2);
	assert.deepEqual(result.late_intervals, [{ observedAt: null, clockAvailability: "unavailable",
		deadline_misses: 1, send_errors: 2, counter_reset: false }]);
	assert.equal(result.diagnostic_windows.coverageComplete, false);
	assert.equal(result.diagnostic_windows.windows[0].invalidClockIntervals, 2);
	assert.equal(result.diagnostic_windows.windows[0].deadlineMisses, 1);
	assert.equal(result.diagnostic_windows.windows[0].sendErrors, 2);
});
