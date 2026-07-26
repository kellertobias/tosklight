import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FrontendSnapshotRequestDiagnostic } from "./diagnostics";

describe("frontend performance diagnostics", () => {
	beforeEach(() => {
		vi.resetModules();
		performance.clearMarks();
		performance.clearMeasures();
	});

	it("records snapshot count, serialized bytes, concurrency, and User Timing", async () => {
		const { frontendPerformanceDiagnostics: diagnostics } = await import(
			"./diagnostics"
		);
		const finishA = diagnostics.beginSnapshotRequest("show-objects:group");
		const finishB = diagnostics.beginSnapshotRequest("playback:runtime");
		finishA({ objects: [{ id: "1" }] });
		finishB(undefined, new Error("offline"));

		expect(diagnostics.snapshot()).toMatchObject({
			snapshotRequestCount: 2,
			maxSnapshotConcurrency: 2,
		});
		expect(diagnostics.snapshot().snapshotPayloadBytes).toBeGreaterThan(0);
		expect(
			performance.getEntriesByName(
				"tosklight:snapshot:show-objects:group:1",
				"measure",
			),
		).toHaveLength(1);
	});

	it("accounts for retained Map and Set contents", async () => {
		const { serializedModelBytes } = await import("./diagnostics");
		expect(
			serializedModelBytes({
				projections: new Map([["playback:1", { master: 1 }]]),
				ready: new Set(["group"]),
			}),
		).toBeGreaterThan(serializedModelBytes({ projections: [], ready: [] }));
	});

	it("publishes a test-visible immutable snapshot", async () => {
		await import("./diagnostics");
		const first = window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.snapshot();
		expect(first).toBeDefined();
		(
			first?.snapshotRequests as FrontendSnapshotRequestDiagnostic[] | undefined
		)?.push({
			feature: "forged",
			startedAt: 0,
			finishedAt: 0,
			payloadBytes: 0,
			status: "ready",
		});
		const second = window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.snapshot();
		expect(second?.snapshotRequests).toEqual([]);
	});

	it("records bounded event receipt lag from the authoritative envelope time", async () => {
		const { frontendPerformanceDiagnostics: diagnostics } = await import(
			"./diagnostics"
		);
		diagnostics.recordEventReceipt("show-objects", {
			event: { occurred_at: new Date(Date.now() - 25).toISOString() },
		});
		expect(diagnostics.snapshot().eventLags[0]).toMatchObject({
			feature: "show-objects",
		});
		expect(diagnostics.snapshot().eventLags[0].lagMs).toBeGreaterThanOrEqual(0);
	});

	it("records Patch action, authority, and visible-paint timing as informational evidence", async () => {
		const { frontendPerformanceDiagnostics: diagnostics } = await import(
			"./diagnostics"
		);
		const patch = diagnostics.beginPatchMutation("patch-1", 100);
		patch.optimisticStorePublished();
		patch.responseDecoded();
		patch.authoritativeStorePublished();
		patch.visiblePainted();

		expect(diagnostics.snapshot().patchMutations).toEqual([
			expect.objectContaining({
				requestId: "patch-1",
				fixtureCount: 100,
				actionToVisibleMs: expect.any(Number),
			}),
		]);
		expect(diagnostics.snapshot().patchActionToVisible).toMatchObject({
			samples: 1,
			p50Ms: expect.any(Number),
			p95Ms: expect.any(Number),
			gateEnforced: false,
		});
		expect(
			performance.getEntriesByName("tosklight:patch:patch-1", "measure"),
		).toHaveLength(1);
	});
});
