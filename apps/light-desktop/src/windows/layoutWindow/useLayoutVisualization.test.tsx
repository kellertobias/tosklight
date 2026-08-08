import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useLayoutVisualization } from "./useLayoutVisualization";

const mocks = vi.hoisted(() => ({
	read: vi.fn(),
}));

vi.mock("../../features/patch/PatchState", () => ({
	usePatchedFixturesView: () => [],
}));
vi.mock(
	"../../features/visualizationRuntime/VisualizationRuntimeView",
	() => ({
		useVisualizationRuntimeRead: () => mocks.read,
	}),
);
vi.mock("./fixturePresentation", () => ({
	fixturePresentation: vi.fn(),
}));

describe("useLayoutVisualization", () => {
	beforeEach(() => {
		vi.useFakeTimers();
		mocks.read.mockReset();
	});
	afterEach(() => vi.useRealTimers());

	it("never overlaps eventual-consistency reads when one response is slow", async () => {
		let finish: ((value: Record<string, unknown>) => void) | undefined;
		mocks.read.mockImplementation(
			() =>
				new Promise((resolve) => {
					finish = resolve;
				}),
		);
		renderHook(() => useLayoutVisualization(true, ["fixture-a"], 100));
		expect(mocks.read).toHaveBeenCalledTimes(1);

		await act(() => vi.advanceTimersByTimeAsync(1_000));
		expect(mocks.read).toHaveBeenCalledTimes(1);

		await act(async () => finish?.({ revision: 1, values: [] }));
		await act(() => vi.advanceTimersByTimeAsync(99));
		expect(mocks.read).toHaveBeenCalledTimes(1);
		await act(() => vi.advanceTimersByTimeAsync(1));
		expect(mocks.read).toHaveBeenCalledTimes(2);
	});
});
