import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useDeskLoadingController } from "./useDeskLoadingController";

describe("useDeskLoadingController", () => {
	it("keeps an older operation visible after an overlapping newer one finishes", () => {
		const rendered = renderHook(() => useDeskLoadingController());
		let first = 0;
		let second = 0;
		act(() => {
			first = rendered.result.current.beginDeskLoading("First", "Preparing");
			second = rendered.result.current.beginDeskLoading("Second", "Hydrating");
		});
		expect(rendered.result.current.deskLoading?.title).toBe("Second");

		act(() => rendered.result.current.finishDeskLoading(second));
		expect(rendered.result.current.deskLoading?.title).toBe("First");

		act(() => rendered.result.current.finishDeskLoading(first));
		expect(rendered.result.current.deskLoading).toBeNull();
	});
});
