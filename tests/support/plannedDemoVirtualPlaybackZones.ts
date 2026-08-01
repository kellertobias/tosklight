import type { ApiDriver } from "../bench/core/api";

export const PLANNED_DEMO_VIRTUAL_PLAYBACK_EXCLUSION_ZONES = [
	{
		id: "planned-demo-beam-show-intensity",
		name: "Beam Show Intensity Effects",
		playback_numbers: [1001, 1002, 1003],
	},
	{
		id: "planned-demo-beam-show-movement",
		name: "Beam Show Movement Effects",
		playback_numbers: [1019, 1020],
	},
] as const;

interface VirtualPlaybackExclusionSnapshot {
	revision: number;
}

export async function installPlannedDemoVirtualPlaybackExclusionZones(
	api: ApiDriver,
	showId: string,
) {
	const snapshot = await api.request<VirtualPlaybackExclusionSnapshot>(
		"GET",
		"/api/v2/virtual-playback-exclusion-zones",
		undefined,
		true,
		undefined,
		{ showId },
	);
	return api.request(
		"POST",
		"/api/v2/virtual-playback-exclusion-zones/update",
		{
			request_id: crypto.randomUUID(),
			expected_revision: snapshot.revision,
			zones: PLANNED_DEMO_VIRTUAL_PLAYBACK_EXCLUSION_ZONES,
		},
		true,
		undefined,
		{ showId },
	);
}
