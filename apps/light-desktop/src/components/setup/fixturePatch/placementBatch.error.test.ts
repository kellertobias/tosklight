import { describe, expect, it, vi } from "vitest";
import { commitPlacementBatch } from "./placementBatch";

describe("Patch placement errors", () => {
	it("shows the structured server cause instead of replacing it", async () => {
		const setStatus = vi.fn();
		const setBusy = vi.fn();
		const patchFixtures = vi
			.fn()
			.mockRejectedValue(
				new Error(
					"Internal fixture Audio Player 10 requires a regular positive fixture ID",
				),
			);

		await commitPlacementBatch(
			{
				ui: { setStatus, setBusy },
				patch: { patchFixtures },
			} as never,
			[{}] as never,
		);

		expect(setStatus).toHaveBeenCalledWith(
			"Internal fixture Audio Player 10 requires a regular positive fixture ID",
		);
		expect(setBusy.mock.calls).toEqual([[true], [false]]);
	});
});
