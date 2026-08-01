import { describe, expect, it, vi } from "vitest";
import { plannedDemoDynamicDefinitions } from "../../support/plannedDemoDynamics";
import {
	installPlannedDemoVirtualPlaybackExclusionZones,
	PLANNED_DEMO_VIRTUAL_PLAYBACK_EXCLUSION_ZONES,
} from "../../support/plannedDemoVirtualPlaybackZones";

describe("Plan 76 Virtual Playback exclusion zones", () => {
	it("keeps Beam Show intensity and movement effects mutually exclusive", async () => {
		const request = vi
			.fn()
			.mockResolvedValueOnce({ revision: 7 })
			.mockResolvedValueOnce({ changed: true });
		await installPlannedDemoVirtualPlaybackExclusionZones(
			{ request } as never,
			"00000000-0000-4000-8000-000000000001",
		);

		const definitions = plannedDemoDynamicDefinitions();
		const definitionForPlayback = (playbackNumber: number) =>
			definitions.find(
				(definition) => definition.pool_number === playbackNumber - 1000,
			);
		expect(PLANNED_DEMO_VIRTUAL_PLAYBACK_EXCLUSION_ZONES).toEqual([
			expect.objectContaining({
				name: "Beam Show Intensity Effects",
				playback_numbers: [1001, 1002, 1003],
			}),
			expect.objectContaining({
				name: "Beam Show Movement Effects",
				playback_numbers: [1019, 1020],
			}),
		]);
		const intensityDefinitions = [1001, 1002, 1003].map(definitionForPlayback);
		expect(
			intensityDefinitions.every(
				(definition) =>
					definition?.target_binding.group_id === "4" &&
					definition.lanes.every(
						(lane: { attribute: string }) => lane.attribute === "intensity",
					),
			),
		).toBe(true);
		const movementDefinitions = [1019, 1020].map(definitionForPlayback);
		expect(
			movementDefinitions.every(
				(definition) =>
					definition?.target_binding.group_id === "4" &&
					definition.lanes.some((lane: { attribute: string }) =>
						["pan", "tilt"].includes(lane.attribute),
					),
			),
		).toBe(true);
		expect(request).toHaveBeenLastCalledWith(
			"POST",
			"/api/v2/virtual-playback-exclusion-zones/update",
			expect.objectContaining({
				expected_revision: 7,
				zones: PLANNED_DEMO_VIRTUAL_PLAYBACK_EXCLUSION_ZONES,
			}),
			true,
			undefined,
			{ showId: "00000000-0000-4000-8000-000000000001" },
		);
	});
});
