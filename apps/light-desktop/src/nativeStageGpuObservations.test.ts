import { describe, expect, it } from "vitest";
import { NativeStageGpuObservations } from "./nativeStageGpuObservations";
import type { StagePaneBenchmarkSample } from "./platform/desktop/types";

function frame(overrides: Partial<StagePaneBenchmarkSample> = {}): StagePaneBenchmarkSample {
	return { paneId: "live", attachmentId: "attachment-1", sequence: 1, sourceFrame: 42,
		sourceInputEpochMicros: 1_000_000, presentedEpochMicros: 1_000_001,
		cpuMicros: 10, acquireMicros: 2, gpuMicros: 100, gpuSampleId: 1, gpuSampleMatchesFrame: true,
		instances: 300, drawCalls: 4, degraded: false, renderer: "native-helper",
		quality: "draft", mode: "shaded", followPreload: false, width: 1920, height: 1080,
		transport: "shared", ...overrides };
}

describe("NativeStageGpuObservations", () => {
	it("counts new helper submissions while source is held and credits completed GPU sample once", () => {
		const collector = new NativeStageGpuObservations();
		collector.record([frame(), frame({ sequence: 2 }), frame({ sequence: 3, gpuSampleId: 2, gpuMicros: 200 }), frame({ sequence: 3 })]);
		const report = collector.report();
		expect(report.counts.helperSubmissions).toBe(3);
		expect(report.counts.duplicateSubmissions).toBe(1);
		expect(report.counts.repeatedOrStaleGpuSamples).toBe(1);
		expect(report.rows[0]).toMatchObject({ helperSubmissionCount: 3, gpuSampleCount: 2, gpuSampleMicrosSum: 300, gpuSampleMicrosMax: 200, gpuSampleMicrosMean: 150 });
	});

	it("consumes out-of-scope samples so they cannot gain credit when scope changes", () => {
		const collector = new NativeStageGpuObservations();
		collector.record([frame({ gpuSampleMatchesFrame: false }), frame({ sequence: 2, quality: "ultra" }), frame({ sequence: 3, gpuSampleId: 2, gpuSampleMatchesFrame: undefined })]);
		const report = collector.report();
		expect(report.counts.rejectedGpuProvenance).toBe(2);
		expect(report.counts.repeatedOrStaleGpuSamples).toBe(1);
		expect(report.rows.every((row) => row.gpuSampleCount === 0)).toBe(true);
	});

	it("allows counters to reset for replacement attachments while rejecting resets within one attachment", () => {
		const collector = new NativeStageGpuObservations();
		collector.record([frame({ sequence: 50, gpuSampleId: 20 }), frame(), frame({ attachmentId: "attachment-2" }), frame({ paneId: "preload" })]);
		const report = collector.report();
		expect(report.counts.helperSubmissions).toBe(3);
		expect(report.counts.invalidFrames).toBe(1);
		expect(report.attachmentCount).toBe(3);
		expect(report.rows.map((row) => row.gpuSampleCount)).toEqual([1, 1, 1]);
	});

	it("separates second, pane, attachment, quality, mode, dimensions and transport", () => {
		const collector = new NativeStageGpuObservations();
		const changes: Partial<StagePaneBenchmarkSample>[] = [{}, { presentedEpochMicros: 2_000_000 }, { quality: "ultra" }, { mode: "lines" }, { width: 1280 }, { height: 720 }, { transport: "copy" }, { paneId: "preload" }, { attachmentId: "other" }];
		collector.record(changes.map((change, i) => frame({ sequence: i + 1, gpuSampleId: i + 1, ...change })));
		expect(collector.report().rows).toHaveLength(changes.length);
	});

	it("bounds attachment and row storage and retains explicit overflow counts", () => {
		const collector = new NativeStageGpuObservations({ attachments: 1, rows: 1 });
		collector.record([frame(), frame({ sequence: 2, presentedEpochMicros: 2_000_000, gpuSampleId: 2 }), frame({ attachmentId: "foreign" }), frame({ sequence: 3, gpuSampleId: 2 })]);
		const report = collector.report();
		expect(report.rows).toHaveLength(1);
		expect(report.attachmentCount).toBe(1);
		expect(report.counts).toMatchObject({ helperSubmissions: 3, attachmentOverflow: 1, rowOverflow: 1, repeatedOrStaleGpuSamples: 1 });
		expect(report.rows[0].gpuSampleCount).toBe(1);
	});

	it("reports malformed and unavailable timings explicitly without accepting historical timings", () => {
		const collector = new NativeStageGpuObservations();
		collector.record([frame({ width: 0 }), frame({ gpuSampleId: undefined, mode: undefined, transport: undefined }), frame({ sequence: 2, gpuMicros: Number.NaN }), frame({ sequence: 3, gpuSampleId: 2, gpuMicros: null })]);
		const report = collector.report();
		expect(report.counts).toMatchObject({ invalidFrames: 1, unavailableGpuSamples: 2, invalidGpuSamples: 1, unavailableMode: 1, unavailableTransport: 1 });
		expect(report.rows.every((row) => row.gpuSampleMicrosMean === null)).toBe(true);
		expect(report.measurementBoundaries.display).toContain("external trace");
		expect(report.measurementBoundaries.cpuMicros).toContain("wall time");
	});
});
