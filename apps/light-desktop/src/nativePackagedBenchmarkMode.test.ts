import { describe, expect, it } from "vitest";
import {
	NativeHostObservations,
	nativeBenchmarkObservationPolicy,
	nativeBenchmarkRetentionLimits,
	retainNativeHistory,
} from "./nativePackagedBenchmarkMode";
import type { PackagedStageBenchmarkConfig } from "./platform/desktop/types";

const config = (durationSeconds: number, extra: Partial<PackagedStageBenchmarkConfig> = {}) =>
	({ durationSeconds, ...extra }) as PackagedStageBenchmarkConfig;
const empty = { frames: [], heartbeats: [] };

describe("bounded native long-run observation policy", () => {
	it("retains normal, phase and capture polling semantics", () => {
		for (const seconds of [1, 30, 300, 1799])
			expect(nativeBenchmarkObservationPolicy(config(seconds)).pollingIntervalMillis).toBe(100);
		expect(nativeBenchmarkObservationPolicy(config(1800)).pollingIntervalMillis).toBe(250);
		expect(nativeBenchmarkObservationPolicy(config(3600)).pollingIntervalMillis).toBe(250);
		expect(nativeBenchmarkObservationPolicy(config(1800, { baselinePhases: "static-camera-hidden" })).pollingIntervalMillis).toBe(100);
		expect(nativeBenchmarkObservationPolicy(config(1800, { captureExercise: "live-ultra" })).pollingIntervalMillis).toBe(100);
		expect(nativeBenchmarkObservationPolicy(config(1800, { stageEnabled: false })).pollingIntervalMillis).toBe(1000);
	});

	it("rejects unsupported durations before collecting", () => {
		for (const duration of [0, -1, 3601, Infinity, NaN, 1.5])
			expect(() => nativeBenchmarkObservationPolicy(config(duration))).toThrow("1..3600");
	});

	it.each([1800, 1860, 3600])("retains real early and final timestamps for %i seconds", (seconds) => {
		const policy = nativeBenchmarkObservationPolicy(config(seconds));
		const host = new NativeHostObservations();
		const timeline: number[] = [];
		const activity: number[] = [];
		const start = 1_000_000;
		for (let elapsed = 0; elapsed <= seconds * 1000; elapsed += policy.pollingIntervalMillis) {
			host.record(empty, start + elapsed);
			expect(retainNativeHistory(timeline, start + elapsed, policy.retentionLimits.timeline)).toBe(true);
			for (let pane = 0; pane < 2; pane++)
				expect(retainNativeHistory(activity, start + elapsed, policy.retentionLimits.activity)).toBe(true);
		}
		expect(host.overflow).toBe(0);
		expect(host.rows[0].recordedAt).toBe(start);
		expect(host.rows.at(-1)?.recordedAt).toBe(start + seconds * 1000);
		expect(timeline.at(-1)).toBe(start + seconds * 1000);
		expect(timeline).toContain(start + 60_000);
		expect(timeline).toContain(start + seconds * 1000 - 300_000);
		if (seconds === 3600) {
			expect(host.rows.length).toBeGreaterThan(10_000);
			expect(activity.length).toBeGreaterThan(20_000);
		}
	});

	it("records every host overflow and retains the exact bounded prefix", () => {
		const host = new NativeHostObservations();
		for (let index = 0; index < nativeBenchmarkRetentionLimits.host; index++) host.record(empty, index);
		for (let rejected = 1; rejected <= 2; rejected++) {
			expect(() => host.record(empty, 999_999)).toThrow("overflow");
			expect(host.overflow).toBe(rejected);
		}
		expect(host.rows).toHaveLength(nativeBenchmarkRetentionLimits.host);
		expect(host.rows.at(-1)?.recordedAt).toBe(nativeBenchmarkRetentionLimits.host - 1);
	});

	it("rejects each history row at its exact cap without eviction or hidden replacement", () => {
		for (const limit of Object.values(nativeBenchmarkRetentionLimits)) {
			const rows: number[] = [];
			for (let index = 0; index < limit; index++) {
				if (!retainNativeHistory(rows, index, limit)) throw new Error("premature history rejection");
			}
			expect(retainNativeHistory(rows, -1, limit)).toBe(false);
			expect(retainNativeHistory(rows, -2, limit)).toBe(false);
			expect(rows).toHaveLength(limit);
			expect(rows[0]).toBe(0);
			expect(rows.at(-1)).toBe(limit - 1);
		}
	});
});
