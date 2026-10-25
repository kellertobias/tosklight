import assert from "node:assert/strict";
import test from "node:test";
import { summarizeStageLongRunResources } from "./stage-resource-stability.mjs";

function evidence({
	memoryGrowthPerMinute = 0,
	lateGeometries = 5,
	lateDrawCalls = 10,
} = {}) {
	const startedAt = 1_000_000;
	const processMemory = Array.from({ length: 1_801 }, (_, index) => ({
		recordedAt: startedAt + index * 1_000,
		phase: "stage",
		residentBytes: 100_000_000 + (memoryGrowthPerMinute * index) / 60,
	}));
	const timeline = Array.from({ length: 1_801 }, (_, index) => ({
		recordedAt: startedAt + index * 1_000,
		latestRender: {
			paneId: "live",
			recordedAt: startedAt + index * 1_000,
			submittedAt: index * 1_000,
			durationMs: 2,
			calls: index >= 1_500 ? lateDrawCalls : 10,
			transparentDrawCalls: 4,
			geometries: index >= 1_500 ? lateGeometries : 5,
			textures: 2,
		},
	}));
	return { processMemory, timeline };
}

test("passes stable 30-minute packaged resources", () => {
	const sample = evidence();
	const result = summarizeStageLongRunResources(
		sample.timeline,
		sample.processMemory,
		1_800,
	);

	assert.equal(result.enforced, true);
	assert.equal(result.passed, true);
	assert.deepEqual(result.failures, []);
});

test("rejects sustained memory and WebGL resource growth", () => {
	const sample = evidence({
		memoryGrowthPerMinute: 2 * 1_048_576,
		lateGeometries: 6,
	});
	const result = summarizeStageLongRunResources(
		sample.timeline,
		sample.processMemory,
		1_800,
	);

	assert.equal(result.passed, false);
	assert.match(result.failures.join("\n"), /memory grew faster/);
	assert.match(result.failures.join("\n"), /geometry or texture counts grew/);
});

test("keeps short development runs informational", () => {
	const result = summarizeStageLongRunResources([], [], 30);

	assert.equal(result.enforced, false);
	assert.equal(result.passed, null);
	assert.deepEqual(result.failures, []);
});

test("names native renderer evidence without WebGL terminology", () => {
	const result = summarizeStageLongRunResources([], [], 30, {
		renderer: "native",
	});

	assert.match(result.gpuFrameMeasurement, /Native helper frame telemetry/u);
});

test("rejects retained native draw-call ownership growth", () => {
	const sample = evidence({ lateDrawCalls: 11 });
	const result = summarizeStageLongRunResources(
		sample.timeline,
		sample.processMemory,
		1_800,
		{ renderer: "native", stageWindow: { startedAt: 1_000_000, completedAt: 2_800_000 } },
	);

	assert.equal(result.passed, false);
	assert.match(result.failures.join("\n"), /native instance or draw-call/u);
});

test("uses each newly recorded render instead of repeated latest snapshots", () => {
	const sample = evidence();
	sample.timeline = sample.timeline.map((entry, index) => ({
		...entry,
		latestRender: { ...entry.latestRender, durationMs: 100 },
		newRenders: [
			{
				...entry.latestRender,
				benchmarkSequence: index * 2 + 1,
				durationMs: 2,
			},
			{
				...entry.latestRender,
				benchmarkSequence: index * 2 + 2,
				durationMs: 3,
			},
		],
	}));

	const result = summarizeStageLongRunResources(
		sample.timeline,
		sample.processMemory,
		1_800,
	);

	assert.equal(result.passed, true);
	assert.equal(result.late.cpuFrameP95Ms, 3);
	assert.ok(result.late.samples > 300);
});

const nativeWindow = { startedAt: 1_000_000, completedAt: 2_800_000 };

test("native resource acceptance requires independent complete Stage window coverage", () => {
    const sample = evidence();
    const options = { renderer: "native", stageWindow: nativeWindow };
    assert.equal(summarizeStageLongRunResources(sample.timeline, sample.processMemory, 1800, options).passed, true);
    // This reproduces the old collector stopping after its host-row cap. Fitting a stable
    // partial prefix and calling that prefix's last five minutes 'late' must not pass.
    const truncated = sample.timeline.slice(0, 961);
    const bad = summarizeStageLongRunResources(truncated, sample.processMemory, 1800, options);
    assert.equal(bad.passed, false);
    assert.match(bad.failures.join("\n"), /render coverage/u);
    const missingRss = summarizeStageLongRunResources(sample.timeline, sample.processMemory.slice(0, 961), 1800, options);
    assert.equal(missingRss.passed, false);
    assert.match(missingRss.failures.join("\n"), /RSS coverage/u);
    assert.equal(summarizeStageLongRunResources(sample.timeline, sample.processMemory, 1800, {renderer: "native"}).passed, false);
});

test("late observation clocks cannot refresh stale native render timestamps", () => {
    const sample = evidence();
    const timeline = sample.timeline.map((entry, index) => ({...entry, newRenders: [{
        ...entry.latestRender, benchmarkSequence: index, recordedAt: Math.min(entry.recordedAt, 1_960_000),
    }]}));
    const result = summarizeStageLongRunResources(timeline, sample.processMemory, 1800, {renderer: "native", stageWindow: nativeWindow});
    assert.equal(result.passed, false);
    assert.match(result.failures.join("\n"), /render coverage/u);
});


test("native late window stays anchored to intended Stage end but rejects sparse ending coverage", () => {
    const sample = evidence();
    const timeline = sample.timeline.filter((entry) => entry.recordedAt < 2_500_000 || entry.recordedAt === nativeWindow.completedAt);
    const result = summarizeStageLongRunResources(timeline, sample.processMemory, 1800, {renderer: "native", stageWindow: nativeWindow});
    assert.equal(result.passed, false);
    assert.match(result.failures.join("\n"), /render coverage/u);
    assert.equal(result.early.samples, 300);
    assert.equal(result.late.samples, 1);
    assert.equal(result.coverage.renders.lastRecordedAt, nativeWindow.completedAt);
});

test("native snapshots require actual render clocks and cannot refresh a repeated image", () => {
    const sample = evidence();
    const repeated = sample.timeline.map((entry) => ({...entry, latestRender: {
        recordedAt: nativeWindow.startedAt, submittedAt: nativeWindow.startedAt,
        benchmarkSequence: "same-image", durationMs: 2, calls: 10, geometries: 5,
    }}));
    const stale = summarizeStageLongRunResources(repeated, sample.processMemory, 1800, {renderer: "native", stageWindow: nativeWindow});
    assert.equal(stale.passed, false);
    assert.equal(stale.coverage.renders.lastRecordedAt, nativeWindow.startedAt);
    const noClock = sample.timeline.map((entry) => ({...entry, latestRender: {
        benchmarkSequence: entry.recordedAt, durationMs: 2, calls: 10, geometries: 5,
    }}));
    const missing = summarizeStageLongRunResources(noClock, sample.processMemory, 1800, {renderer: "native", stageWindow: nativeWindow});
    assert.equal(missing.passed, false);
    assert.match(missing.failures.join("\n"), /render coverage/u);
});

test("native endpoint tolerance does not accept late startup or incomplete intended duration", () => {
    const sample = evidence();
    const lateStart = sample.timeline.slice(6);
    const late = summarizeStageLongRunResources(lateStart, sample.processMemory, 1800, {renderer: "native", stageWindow: nativeWindow});
    assert.equal(late.passed, false);
    assert.match(late.failures.join("\n"), /render coverage/u);
    const short = summarizeStageLongRunResources(sample.timeline, sample.processMemory, 1800, {renderer: "native", stageWindow: {...nativeWindow, completedAt: nativeWindow.completedAt - 10_000}});
    assert.equal(short.passed, false);
    assert.match(short.failures.join("\n"), /independent complete Stage window/u);
});


test("native submission epoch is accepted when recordedAt is absent and short runs stay informational", () => {
    const sample = evidence();
    const timeline = sample.timeline.map((entry) => {
        const {recordedAt: unused, ...render} = entry.latestRender;
        return {...entry, latestRender: {...render, submittedAt: entry.recordedAt}};
    });
    const valid = summarizeStageLongRunResources(timeline, sample.processMemory, 1800, {renderer: "native", stageWindow: nativeWindow});
    assert.equal(valid.passed, true);
    const short = summarizeStageLongRunResources(timeline, sample.processMemory, 30, {renderer: "native"});
    assert.equal(short.passed, null);
    assert.equal(short.failures.length, 0);
    assert.ok(short.late.samples > 0);
});


test("native independent duration tolerance is explicit and bounded to five seconds", () => {
    const sample = evidence();
    const result = summarizeStageLongRunResources(sample.timeline, sample.processMemory, 1800, {
        renderer: "native", stageWindow: {...nativeWindow, completedAt: nativeWindow.completedAt - 5_000},
    });
    assert.equal(result.passed, true);
    assert.equal(result.coverage.actualDurationMillis, 1_795_000);
    assert.equal(result.coverage.requestedDurationMillis, 1_800_000);
    assert.equal(result.coverage.durationToleranceMillis, 5_000);
});

test("enforced resource acceptance rejects unavailable late CPU p95", () => {
    const sample = evidence();
    const timeline = sample.timeline.map((entry) => ({...entry, latestRender: {...entry.latestRender, durationMs: null}}));
    for (const options of [{renderer: "webgl"}, {renderer: "native", stageWindow: nativeWindow}]) {
        const result = summarizeStageLongRunResources(timeline, sample.processMemory, 1800, options);
        assert.equal(result.passed, false);
        assert.match(result.failures.join("\n"), /CPU submission p95 is unavailable/u);
    }
});


test("each observed native pane must independently cover the complete animated Stage window", () => {
    const sample = evidence();
    const dual = sample.timeline.map((entry, index) => ({...entry, newRenders: [
        {...entry.latestRender, benchmarkSequence: `live:${index}`},
        ...(index <= 900 ? [{...entry.latestRender, paneId: "preload", benchmarkSequence: `preload:${index}`}] : []),
    ]}));
    const result = summarizeStageLongRunResources(dual, sample.processMemory, 1800, {renderer: "native", stageWindow: nativeWindow});
    assert.equal(result.passed, false);
    assert.equal(result.coverage.renders.complete, true);
    assert.equal(result.coverage.panes.length, 2);
    assert.equal(result.coverage.panes.find((pane) => pane.paneId === "preload").complete, false);
    assert.match(result.failures.join("\n"), /render coverage is incomplete for pane preload/u);
});

test("native RSS and each lane reject internal gaps above the declared tolerance", () => {
    const sample = evidence();
    const gapMemory = sample.processMemory.filter((entry, index) => index < 500 || index > 507);
    const rss = summarizeStageLongRunResources(sample.timeline, gapMemory, 1800, {renderer: "native", stageWindow: nativeWindow});
    assert.equal(rss.passed, false);
    assert.equal(rss.coverage.rss.maximumGapMillis, 9_000);
    assert.match(rss.failures.join("\n"), /RSS coverage/u);
    const dual = sample.timeline.map((entry, index) => ({...entry, newRenders: [
        {...entry.latestRender, benchmarkSequence: `live:${index}`},
        ...(index < 500 || index > 507 ? [{...entry.latestRender, paneId: "preload", benchmarkSequence: `preload:${index}`}] : []),
    ]}));
    const lane = summarizeStageLongRunResources(dual, sample.processMemory, 1800, {renderer: "native", stageWindow: nativeWindow});
    assert.equal(lane.passed, false);
    assert.equal(lane.coverage.renders.complete, true);
    assert.equal(lane.coverage.panes.find((pane) => pane.paneId === "preload").maximumGapMillis, 9_000);
    assert.match(lane.failures.join("\n"), /render coverage is incomplete for pane preload/u);
});
