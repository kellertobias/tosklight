import { describe, expect, it, vi } from "vitest";
import { recordStaticCompositorBatch, hiddenCompositorIntervals, newBaselinePhaseReport, runBaselinePhases, validateBaselinePhases } from "./nativePackagedStagePhases";
import type { DesktopBridge, PackagedStageBenchmarkConfig, StageCompositorObservation } from "./platform/desktop/types";
const config = { baselinePhases: "static-camera-hidden", baselineLayout: "dual", stageEnabled: true, profile: "stage-500", durationSeconds: 40 } as PackagedStageBenchmarkConfig;
const identities = [{ paneId: "main:a", attachmentId: "1" }, { paneId: "main:b", attachmentId: "2" }];

describe("opt-in fixed baseline phases", () => {
	it("rejects truncated/nonisolated phase configuration", () => {
		expect(() => validateBaselinePhases({ ...config, durationSeconds: 39 })).toThrow();
		expect(() => validateBaselinePhases({ ...config, baselineLayout: null })).toThrow();
		expect(() => validateBaselinePhases({ ...config, profile: "canonical-demo" })).toThrow();
		expect(() => validateBaselinePhases({ ...config, stageEnabled: false })).toThrow();
		expect(() => validateBaselinePhases({ ...config, baselinePhases: undefined })).not.toThrow();
	});
	it("retains resident attachments and actual phase timestamps without claiming measured savings", async () => {
		let clock = 1_000_000;
		let cameraX = 0;
		const consumed = vi.fn(async (_batches: unknown) => undefined);
		const before = vi.fn(async () => undefined);
		const after = vi.fn();
		const send = vi.fn(async (_gesture: string, _x: number, _y: number, _paneId: string) => { cameraX++; });
		const desktop = {
			stagePaneCamera: async () => [cameraX, 0, 0, 0, 0, 1], sendStagePaneInput: send,
			exercisePackagedStageBenchmarkVisibility: async () => {
				const hiddenEpochMicros = clock * 1_000; clock += 10_000;
				return { hiddenEpochMicros, revealedEpochMicros: clock * 1_000, hiddenVerified: true, revealedVerified: true, observations: [] };
			},
		} as unknown as DesktopBridge;
		const report = newBaselinePhaseReport();
		await runBaselinePhases({ config, desktop, report, panes: () => identities, signal: new AbortController().signal,
			now: () => clock, sleep: async (millis) => { clock += millis; }, beforeHidden: before, collectHidden: consumed, afterHidden: after });
		expect(report.status).toBe("completed");
		expect(report.rows.map((row) => [row.phase, row.finishedEpochMillis! - row.startedEpochMillis])).toEqual([
			["static", 10_000], ["camera", 10_000], ["hidden", 10_000], ["revealed", 10_000],
		]);
		expect(report.rows[1].cameraBefore).not.toEqual(report.rows[1].cameraAfter);
		expect(send.mock.calls.every((call) => ["a", "b"].includes(call[3]))).toBe(true);
		expect(report.attachmentsBeforeHidden).toEqual(report.attachmentsAfterReveal);
		expect(before).toHaveBeenCalledOnce(); expect(consumed).toHaveBeenCalledWith([]); expect(after).toHaveBeenCalledOnce();
		expect(report.measurementStatus).toBe("pending");
		expect(report.nativeVisibility).not.toHaveProperty("observations");
	});
	it("leaves an explicit failure and resumes frontend draining if native collection fails", async () => {
		let clock = 0;
		const after = vi.fn();
		const desktop = { stagePaneCamera: async () => [0, 0, 0, 0, 0, 1], sendStagePaneInput: async () => {},
			exercisePackagedStageBenchmarkVisibility: async () => { throw new Error("bounded collection overflowed"); },
		} as unknown as DesktopBridge;
		const report = newBaselinePhaseReport();
		await runBaselinePhases({ config, desktop, report, panes: () => identities, signal: new AbortController().signal,
			now: () => clock, sleep: async (millis) => { clock += millis; }, afterHidden: after });
		expect(report.status).toBe("failed"); expect(report.error).toContain("overflowed");
		expect(report.rows.at(-1)?.status).toBe("failed"); expect(after).toHaveBeenCalledOnce();
	});
});

it("keeps hidden compositor attempts, submissions, occlusion and acquire cost separate from helper work", async () => {
	let clock = 1_000_000;
	const totals = { drawAttempts: 11, submissions: 7, occluded: 4, acquireWallMicros: 90 };
	const desktop = {
		stagePaneCamera: async () => [clock, 0, 0, 0, 0, 1], sendStagePaneInput: async () => {},
		exercisePackagedStageBenchmarkVisibility: async () => {
			const hiddenEpochMicros = clock * 1_000;
			clock += 10_000;
			return { hiddenEpochMicros, revealedEpochMicros: clock * 1_000, hiddenVerified: true, revealedVerified: true,
				observations: [{ observedEpochMicros: clock * 1_000, windowVisible: false, samples: { frames: [], heartbeats: [] } }] };
		},
	} as unknown as DesktopBridge;
	const report = newBaselinePhaseReport();
	await runBaselinePhases({ config, desktop, report, panes: () => identities, signal: new AbortController().signal,
		now: () => clock, sleep: async (millis) => { clock += millis; },
		compositorSnapshot: () => ({ totals: { ...totals }, observedEpochMillis: clock - 80 }),
		collectHidden: async () => { totals.drawAttempts += 1234; totals.submissions += 12; totals.occluded += 1222; totals.acquireWallMicros += 4567; },
	});
	expect(report.rows[2].compositorCounters?.values).toEqual({ drawAttempts: 1234, submissions: 12, occluded: 1222, acquireWallMicros: 4567 });
	expect(report.rows[0].compositorCounters?.values.drawAttempts).toBe(0);
	expect(report.rows[2].compositorCounters).toMatchObject({ nominalPollMillis: 100, beforeArrivalLagMillis: 80, afterArrivalLagMillis: 80 });
	expect(report.rows[2].compositorCounters?.boundary).toContain("not arrival");
});

it("labels animated hidden/resume evidence separately from static idling", async () => {
	let clock = 1_000_000;
	const desktop = {
		stagePaneCamera: async () => [clock, 0, 0, 0, 0, 1], sendStagePaneInput: async () => {},
		exercisePackagedStageBenchmarkVisibility: async () => {
			const hiddenEpochMicros = clock * 1_000; clock += 10_000;
			return { hiddenEpochMicros, revealedEpochMicros: clock * 1_000, hiddenVerified: true, revealedVerified: true, observations: [] };
		},
	} as unknown as DesktopBridge;
	const report = newBaselinePhaseReport();
	await runBaselinePhases({ config: { ...config, baselinePhases: "animated-camera-hidden" }, desktop, report,
		panes: () => identities, signal: new AbortController().signal,
		now: () => clock, sleep: async (millis) => { clock += millis; } });
	expect(report.status).toBe("completed");
	expect(report.rows.map((row) => row.phase)).toEqual(["animated", "camera", "hidden", "revealed"]);
});


function compositorObservation(observedEpochMicros: number, submissions = 0): StageCompositorObservation {
	return { paneId: "main:a", attachmentId: "1", observedEpochMicros,
		measurement: { drawAttempts: 5, submissions, occluded: 0, timeouts: 0, reconfigures: 0, errors: 0,
			cpuWallMicros: 20, acquireWallMicros: submissions * 10, cleanSkips: 0, hiddenSkips: 5 - submissions,
			busySkips: 0, pacedSkips: 0, gpuSamples: [] },
	} as unknown as StageCompositorObservation;
}
it("retains native drain intervals with unknown first start instead of assigning edge counters to hidden steady state", () => {
	const batches = [1_000_000, 1_100_000, 1_200_000].map((time, index) => ({ observedEpochMicros: time + 10,
		windowVisible: false as const, samples: { frames: [], heartbeats: [], compositors: [compositorObservation(time, index === 0 ? 2 : 0)] } }));
	const evidence = hiddenCompositorIntervals(batches, identities);
	expect(evidence.invalid).toBe(0);
	expect(evidence.rows.map((row) => [row.startObservedEpochMicros, row.endObservedEpochMicros])).toEqual([
		[null, 1_000_000], [1_000_000, 1_100_000], [1_100_000, 1_200_000],
	]);
	expect(evidence.rows[0].counters.submissions).toBe(2);
	expect(evidence.rows[1].potentialAcquireAttemptsUpperBound).toBe(0);
	expect(evidence.rows[0]).not.toHaveProperty("gpuSamples");
});
it("bounds interval storage and exposes invalid counter/timestamp inputs", () => {
	const batches = Array.from({ length: 300 }, (_, index) => ({ observedEpochMicros: (index + 1) * 100_000,
		windowVisible: false as const, samples: { frames: [], heartbeats: [], compositors: [compositorObservation((index + 1) * 100_000)] } }));
	const evidence = hiddenCompositorIntervals(batches, identities);
	expect(evidence.rows).toHaveLength(240);
	expect(evidence.overflow).toBe(60);
	const bad = compositorObservation(2_000_000);
	bad.measurement.drawAttempts = -1;
	expect(hiddenCompositorIntervals([{ observedEpochMicros: 2_000_010, windowVisible: false, samples: { frames: [], heartbeats: [], compositors: [bad] } }], identities).invalid).toBe(1);
	expect(hiddenCompositorIntervals([batches[1], batches[0]], identities).invalid).toBe(1);
});

it("records existing consumed batches only during static and bounds compact evidence", () => {
	const report = newBaselinePhaseReport();
	report.attachmentsDuringStatic = identities;
	report.staticCompositorIntervals = { rows: [], overflow: 0, invalid: 0, limit: 240, boundary: "test" };
	const row = { phase: "static" as const, status: "pending" as "pending" | "completed", startedEpochMillis: 1_000, finishedEpochMillis: null };
	report.rows.push(row);
	const batch = (time: number) => ({ frames: [], heartbeats: [], compositors: [compositorObservation(time)] });
	recordStaticCompositorBatch(report, batch(1_100_123), 1_101);
	recordStaticCompositorBatch(report, batch(1_200_123), 1_201);
	expect(report.staticCompositorIntervals.rows.map((entry) => entry.startObservedEpochMicros)).toEqual([null, 1_100_123]);
	row.status = "completed";
	recordStaticCompositorBatch(report, batch(1_300_123), 1_301);
	expect(report.staticCompositorIntervals.rows).toHaveLength(2);
	row.status = "pending";
	for (let index = 0; index < 250; index++) recordStaticCompositorBatch(report, batch(2_000_000 + index * 100_000), 2_001 + index * 100);
	expect(report.staticCompositorIntervals.rows).toHaveLength(240);
	expect(report.staticCompositorIntervals.overflow).toBe(12);
	expect(report.staticCompositorIntervals.rows[0]).not.toHaveProperty("gpuSamples");
});

it("discovers complete static identities from consumed native batches after initially empty panes", async () => {
	let clock = 1_000_000;
	const report = newBaselinePhaseReport();
	const abort = new AbortController();
	const desktop = {} as DesktopBridge;
	const batch = (time: number, dual = true) => ({ frames: [], heartbeats: [], compositors:
		(dual ? identities : identities.slice(0, 1)).map((pane) => ({ ...compositorObservation(time), ...pane })) });
	await runBaselinePhases({ config, desktop, report, panes: () => [], signal: abort.signal,
		now: () => clock, sleep: async () => {
			expect(report.attachmentsDuringStatic).toEqual([]);
			recordStaticCompositorBatch(report, batch(clock * 1_000, false), clock);
			expect(report.staticCompositorIntervals?.rows).toHaveLength(0);
			expect(report.staticIdentityDiscovery?.lockedAtObservedEpochMicros).toBeNull();
			clock += 300;
			recordStaticCompositorBatch(report, batch(clock * 1_000), clock);
			expect(report.attachmentsDuringStatic).toEqual(identities);
			expect(report.staticCompositorIntervals?.rows.every((row) => row.startObservedEpochMicros === null)).toBe(true);
			clock += 100;
			recordStaticCompositorBatch(report, batch(clock * 1_000), clock);
			expect(report.staticCompositorIntervals?.rows.filter((row) => row.startObservedEpochMicros !== null)).toHaveLength(2);
			const replaced = batch((clock + 100) * 1_000);
			replaced.compositors[0].attachmentId = "replacement";
			recordStaticCompositorBatch(report, replaced, clock + 100);
			expect(report.staticCompositorIntervals?.invalid).toBe(1);
			expect(report.attachmentsDuringStatic).toEqual(identities);
			throw new Error("stop after bounded static discovery regression");
		} });
	expect(report.staticIdentityDiscovery?.pendingBatches).toBe(1);
});
