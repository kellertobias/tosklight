import type { ApiDriver } from "../bench/core/api";

export const PLANNED_DEMO_BENCHMARK_ASSIGNMENTS = [
	{ name: "ACL Chase", page: 1, position: 107 },
	{ name: "Wash Show Waterfall", page: 1, position: 424 },
	{ name: "Beam Show Circle", page: 1, position: 419 },
	{ name: "Beam Show PWM", page: 1, position: 401 },
	{ name: "LED Show Random", page: 1, position: 414 },
	{ name: "LED Show Random Strobe", page: 1, position: 430 },
	{ name: "Sunstrip Rain", page: 1, position: 429 },
	{ name: "Beam Auxiliary Show Circle", page: 1, position: 421 },
	{ name: "Beam Auxiliary Show PWM", page: 1, position: 404 },
	{ name: "Wash Auxiliary Show Waterfall", page: 1, position: 426 },
	{ name: "Wash Auxiliary Show Random", page: 1, position: 411 },
	{ name: "LED Auxiliary Show Sinus", page: 1, position: 418 },
] as const;

export const PLANNED_DEMO_BENCHMARK_SPEED_GROUPS = {
	A: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
	B: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
	C: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
	D: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
	E: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
} as const;

async function action(
	api: ApiDriver,
	showId: string,
	assignment: (typeof PLANNED_DEMO_BENCHMARK_ASSIGNMENTS)[number],
	action: object,
) {
	if (!api.session)
		throw new Error("Plan 76 benchmark control requires an API session");
	return api.request(
		"POST",
		"/api/v2/playback-actions",
		{
			address: {
				kind: "playback",
				page: assignment.page,
				position: assignment.position,
			},
			action,
			control_source: {
				surface_id:
					assignment.position >= 401 ? "pane:busking-playbacks" : "main",
				control_id: assignment.position,
			},
		},
		true,
		undefined,
		{ showId, deskId: api.session.desk.id },
	);
}

export async function startPlannedDemoBenchmarkLook(
	api: ApiDriver,
	showId: string,
) {
	await api.request("POST", "/api/v2/test/clock/advance", { millis: 0 }, false);
	for (const assignment of PLANNED_DEMO_BENCHMARK_ASSIGNMENTS) {
		await action(api, showId, assignment, { type: "master", value: 1 });
		await action(api, showId, assignment, { type: "on", pressed: true });
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
			identities: PLANNED_DEMO_BENCHMARK_ASSIGNMENTS.map(
				({ page, position }) => ({ kind: "playback", page, position }),
			),
		},
		true,
		undefined,
		{ showId, deskId: api.session!.desk.id },
	);
}

export async function stopPlannedDemoBenchmarkLook(
	api: ApiDriver,
	showId: string,
) {
	for (const assignment of PLANNED_DEMO_BENCHMARK_ASSIGNMENTS)
		await action(api, showId, assignment, { type: "off", pressed: true });
	await api.request(
		"POST",
		"/api/v2/test/clock/advance",
		{ millis: 20 },
		false,
	);
}
