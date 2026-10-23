import { describe, expect, it, vi } from "vitest";
import { newBaselinePhaseReport, runBaselinePhases, validateBaselinePhases } from "./nativePackagedStagePhases";
import type { DesktopBridge, PackagedStageBenchmarkConfig } from "./platform/desktop/types";
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
