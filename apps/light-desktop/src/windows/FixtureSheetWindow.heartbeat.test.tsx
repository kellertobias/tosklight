import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const appendPackagedStageBenchmarkSample = vi.fn(
	async (_sample: Record<string, unknown>) => undefined,
);
const packagedStageBenchmarkConfig = vi.fn(async () => ({
	fixtureSheet: true,
	profile: "demo",
	expectedFixtureRecords: null as number | null,
}));

// One stable bridge, the way the real context hands the same object to every render.
const bridge = {
	appendPackagedStageBenchmarkSample,
	packagedStageBenchmarkConfig,
};

vi.mock("../platform/desktop", () => ({
	useDesktopBridge: () => bridge,
}));

import { useFixtureSheetBenchmarkReady } from "./FixtureSheetWindow";

/** A fresh array each call, the way a live visualization snapshot rebuilds the Sheet's rows. */
function rows(count: number) {
	return Array.from({ length: count }, (_, index) => ({
		parentFixtureId: `fixture-${index}`,
	}));
}

describe("Fixture Sheet benchmark heartbeat", () => {
	beforeEach(() => {
		vi.useFakeTimers();
		appendPackagedStageBenchmarkSample.mockReset().mockResolvedValue(undefined);
	});
	it("reports an exact fixture-count mismatch without declaring readiness", async () => {
		packagedStageBenchmarkConfig.mockResolvedValueOnce({
			fixtureSheet: true,
			profile: "demo",
			expectedFixtureRecords: 4,
		});
		const fixedRows = rows(3);
		renderHook(() =>
			useFixtureSheetBenchmarkReady({
				active: true,
				rows: fixedRows,
				activeValuesLoading: false,
				groupRuntimeLoading: false,
			}),
		);
		await act(async () => {
			await vi.advanceTimersByTimeAsync(0);
		});
		expect(appendPackagedStageBenchmarkSample).toHaveBeenCalledWith(
			expect.objectContaining({
				kind: "fixture-sheet-readiness",
				fixtureRecords: 3,
				expectedFixtureRecords: 4,
			}),
		);
		expect(
			appendPackagedStageBenchmarkSample.mock.calls.some(
				([sample]) => sample.kind === "fixture-sheet-ready",
			),
		).toBe(false);
	});

	it("records a failed ready append and preserves the absence of heartbeats until a later successful ready append", async () => {
		let fail = true;
		appendPackagedStageBenchmarkSample.mockImplementation(async (sample) => {
			if (sample.kind === "fixture-sheet-ready" && fail) {
				fail = false;
				throw new Error("report write failed");
			}
		});
		const view = renderHook(
			({ fixtures }) =>
				useFixtureSheetBenchmarkReady({
					active: true,
					rows: fixtures,
					activeValuesLoading: false,
					groupRuntimeLoading: false,
				}),
			{ initialProps: { fixtures: rows(3) } },
		);
		await act(async () => {
			await vi.advanceTimersByTimeAsync(0);
		});
		expect(appendPackagedStageBenchmarkSample).toHaveBeenCalledWith(
			expect.objectContaining({ kind: "fixture-sheet-ready-append-failed" }),
		);
		await act(async () => {
			await vi.advanceTimersByTimeAsync(1_000);
		});
		expect(
			appendPackagedStageBenchmarkSample.mock.calls.some(
				([sample]) => sample.kind === "fixture-sheet-heartbeat",
			),
		).toBe(false);
		view.rerender({ fixtures: rows(3) });
		await act(async () => {
			await vi.advanceTimersByTimeAsync(1_000);
		});
		expect(appendPackagedStageBenchmarkSample).toHaveBeenCalledWith(
			expect.objectContaining({ kind: "fixture-sheet-heartbeat" }),
		);
	});
	afterEach(() => {
		cleanup();
		vi.useRealTimers();
	});

	it.each([
		{
			active: false,
			activeValuesLoading: false,
			groupRuntimeLoading: false,
			count: 3,
		},
		{
			active: true,
			activeValuesLoading: true,
			groupRuntimeLoading: false,
			count: 3,
		},
		{
			active: true,
			activeValuesLoading: false,
			groupRuntimeLoading: true,
			count: 3,
		},
		{
			active: true,
			activeValuesLoading: false,
			groupRuntimeLoading: false,
			count: 0,
		},
	])("reports the actual blocked readiness predicates: %j", async ({
		count,
		...predicates
	}) => {
		renderHook(() =>
			useFixtureSheetBenchmarkReady({ ...predicates, rows: rows(count) }),
		);
		await act(async () => {
			await vi.advanceTimersByTimeAsync(0);
		});
		const samples = appendPackagedStageBenchmarkSample.mock.calls.map(
			([sample]) => sample,
		);
		const diagnostic = samples.find(
			(sample) => sample.kind === "fixture-sheet-readiness",
		);
		expect(diagnostic).toMatchObject({
			...predicates,
			rowCount: count,
			fixtureRecords: count,
			expectedFixtureRecords: null,
		});
		expect(
			samples.some((sample) => sample.kind === "fixture-sheet-ready"),
		).toBe(false);
	});

	it("keeps beating while live values rebuild the rows faster than it beats", async () => {
		const view = renderHook(
			(props: { rows: Array<{ parentFixtureId: string }> }) =>
				useFixtureSheetBenchmarkReady({
					active: true,
					rows: props.rows,
					activeValuesLoading: false,
					groupRuntimeLoading: false,
				}),
			{ initialProps: { rows: rows(3) } },
		);
		// Let the harness configuration resolve and the Sheet report itself ready.
		await vi.advanceTimersByTimeAsync(0);
		await vi.advanceTimersByTimeAsync(0);
		appendPackagedStageBenchmarkSample.mockClear();

		// The Sheet is alive and rendering; only its data identity keeps changing. Rebuild the
		// rows every 400ms across six seconds, well inside the one-second beat.
		for (let step = 0; step < 15; step += 1) {
			view.rerender({ rows: rows(3) });
			await vi.advanceTimersByTimeAsync(400);
		}

		// Six seconds of a living Sheet is six beats, give or take one for scheduling.
		expect(
			appendPackagedStageBenchmarkSample.mock.calls.filter(
				([sample]) => sample.kind === "fixture-sheet-heartbeat",
			).length,
		).toBeGreaterThanOrEqual(5);
	});

	it("keeps beating when the rows rebuild faster than once a second", async () => {
		const view = renderHook(
			(props: { rows: Array<{ parentFixtureId: string }> }) =>
				useFixtureSheetBenchmarkReady({
					active: true,
					rows: props.rows,
					activeValuesLoading: false,
					groupRuntimeLoading: false,
				}),
			{ initialProps: { rows: rows(3) } },
		);
		await vi.advanceTimersByTimeAsync(0);
		await vi.advanceTimersByTimeAsync(0);
		appendPackagedStageBenchmarkSample.mockClear();

		// Churn faster than the beat itself. This is the case that produced a long unbroken
		// silence in the release measurement: an interval rebuilt every 200ms never fires.
		for (let step = 0; step < 30; step += 1) {
			view.rerender({ rows: rows(3) });
			await vi.advanceTimersByTimeAsync(200);
		}

		expect(
			appendPackagedStageBenchmarkSample.mock.calls.filter(
				([sample]) => sample.kind === "fixture-sheet-heartbeat",
			).length,
		).toBeGreaterThanOrEqual(5);
	});
});
