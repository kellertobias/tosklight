import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const appendPackagedStageBenchmarkSample = vi.fn(async () => undefined);
const packagedStageBenchmarkConfig = vi.fn(async () => ({
	fixtureSheet: true,
	profile: "demo",
	expectedFixtureRecords: null,
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
		appendPackagedStageBenchmarkSample.mockClear();
	});
	afterEach(() => {
		vi.useRealTimers();
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
			appendPackagedStageBenchmarkSample.mock.calls.length,
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
			appendPackagedStageBenchmarkSample.mock.calls.length,
		).toBeGreaterThanOrEqual(5);
	});
});
