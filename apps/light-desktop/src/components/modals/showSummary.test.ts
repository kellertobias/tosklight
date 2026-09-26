import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../api/types";
import { showPatchSummary } from "./showSummary";

describe("showPatchSummary", () => {
	it("counts patched splits and multi-patches across unique universes", () => {
		const fixture = {
			fixture_id: "fixture-1",
			universe: 1,
			address: 1,
			definition: {
				footprint: 6,
				mode_id: "mode-1",
				profile_snapshot: {
					modes: [{ id: "mode-1", splits: [{ number: 1, footprint: 2 }, { number: 2, footprint: 4 }] }],
				},
			},
			split_patches: [{ split: 2, universe: 2, address: 10 }],
			multipatch: [{ id: "copy", universe: 3, address: 20, split_patches: [] }],
		} as unknown as PatchedFixture;
		expect(showPatchSummary([fixture])).toEqual({ universes: 3, parameters: 8 });
	});
});
