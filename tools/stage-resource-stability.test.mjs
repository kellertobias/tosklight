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
		{ renderer: "native" },
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
