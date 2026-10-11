import { describe, expect, it, vi } from "vitest";
import { plannedDemoDynamicDefinitions } from "../../support/plannedDemoDynamics";
import {
	installPlannedDemoSoloRegions,
	PLANNED_DEMO_SOLO_REGIONS,
} from "../../support/plannedDemoSoloRegions";

describe("Plan 76 unified Playback Solo regions", () => {
	it("keeps Beam Show intensity and movement effects mutually exclusive", async () => {
		const request = vi
			.fn()
			.mockResolvedValueOnce({ revision: 7 })
			.mockResolvedValueOnce({ changed: true });
		await installPlannedDemoSoloRegions(
			{ request } as never,
			"00000000-0000-4000-8000-000000000001",
		);

		const definitions = plannedDemoDynamicDefinitions();
		const definitionForPlayback = (playbackNumber: number) =>
			definitions.find(
				(definition) => definition.pool_number === playbackNumber - 400,
			);
		expect(PLANNED_DEMO_SOLO_REGIONS).toEqual([
			expect.objectContaining({
				name: "Beam Show Intensity Effects",
				members: [401, 402, 403].map((position) => ({ page: 1, position })),
			}),
			expect.objectContaining({
				name: "Beam Show Movement Effects",
				members: [419, 420].map((position) => ({ page: 1, position })),
			}),
		]);
		const intensityDefinitions = [401, 402, 403].map(definitionForPlayback);
		expect(
			intensityDefinitions.every(
				(definition) =>
					definition?.target_binding.group_id === "4" &&
					definition.lanes.every(
						(lane: { attribute: string }) => lane.attribute === "intensity",
					),
			),
		).toBe(true);
		const movementDefinitions = [419, 420].map(definitionForPlayback);
		expect(
			movementDefinitions.every(
				(definition) =>
					definition?.target_binding.group_id === "4" &&
					// TL-552: movement lanes are typed Angles Programming lanes.
					definition.lanes.some((lane: any) =>
						["pan", "tilt"].includes(
							lane.programming?.address?.component?.kind,
						),
					),
			),
		).toBe(true);
		expect(request).toHaveBeenLastCalledWith(
			"POST",
			"/api/v2/playback-solo-regions/update",
			expect.objectContaining({
				expected_revision: 7,
				regions: PLANNED_DEMO_SOLO_REGIONS,
			}),
			true,
			undefined,
			{ showId: "00000000-0000-4000-8000-000000000001" },
		);
	});
});
