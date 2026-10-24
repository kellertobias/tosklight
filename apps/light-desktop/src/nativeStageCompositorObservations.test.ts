import { expect, it } from "vitest";
import { NativeStageCompositorObservations } from "./nativeStageCompositorObservations";
import type { StageCompositorObservation } from "./platform/desktop/types";

function observation(attachmentId = "one", sampleId = 1): StageCompositorObservation {
	return { paneId: "live", attachmentId, observedEpochMicros: 2_000_000,
		measurement: { drawAttempts: 5, submissions: 3, occluded: 1, timeouts: 1,
			cleanSkips: 0, hiddenSkips: 0, busySkips: 0, pacedSkips: 0, retries: 1,
			reconfigures: 0, errors: 0, gpuReadbackErrors: 0, gpuInvalidSamples: 0, gpuSampleOverflow: 0,
			cpuWallMicros: 200, acquireWallMicros: 100, gpuTimingUnavailable: null,
			actualDisplayedFrames: null, measurementBoundary: "submission only",
			gpuSamples: [{ sampleId, submissionId: 3, submittedEpochMicros: 1_000_000,
				gpuMicros: 50, scope: { generation: 0, surfaceWidth: 1920, surfaceHeight: 1080,
					pane: [0, 0, 1920, 1080], sourceSize: [1920, 1080], transport: "shared" } }] } };
}

it("keeps compositor skips and work separate while rejecting repeated samples", () => {
	const collector = new NativeStageCompositorObservations();
	collector.record([observation(), observation(), observation("replacement")]);
	const report = collector.report();
	expect(report.totals).toMatchObject({ drawAttempts: 15, submissions: 9, occluded: 3, timeouts: 3, retries: 3 });
	expect(report.repeatedSamples).toBe(1);
	expect(report.rows).toHaveLength(2);
	expect(report.rows.map((row) => row.microsMean)).toEqual([50, 50]);
	expect(report.actualDisplayedFrames).toBeNull();
});

it("retains idle and hidden skip counts even when no GPU samples or submissions exist", () => {
	const collector = new NativeStageCompositorObservations();
	const idle = observation();
	Object.assign(idle.measurement, { drawAttempts: 100, submissions: 0, occluded: 0, timeouts: 0, retries: 0,
		cleanSkips: 100, cpuWallMicros: 30, acquireWallMicros: 0, gpuSamples: [] });
	const hidden = observation("hidden");
	Object.assign(hidden.measurement, { drawAttempts: 120, submissions: 0, occluded: 0, timeouts: 0, retries: 0,
		hiddenSkips: 110, busySkips: 4, pacedSkips: 6, cpuWallMicros: 40, acquireWallMicros: 0, gpuSamples: [] });
	collector.record([idle, hidden]);
	const report = collector.report();
	expect(report.totals).toMatchObject({ drawAttempts: 220, submissions: 0, cleanSkips: 100,
		hiddenSkips: 110, busySkips: 4, pacedSkips: 6, retries: 0, acquireWallMicros: 0, cpuWallMicros: 70 });
	expect(report.rows).toHaveLength(0);
	expect(report.invalid).toBe(0);
	expect(report.actualDisplayedFrames).toBeNull();
});

it("reports unavailable historical scheduling counters separately from measured zeros", () => {
	const collector = new NativeStageCompositorObservations();
	const historical = observation();
	for (const name of ["cleanSkips", "hiddenSkips", "busySkips", "pacedSkips", "retries"] as const) delete historical.measurement[name];
	collector.record([historical]);
	const report = collector.report();
	expect(report.unavailableCounters).toEqual({ cleanSkips: 1, hiddenSkips: 1, busySkips: 1, pacedSkips: 1, retries: 1 });
	expect(report.invalid).toBe(0);
});

it("rejects malformed skip counters without crediting submissions or displayed frames", () => {
	const collector = new NativeStageCompositorObservations();
	const malformed = observation();
	Object.assign(malformed.measurement, { cleanSkips: Number.NaN, hiddenSkips: -1 });
	collector.record([malformed]);
	const report = collector.report();
	expect(report.invalid).toBe(2);
	expect(report.totals.cleanSkips).toBe(0);
	expect(report.totals.hiddenSkips).toBe(0);
	expect(report.totals.submissions).toBe(3);
	expect(report.actualDisplayedFrames).toBeNull();
});

it("bounds attachment identities and reports overflow", () => {
	const collector = new NativeStageCompositorObservations();
	collector.record(Array.from({ length: 70 }, (_, i) => observation(String(i))));
	expect(collector.report().rows).toHaveLength(64);
	expect(collector.report().overflow).toBe(6);
});
