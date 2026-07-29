import type { Session } from "../bench/core/api";
import {
	type BenchContractContext,
	expect,
	test,
} from "../bench/core/fixtures";
import { object, putObject } from "../support/catalog";
import {
	activePlayback,
	activeVirtualPlayback,
	audit,
	configuration,
	playbacks,
	prepare,
	saveVirtualZoneSurface,
	virtualAction,
	virtualZoneSnapshot,
	visualizationLevel,
	writeVirtualPage,
} from "./support";

type VirtualZoneApiContext = Pick<BenchContractContext, "api" | "bench">;

async function prepareAuthoritativeVirtualZone({
	api,
	bench,
}: VirtualZoneApiContext) {
	const prepared = await prepare(
		api,
		bench,
		"vpb-007-authoritative",
		[
			{ number: 71, fixture: 3, levels: [0.2], name: "Zone A" },
			{ number: 72, fixture: 4, levels: [0.4], name: "Zone B" },
			{ number: 73, fixture: 5, levels: [0.6], name: "Zone C" },
		],
		{},
	);
	await writeVirtualPage(api, 1, { 1001: 71, 1002: 72, 1003: 73 });
	await writeVirtualPage(api, 2, { 1001: 73, 1002: 71, 1003: 72 });
	await api.request("PUT", "/api/v2/configuration", {
		...(await configuration(api)),
		sequence_master_fade_millis: 0,
	});
	const firstDefinition = await object<any>(api, "playback", "71");
	await putObject(
		api,
		"playback",
		"71",
		{ ...firstDefinition.body, auto_off: false },
		firstDefinition.revision,
	);
	const firstDesk = api.session!.desk;
	const zones = [
		{ id: "front-pair", name: "Front pair", slots: [1, 2] },
		{ id: "overlap", name: "Overlap pair", slots: [2, 3] },
	];

	await virtualAction(api, 1, 1001, "go");
	await virtualAction(api, 1, 1002, "go");
	await virtualAction(api, 1, 1003, "go");
	expect(await activeVirtualPlayback(api, 1, 1001)).toMatchObject({
		enabled: true,
	});
	expect(await activeVirtualPlayback(api, 1, 1002)).toMatchObject({
		enabled: true,
	});
	expect(await activeVirtualPlayback(api, 1, 1003)).toMatchObject({
		enabled: true,
	});
	const zoneRequestId = crypto.randomUUID();
	const saved = await saveVirtualZoneSurface(api, "vpb-api-surface", zones, {
		requestId: zoneRequestId,
		pageMode: { type: "follow_main" },
	});
	expect(saved).toMatchObject({
		show_id: prepared.showId,
		desk_id: firstDesk.id,
		surface_id: "vpb-api-surface",
		surface: {
			revision: 1,
			page_mode: { type: "follow_main" },
			zones,
		},
		replayed: false,
		changed: true,
	});
	expect(saved.request_id).toEqual(expect.any(String));
	const replay = await saveVirtualZoneSurface(api, "vpb-api-surface", zones, {
		requestId: zoneRequestId,
		pageMode: { type: "follow_main" },
	});
	expect(replay).toMatchObject({
		request_id: zoneRequestId,
		replayed: true,
		changed: true,
		surface: { revision: 1 },
	});
	await expect(
		saveVirtualZoneSurface(
			api,
			"vpb-api-surface",
			[{ id: "stale", name: "Stale", slots: [1, 3] }],
			{
				expectedRevision: 0,
				pageMode: { type: "follow_main" },
			},
		),
	).rejects.toThrow(/returned 409:.*expected 0, actual 1/);
	expect(await activeVirtualPlayback(api, 1, 1001)).toMatchObject({
		enabled: true,
	});
	expect(await activeVirtualPlayback(api, 1, 1002)).toMatchObject({
		enabled: true,
	});
	expect(await activeVirtualPlayback(api, 1, 1003)).toMatchObject({
		enabled: true,
	});
	expect(await virtualZoneSnapshot(api)).toMatchObject({
		show_id: prepared.showId,
		desks: {
			[firstDesk.id]: {
				"vpb-api-surface": {
					revision: 1,
					page_mode: { type: "follow_main" },
					zones,
				},
			},
		},
	});
	return { prepared, firstDesk, zones };
}

type AuthoritativeVirtualZoneSetup = Awaited<
	ReturnType<typeof prepareAuthoritativeVirtualZone>
>;

async function verifyRestartedVirtualZone(
	{ api, bench }: VirtualZoneApiContext,
	{ prepared, firstDesk, zones }: AuthoritativeVirtualZoneSetup,
) {
	await bench.stopServerGracefully(api.session!.token);
	await bench.startServer();
	api.session = await api.request<Session>(
		"POST",
		"/api/v2/sessions",
		{ username: "Operator", desk_id: firstDesk.id },
		false,
	);
	expect((await object<any>(api, "playback", "71")).body.auto_off).toBe(false);
	expect(await activeVirtualPlayback(api, 1, 1001)).toMatchObject({
		enabled: false,
	});
	expect(await activeVirtualPlayback(api, 1, 1002)).toMatchObject({
		enabled: false,
	});
	expect(await activeVirtualPlayback(api, 1, 1003)).toMatchObject({
		enabled: true,
	});
	expect(
		(await virtualZoneSnapshot(api)).desks[firstDesk.id][
			"vpb-api-surface"
		].zones,
	).toEqual(zones);

	for (const number of [1001, 1002, 1003])
		await virtualAction(api, 1, number, "off");
	await Promise.all([
		virtualAction(api, 1, 1001, "go"),
		virtualAction(api, 1, 1002, "go"),
	]);
	const concurrent = (await playbacks(api)).active.filter(
		(entry: any) =>
			entry.playback_identity?.kind === "virtual" &&
			entry.playback_identity.page === 1 &&
			[1001, 1002].includes(entry.playback_identity.number) &&
			entry.enabled,
	);
	expect(concurrent).toHaveLength(1);
	await virtualAction(api, 1, 1003, "go");
	await virtualAction(api, 1, 1002, "go");
	expect(await activeVirtualPlayback(api, 1, 1001)).toMatchObject({
		enabled: false,
	});
	expect(await activeVirtualPlayback(api, 1, 1002)).toMatchObject({
		enabled: true,
	});
	expect(await activeVirtualPlayback(api, 1, 1003)).toMatchObject({
		enabled: false,
	});
	await bench.tick(0);
	expect(await visualizationLevel(api, prepared.fixtures[3])).toBeCloseTo(0, 5);
	expect(await visualizationLevel(api, prepared.fixtures[4])).toBeCloseTo(
		0.4,
		5,
	);
	expect(await visualizationLevel(api, prepared.fixtures[5])).toBeCloseTo(0, 5);
	await virtualAction(api, 1, 1002, "off");
	expect(await activeVirtualPlayback(api, 1, 1001)).toMatchObject({
		enabled: false,
	});
	expect(await activeVirtualPlayback(api, 1, 1003)).toMatchObject({
		enabled: false,
	});
}

async function verifyFirstDeskPageAndOsc({
	api,
	bench,
}: VirtualZoneApiContext) {
	const desk = api.session!.desk;
	await api.request(
		"POST",
		`/api/v2/control-desks/${desk.id}/actions`,
		{
			request_id: crypto.randomUUID(),
			action: { type: "set_page", page: 2, existing_only: false },
		},
	);
	for (const page of [1, 2])
		for (const number of [1001, 1002, 1003])
			await virtualAction(api, page, number, "off");
	await virtualAction(api, 2, 1001, "go");
	await virtualAction(api, 2, 1002, "go");
	expect(await activeVirtualPlayback(api, 2, 1001)).toMatchObject({
		enabled: false,
	});
	expect(await activeVirtualPlayback(api, 2, 1002)).toMatchObject({
		enabled: true,
	});
	expect(await activeVirtualPlayback(api, 1, 1002)).toMatchObject({
		enabled: false,
	});
	await api.request(
		"POST",
		`/api/v2/control-desks/${desk.id}/actions`,
		{
			request_id: crypto.randomUUID(),
			action: { type: "set_page", page: 1, existing_only: false },
		},
	);
	for (const number of [1001, 1002, 1003])
		await virtualAction(api, 1, number, "off");

	const firstHardware = await bench.osc();
	try {
		await firstHardware.subscribe("vpb-007-first", desk.osc_alias);
		await firstHardware.send(
			`/light/${desk.osc_alias}/virtual-playback/1/1001/button/1`,
			[true],
		);
		await firstHardware.send(
			`/light/${desk.osc_alias}/virtual-playback/1/1002/button/1`,
			[true],
		);
		await expect
			.poll(
				async () =>
					(await activeVirtualPlayback(api, 1, 1001))?.enabled,
			)
			.toBe(false);
		await expect
			.poll(
				async () =>
					(await activeVirtualPlayback(api, 1, 1002))?.enabled,
			)
			.toBe(true);
		expect(
			(await audit(api)).some(
				(event) =>
					event.kind === "playback_exclusion_applied" &&
					event.payload?.source === "osc" &&
					event.payload?.activated_page === 1 &&
					event.payload?.activated_playback === 1002,
			),
		).toBe(true);
	} finally {
		await firstHardware.close();
	}
}

async function verifySecondDeskPartitionIsolation(
	{ api, bench }: VirtualZoneApiContext,
	{ prepared, firstDesk, zones }: AuthoritativeVirtualZoneSetup,
) {
	const second = await api.request<Session>(
		"POST",
		"/api/v2/sessions",
		{ username: "Operator", client_id: crypto.randomUUID() },
		false,
	);
	api.session = second;
	const secondZones = [
		{ id: "wing-spares", name: "Wing spares", slots: [1, 2] },
	];
	const secondSaved = await saveVirtualZoneSurface(
		api,
		"vpb-second-surface",
		secondZones,
		{ pageMode: { type: "pinned", page: 2 } },
	);
	expect(secondSaved).toMatchObject({
		show_id: prepared.showId,
		desk_id: second.desk.id,
		surface_id: "vpb-second-surface",
		surface: {
			revision: 1,
			page_mode: { type: "pinned", page: 2 },
			zones: secondZones,
		},
		replayed: false,
		changed: true,
	});
	expect(await virtualZoneSnapshot(api)).toMatchObject({
		show_id: prepared.showId,
		desks: {
			[firstDesk.id]: {
				"vpb-api-surface": {
					revision: 1,
					page_mode: { type: "follow_main" },
					zones,
				},
			},
			[second.desk.id]: {
				"vpb-second-surface": {
					revision: 1,
					page_mode: { type: "pinned", page: 2 },
					zones: secondZones,
				},
			},
		},
	});
	for (const page of [1, 2])
		for (const number of [1001, 1002, 1003])
			await virtualAction(api, page, number, "off");
	const secondHardware = await bench.osc();
	try {
		await secondHardware.subscribe("vpb-007-second", second.desk.osc_alias);
		await secondHardware.send(
			`/light/${second.desk.osc_alias}/virtual-playback/1/1001/button/1`,
			[true],
		);
		await secondHardware.send(
			`/light/${second.desk.osc_alias}/virtual-playback/1/1002/button/1`,
			[true],
		);
		await expect
			.poll(
				async () =>
					(await activeVirtualPlayback(api, 1, 1001))?.enabled,
			)
			.toBe(true);
		await expect
			.poll(
				async () =>
					(await activeVirtualPlayback(api, 1, 1002))?.enabled,
			)
			.toBe(true);
		await virtualAction(api, 2, 1001, "go");
		await virtualAction(api, 2, 1002, "go");
		expect(await activeVirtualPlayback(api, 2, 1001)).toMatchObject({
			enabled: false,
		});
		expect(await activeVirtualPlayback(api, 2, 1002)).toMatchObject({
			enabled: true,
		});
		expect(await activePlayback(api, 71)).toBeUndefined();
		expect(await activePlayback(api, 72)).toBeUndefined();
		await bench.tick(0);
		expect(await visualizationLevel(api, prepared.fixtures[3])).toBeCloseTo(
			0.2,
			5,
		);
		expect(await visualizationLevel(api, prepared.fixtures[4])).toBeCloseTo(
			0.4,
			5,
		);
	} finally {
		await secondHardware.close();
	}
}

const virtualZoneApiSupplement = async ({
	api,
	bench,
}: BenchContractContext) => {
	const context = { api, bench };
	const setup = await prepareAuthoritativeVirtualZone(context);
	await verifyRestartedVirtualZone(context, setup);
	await verifyFirstDeskPageAndOsc(context);
	await verifySecondDeskPartitionIsolation(context, setup);
};

export function registerVirtualZoneApiScenario(): void {
	test(
		"VPB-007 @api › revisioned Follow Main and Pinned zone surfaces retain page-qualified desk isolation across restart and OSC",
		virtualZoneApiSupplement,
	);
}
