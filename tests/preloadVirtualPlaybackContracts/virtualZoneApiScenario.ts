import type { Session } from "../bench/core/api";
import {
	type BenchContractContext,
	expect,
	test,
} from "../bench/core/fixtures";
import { object, putObject } from "../support/catalog";
import {
	activeAssignedPlayback,
	audit,
	configuration,
	playbacks,
	prepare,
	pageObject,
	saveSoloRegions,
	paneAction,
	soloRegionSnapshot,
	visualizationLevel,
	copyPageAssignments,
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
			{ position: 71, fixture: 3, levels: [0.2], name: "Zone A" },
			{ position: 72, fixture: 4, levels: [0.4], name: "Zone B" },
			{ position: 73, fixture: 5, levels: [0.6], name: "Zone C" },
		],
		{},
	);
	await copyPageAssignments(api, 1, { 401: 71, 402: 72, 403: 73 });
	await copyPageAssignments(api, 2, { 401: 73, 402: 71, 403: 72 });
	await api.request("POST", "/api/v2/configuration/update", {request_id:crypto.randomUUID(),patch:{sequence_master_fade_millis: 0,
	}});
	const firstPage = await pageObject(api,1);
 firstPage.body.assignments[71].auto_off=false;
 firstPage.body.assignments[401].auto_off=false;
 await putObject(api,"playback_page","1",firstPage.body,firstPage.revision);
	const firstDesk = api.session!.desk;
	const zones = [
		{
			id: "22222222-2222-4222-8222-222222222222",
			name: "Front pair",
			members: [{page:1,position:401}, {page:1,position:402}],
		},
		{
			id: "33333333-3333-4333-8333-333333333333",
			name: "Overlap pair",
			members: [{page:1,position:402}, {page:1,position:403}],
		},
	];

	await paneAction(api, {page: 1, position: 401}, "go");
	await paneAction(api, {page: 1, position: 402}, "go");
	await paneAction(api, {page: 1, position: 403}, "go");
	expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toMatchObject({
		enabled: true,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toMatchObject({
		enabled: true,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 403})).toMatchObject({
		enabled: true,
	});
	const zoneRequestId = crypto.randomUUID();
	const saved = await saveSoloRegions(api, zones, {
		requestId: zoneRequestId,
	});
	expect(saved).toMatchObject({
		show_id: prepared.showId,
		revision: 1,
		regions: zones,
		replayed: false,
		changed: true,
	});
	expect(saved.request_id).toEqual(expect.any(String));
	const replay = await saveSoloRegions(api, zones, {
		requestId: zoneRequestId,
	});
	expect(replay).toMatchObject({
		request_id: zoneRequestId,
		replayed: true,
		changed: true,
		revision: 1,
	});
	await expect(
		saveSoloRegions(
			api,
			[
				{
					id: "stale",
					name: "Stale",
					members: [{page:1,position:401}, {page:1,position:403}],
				},
			],
			{
				expectedRevision: 0,
			},
		),
	).rejects.toThrow(/returned 409:.*expected 0, actual 1/);
	expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toMatchObject({
		enabled: true,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toMatchObject({
		enabled: true,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 403})).toMatchObject({
		enabled: true,
	});
	expect(await soloRegionSnapshot(api)).toMatchObject({
		show_id: prepared.showId,
		revision: 1,
		regions: zones,
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
	expect((await pageObject(api,1)).body.assignments[71].auto_off).toBe(false);
	expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toMatchObject({
		enabled: false,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toMatchObject({
		enabled: false,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 403})).toMatchObject({
		enabled: true,
	});
	expect((await soloRegionSnapshot(api)).regions).toEqual(zones);

	for (const number of [401, 402, 403])
		await paneAction(api, {page: 1, position: number}, "off");
	await Promise.all([
		paneAction(api, {page: 1, position: 401}, "go"),
		paneAction(api, {page: 1, position: 402}, "go"),
	]);
	const concurrent = await Promise.all([401,402].map(position=>activeAssignedPlayback(api,{page:1,position})));
 expect(concurrent.filter(runtime=>runtime?.enabled)).toHaveLength(1);
	await paneAction(api, {page: 1, position: 403}, "go");
	await paneAction(api, {page: 1, position: 402}, "go");
	expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toMatchObject({
		enabled: false,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toMatchObject({
		enabled: true,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 403})).toMatchObject({
		enabled: false,
	});
	await bench.tick(0);
	expect(await visualizationLevel(api, prepared.fixtures[3])).toBeCloseTo(0, 5);
	expect(await visualizationLevel(api, prepared.fixtures[4])).toBeCloseTo(
		0.4,
		5,
	);
	expect(await visualizationLevel(api, prepared.fixtures[5])).toBeCloseTo(0, 5);
	await paneAction(api, {page: 1, position: 402}, "off");
	expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toMatchObject({
		enabled: false,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 403})).toMatchObject({
		enabled: false,
	});
}

async function verifyFirstDeskPageAndOsc({
	api,
	bench,
}: VirtualZoneApiContext) {
	const desk = api.session!.desk;
	await api.request("POST", "/api/v2/playback-surfaces/main/page", {page:2},true,undefined,{deskId:api.session!.desk.id});
	for (const number of [401, 402, 403])
		await paneAction(api, {page: 1, position: number}, "off");
	for (const number of [401, 402, 403])
		await paneAction(api, {page: 2, position: number}, "off");
	await paneAction(api, {page: 2, position: 401}, "go");
	await paneAction(api, {page: 2, position: 402}, "go");
	expect(await activeAssignedPlayback(api, {page: 2, position: 401})).toMatchObject({
		enabled: true,
	});
	expect(await activeAssignedPlayback(api, {page: 2, position: 402})).toMatchObject({
		enabled: true,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toMatchObject({
		enabled: false,
	});
	await api.request("POST", "/api/v2/playback-surfaces/main/page", {page:1},true,undefined,{deskId:api.session!.desk.id});
	for (const number of [401, 402, 403])
		await paneAction(api, {page: 1, position: number}, "off");

	const firstHardware = await bench.osc();
	try {
		await firstHardware.subscribe("vpb-007-first", "desk");
		await firstHardware.send(
			`/light/desk/playback/1/401/button/1`,
			[true],
		);
		await firstHardware.send(
			`/light/desk/playback/1/402/button/1`,
			[true],
		);
		await expect
			.poll(async () => (await activeAssignedPlayback(api, {page: 1, position: 401}))?.enabled)
			.toBe(false);
		await expect
			.poll(async () => (await activeAssignedPlayback(api, {page: 1, position: 402}))?.enabled)
			.toBe(true);
		expect(
			(await audit(api)).some(
				(event) =>
					event.kind === "playback_exclusion_applied" &&
					event.payload?.source === "osc" &&
					event.payload?.activated_playback?.page === 1 &&
					event.payload?.activated_playback?.position === 402,
			),
		).toBe(true);
	} finally {
		await firstHardware.close();
	}
}

async function verifySecondDeskSharesShowZones(
	{ api, bench }: VirtualZoneApiContext,
	{ prepared, zones }: AuthoritativeVirtualZoneSetup,
) {
	const second = await api.request<Session>(
		"POST",
		"/api/v2/sessions",
		{ username: "Operator", client_id: crypto.randomUUID() },
		false,
	);
	api.session = second;
	await api.request("POST", "/api/v2/playback-surfaces/main/page", {page:2},true,undefined,{deskId:api.session!.desk.id});
	expect(await soloRegionSnapshot(api)).toMatchObject({
		show_id: prepared.showId,
		revision: 1,
		regions: zones,
	});
	for (const number of [401, 402, 403])
		await paneAction(api, {page: 1, position: number}, "off");
	for (const number of [401, 402, 403])
		await paneAction(api, {page: 2, position: number}, "off");
	const secondHardware = await bench.osc();
	try {
		await secondHardware.subscribe("vpb-007-second", "desk");
		await secondHardware.send(
			`/light/desk/playback/1/401/button/1`,
			[true],
		);
		await secondHardware.send(
			`/light/desk/playback/1/402/button/1`,
			[true],
		);
		await expect
			.poll(async () => (await activeAssignedPlayback(api, {page: 1, position: 401}))?.enabled)
			.toBe(false);
		await expect
			.poll(async () => (await activeAssignedPlayback(api, {page: 1, position: 402}))?.enabled)
			.toBe(true);
		await paneAction(api, {page: 2, position: 401}, "go");
		await paneAction(api, {page: 2, position: 402}, "go");
		expect(await activeAssignedPlayback(api, {page: 2, position: 401})).toMatchObject({
			enabled: true,
		});
		expect(await activeAssignedPlayback(api, {page: 2, position: 402})).toMatchObject({
			enabled: true,
		});
		await bench.tick(0);
		expect(await visualizationLevel(api, prepared.fixtures[3])).toBeCloseTo(
			0.2,
			5,
		);
		expect(await visualizationLevel(api, prepared.fixtures[4])).toBeCloseTo(
			0.4,
			5,
		);
		expect(await visualizationLevel(api, prepared.fixtures[5])).toBeCloseTo(
			0.6,
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
	await verifySecondDeskSharesShowZones(context, setup);
};

export function registerVirtualZoneApiScenario(): void {
	test(
		"VPB-007 @api › revisioned show-owned playback-number zones arbitrate across desks, pages, restart, and OSC",
		virtualZoneApiSupplement,
	);
}
