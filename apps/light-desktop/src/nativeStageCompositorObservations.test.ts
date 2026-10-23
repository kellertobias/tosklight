import { expect, it } from "vitest";
import { NativeStageCompositorObservations } from "./nativeStageCompositorObservations";
import type { StageCompositorObservation } from "./platform/desktop/types";

function observation(attachmentId = "one", sampleId = 1): StageCompositorObservation {
	return { paneId: "live", attachmentId, observedEpochMicros: 2_000_000,
		measurement: { drawAttempts: 5, submissions: 3, occluded: 1, timeouts: 1,
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
	expect(report.totals).toMatchObject({ drawAttempts: 15, submissions: 9, occluded: 3, timeouts: 3 });
	expect(report.repeatedSamples).toBe(1);
	expect(report.rows).toHaveLength(2);
	expect(report.rows.map((row) => row.microsMean)).toEqual([50, 50]);
	expect(report.actualDisplayedFrames).toBeNull();
});

it("bounds attachment identities and reports overflow", () => {
	const collector = new NativeStageCompositorObservations();
	collector.record(Array.from({ length: 70 }, (_, i) => observation(String(i))));
	expect(collector.report().rows).toHaveLength(64);
	expect(collector.report().overflow).toBe(6);
});
