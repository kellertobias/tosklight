import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useCadSelection } from "./useCadSelection";

const replaceSelection = vi.hoisted(() => vi.fn());
vi.mock("./session", () => ({
	cadSession: { replaceSelection, snapshot: vi.fn() },
}));

describe("the selection the patch sheet reveals", () => {
	beforeEach(() => {
		replaceSelection
			.mockReset()
			.mockImplementation((_revision: number, ids: readonly string[]) =>
				Promise.resolve({ revision: 2, selectedIds: ids }),
			);
	});

	it("reveals a selection made outside the sheet", () => {
		const { result } = renderHook(() => useCadSelection(vi.fn()));
		act(() => result.current.receive(["fixture-1"], 1));
		expect(result.current.selected).toEqual(["fixture-1"]);
		expect(result.current.revealRequest).toBe(1);
	});

	it("never reveals the sheet's own selection when the session echoes it", async () => {
		const { result } = renderHook(() => useCadSelection(vi.fn()));
		await act(async () => result.current.replace(["fixture-2", "fixture-1"]));
		expect(replaceSelection).toHaveBeenCalledWith(0, [
			"fixture-2",
			"fixture-1",
		]);

		act(() => result.current.receive(["fixture-1", "fixture-2"], 2));
		expect(result.current.revealRequest).toBe(0);

		act(() => result.current.receive(["fixture-3"], 3));
		expect(result.current.revealRequest).toBe(1);
	});

	it("reveals nothing when the selection is cleared", () => {
		const { result } = renderHook(() => useCadSelection(vi.fn()));
		act(() => result.current.receive([], 1));
		expect(result.current.revealRequest).toBe(0);
	});
});
