import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useAuxiliaryStagePerformance } from "./StageViewApp";

describe("auxiliary Stage performance", () => {
	afterEach(() => {
		vi.useRealTimers();
		vi.restoreAllMocks();
	});

	it("warms up in lines mode before promoting a focused window", () => {
		vi.useFakeTimers();
		vi.spyOn(document, "hasFocus").mockReturnValue(true);
		const { result } = renderHook(() => useAuxiliaryStagePerformance());

		expect(result.current).toEqual({
			renderQuality: "lines_only",
			visualizationIntervalMillis: 1_000,
			pixelRatioCap: 0.25,
		});

		act(() => vi.advanceTimersByTime(250));
		expect(result.current).toEqual({
			renderQuality: "lines_and_beams",
			visualizationIntervalMillis: 100,
			pixelRatioCap: 1.25,
		});

		act(() => window.dispatchEvent(new Event("blur")));
		expect(result.current).toEqual({
			renderQuality: "lines_only",
			visualizationIntervalMillis: 1_000,
			pixelRatioCap: 0.25,
		});

		act(() => window.dispatchEvent(new Event("focus")));
		expect(result.current).toEqual({
			renderQuality: "lines_and_beams",
			visualizationIntervalMillis: 100,
			pixelRatioCap: 1.25,
		});
	});
});
