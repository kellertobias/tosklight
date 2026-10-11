import type { ApiDriver } from "../bench/core/api";

export const PLANNED_DEMO_SOLO_REGIONS = [
	{
		id: "00000000-0000-4714-8200-000000000001",
		name: "Beam Show Intensity Effects",
		members: [401, 402, 403].map((position) => ({ page: 1, position })),
	},
	{
		id: "00000000-0000-4714-8200-000000000002",
		name: "Beam Show Movement Effects",
		members: [419, 420].map((position) => ({ page: 1, position })),
	},
] as const;

interface PlaybackSoloSnapshot {
	revision: number;
}

export async function installPlannedDemoSoloRegions(
	api: ApiDriver,
	showId: string,
) {
	const snapshot = await api.request<PlaybackSoloSnapshot>(
		"GET",
		"/api/v2/playback-solo-regions",
		undefined,
		true,
		undefined,
		{ showId },
	);
	return api.request(
		"POST",
		"/api/v2/playback-solo-regions/update",
		{
			request_id: crypto.randomUUID(),
			expected_revision: snapshot.revision,
			regions: PLANNED_DEMO_SOLO_REGIONS,
		},
		true,
		undefined,
		{ showId },
	);
}
