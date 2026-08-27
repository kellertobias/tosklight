import type {
	DmxSnapshot,
	OutputHealth,
	PatchedFixture,
} from "../../../light-desktop/src/api/types";

function slots(values: Record<number, number>): number[] {
	return Array.from({ length: 512 }, (_, index) => values[index + 1] ?? 0);
}

function cluster(start: number, values: number[]): Record<number, number> {
	return Object.fromEntries(
		values.map((value, index) => [start + index, value]),
	);
}

export const dmxSnapshot: DmxSnapshot = {
	revision: 17,
	universes: [
		{
			universe: 1,
			slots: slots({
				...cluster(1, [204, 128, 42, 12, 180, 96, 64, 255]),
				...cluster(13, [224, 154]),
				...cluster(41, [188, 55, 210, 90, 30, 255]),
				97: 255,
			}),
		},
		{
			universe: 2,
			slots: slots({
				...cluster(1, [180, 44, 220, 76, 160, 255, 90, 32]),
				...cluster(64, [84, 144, 38, 210, 62]),
				...cluster(128, [176, 92, 220, 33]),
				512: 232,
			}),
		},
		{
			universe: 3,
			slots: slots({
				...cluster(1, [255, 210, 44, 180, 96, 33]),
				...cluster(49, [120, 240, 66, 190, 22, 144]),
				...cluster(121, [255, 80, 160, 205]),
			}),
		},
		{
			universe: 4,
			slots: slots({
				...cluster(1, [92, 255, 160, 30, 210, 74, 128]),
				...cluster(81, [220, 112, 44, 188, 250]),
				...cluster(201, [68, 135, 235, 42, 198, 90]),
			}),
		},
	],
	overrides: [
		{ universe: 1, address: 13, value: 224 },
		{ universe: 2, address: 128, value: 176 },
	],
};

export const dmxSnapshotWithoutOverrides: DmxSnapshot = {
	...dmxSnapshot,
	overrides: [],
};

function patchedFixture(
	fixtureId: string,
	fixtureNumber: number,
	name: string,
	deviceType: string,
	universe: number,
	address: number,
	footprint: number,
): PatchedFixture {
	return {
		fixture_id: fixtureId,
		fixture_number: fixtureNumber,
		name,
		universe,
		address,
		definition: {
			name,
			device_type: deviceType,
			footprint,
			heads: [
				{
					parameters: Array.from({ length: footprint }, (_, offset) => ({
						attribute: offset === 0 ? "intensity" : `channel.${offset + 1}`,
						components: [{ offset }],
					})),
				},
			],
		},
		logical_heads: [],
		multipatch: [],
	} as unknown as PatchedFixture;
}

export const dmxPatchedFixtures = [
	patchedFixture(
		"front-profile-left",
		101,
		"Front Profile SL",
		"profile",
		1,
		1,
		8,
	),
	patchedFixture("stage-hazer", 99, "Stage Hazer", "hazer", 1, 13, 2),
	patchedFixture(
		"front-profile-right",
		102,
		"Front Profile SR",
		"profile",
		1,
		41,
		8,
	),
	patchedFixture("back-wash-left", 201, "Back Wash 1", "moving wash", 2, 1, 12),
	patchedFixture(
		"back-wash-right",
		202,
		"Back Wash 2",
		"moving wash",
		2,
		64,
		12,
	),
	patchedFixture("beam-line", 301, "Beam Line", "moving beam", 2, 128, 16),
	patchedFixture(
		"audience-blinders",
		401,
		"Audience Blinders",
		"blinder",
		3,
		1,
		12,
	),
	patchedFixture(
		"stage-pixels",
		501,
		"Stage Pixel Bars",
		"strip light",
		3,
		49,
		24,
	),
	patchedFixture(
		"house-practicals",
		601,
		"House Practicals",
		"dimmer",
		3,
		121,
		8,
	),
	patchedFixture("floor-package", 701, "Floor Package", "led wash", 4, 1, 16),
	patchedFixture("followspots", 801, "Followspots", "profile", 4, 81, 8),
	patchedFixture("scenic-led", 901, "Scenic LED", "strip light", 4, 201, 24),
];

export const dmxOutputHealth: OutputHealth = {
	frames_sent: 85_412,
	packets_sent: 170_824,
	send_errors: 0,
	deadline_misses: 0,
	maximum_lateness_micros: 140,
	frame_hz: 44,
	last_tick_micros: 510,
	maximum_tick_micros: 820,
	scheduler_utilization: 0.08,
	recent_window_seconds: 60,
	recent_frame_hz_minimum: 37.4,
	recent_frame_hz_maximum: 44.2,
	recent_frame_hz_average: 43.8,
	recent_frame_rate_bucket_bounds_hz: [20, 30, 38, 40, 44, 48, 52, 56, 60],
	recent_frame_rate_bucket_counts: [2, 118, 116, 113, 101, 74, 41, 12, 3, 0],
	recent_send_errors: 0,
	frame_rate_band_bounds_hz: [20, 30, 40, 44, 60],
	frame_rate_band_counts: [0, 0, 1, 3, 12],
};
