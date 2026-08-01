import type { ApiDriver } from "../bench/core/api";

export const PLANNED_DEMO_BENCHMARK_ASSIGNMENTS = [
	{ name: "ACL Chase", kind: "physical", playbackNumber: 8 },
	{ name: "Wash Show Waterfall", kind: "virtual", playbackNumber: 1024 },
	{ name: "Beam Show Circle", kind: "virtual", playbackNumber: 1019 },
	{ name: "Beam Show PWM", kind: "virtual", playbackNumber: 1001 },
	{ name: "LED Show Random", kind: "virtual", playbackNumber: 1014 },
	{ name: "LED Show Random Strobe", kind: "virtual", playbackNumber: 1030 },
	{ name: "Sunstrip Rain", kind: "virtual", playbackNumber: 1029 },
	{ name: "Beam Auxiliary Show Circle", kind: "virtual", playbackNumber: 1021 },
	{ name: "Beam Auxiliary Show PWM", kind: "virtual", playbackNumber: 1004 },
	{
		name: "Wash Auxiliary Show Waterfall",
		kind: "virtual",
		playbackNumber: 1026,
	},
	{ name: "Wash Auxiliary Show Random", kind: "virtual", playbackNumber: 1011 },
	{ name: "LED Auxiliary Show Sinus", kind: "virtual", playbackNumber: 1018 },
] as const;

export const PLANNED_DEMO_BENCHMARK_SPEED_GROUPS = {
	A: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
	B: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
	C: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
	D: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
	E: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
} as const;

export async function startPlannedDemoBenchmarkLook(
	api: ApiDriver,
	showId: string,
) {
	if (!api.session)
		throw new Error("Plan 76 benchmark activation requires an API session");
	await api.request("POST", "/api/v2/test/clock/advance", { millis: 0 }, false);
	for (const assignment of PLANNED_DEMO_BENCHMARK_ASSIGNMENTS) {
		if (assignment.kind === "physical") {
			await api.playbackNumberAction(assignment.playbackNumber, "go");
			continue;
		}
		await api.request(
			"POST",
			"/api/v2/playback-actions",
			{
				request_id: crypto.randomUUID(),
				address: {
					kind: "virtual",
					page: 1,
					playback_number: assignment.playbackNumber,
				},
				action: { type: "master", value: 1 },
				surface: "virtual",
			},
			true,
			undefined,
			{ showId, deskId: api.session.desk.id },
		);
		await api.request(
			"POST",
			"/api/v2/playback-actions",
			{
				request_id: crypto.randomUUID(),
				address: {
					kind: "virtual",
					page: 1,
					playback_number: assignment.playbackNumber,
				},
				action: { type: "on", pressed: true },
				surface: "virtual",
			},
			true,
			undefined,
			{ showId, deskId: api.session.desk.id },
		);
	}
	await api.request(
		"POST",
		"/api/v2/test/clock/advance",
		{ millis: 20 },
		false,
	);
	return api.request<any>(
		"POST",
		"/api/v2/playback-runtime/snapshot",
		{
			identities: PLANNED_DEMO_BENCHMARK_ASSIGNMENTS.map((assignment) =>
				assignment.kind === "physical"
					? { kind: "playback", playback_number: assignment.playbackNumber }
					: {
							kind: "virtual",
							page: 1,
							playback_number: assignment.playbackNumber,
						},
			),
		},
		true,
		undefined,
		{ showId, deskId: api.session.desk.id },
	);
}
