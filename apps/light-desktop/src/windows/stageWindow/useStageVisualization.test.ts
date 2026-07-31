import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../api/types";
import { stageFixtures3d } from "./useStageVisualization";

describe("stageFixtures3d", () => {
	it("preserves an explicit zero location instead of replacing it with fallback geometry", () => {
		const fixture = {
			fixture_id: "zero-location",
			fixture_number: 1,
			logical_heads: [],
			location: { x: 0, y: 0, z: 0 },
			rotation: { x: 0, y: 0, z: 0 },
			multipatch: [],
		} as unknown as PatchedFixture;

		expect(
			stageFixtures3d([fixture], { positions: {}, positions3d: {} })[0]
				.position,
		).toMatchObject({ x: 0, y: 0, z: 0 });
	});
});
