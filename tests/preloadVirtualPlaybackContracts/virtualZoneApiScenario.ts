import type { Session } from "../../apps/control-ui/e2e/bench/api";
import {
	type BenchContractContext,
	expect,
	test,
} from "../../apps/control-ui/e2e/bench/fixtures";
import { object, putObject } from "../support/catalog";
import {
	activePlayback,
	audit,
	configuration,
	playbacks,
	poolAction,
	prepare,
	visualizationLevel,
	writePage,
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
		{ 1: 71, 2: 72, 3: 73 },
	);
	await writePage(api, 2, { "1": 73, "2": 71, "3": 72 });
	await api.request("PUT", "/api/v1/configuration", {
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

	await poolAction(api, 71, "go", { surface: "virtual" });
	await poolAction(api, 72, "go", { surface: "virtual" });
	await poolAction(api, 73, "go", { surface: "virtual" });
	expect(await activePlayback(api, 71)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 72)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 73)).toMatchObject({ enabled: true });
	await api.request(
		"PUT",
		"/api/v1/virtual-playback-exclusion-zones/vpb-api-surface",
		{ zones },
	);
	expect(await activePlayback(api, 71)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 72)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 73)).toMatchObject({ enabled: true });
	expect(
		await api.request<any>("GET", "/api/v1/virtual-playback-exclusion-zones"),
	).toMatchObject({
		desk_id: firstDesk.id,
		surfaces: { "vpb-api-surface": zones },
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
		"/api/v1/sessions",
		{ username: "Operator", desk_id: firstDesk.id },
		false,
	);
	expect((await object<any>(api, "playback", "71")).body.auto_off).toBe(false);
	expect(await activePlayback(api, 71)).toMatchObject({ enabled: false });
	expect(await activePlayback(api, 72)).toMatchObject({ enabled: false });
	expect(await activePlayback(api, 73)).toMatchObject({ enabled: true });
	expect(
		(await api.request<any>("GET", "/api/v1/virtual-playback-exclusion-zones"))
			.surfaces["vpb-api-surface"],
	).toEqual(zones);

	for (const number of [71, 72, 73]) await poolAction(api, number, "off");
	await Promise.all([
		poolAction(api, 71, "go", { surface: "virtual" }),
		poolAction(api, 72, "go", { surface: "virtual" }),
	]);
	const concurrent = (await playbacks(api)).active.filter(
		(entry: any) => [71, 72].includes(entry.playback_number) && entry.enabled,
	);
	expect(concurrent).toHaveLength(1);
	await poolAction(api, 73, "go", { surface: "virtual" });
	await poolAction(api, 72, "go", { surface: "virtual" });
	expect(await activePlayback(api, 71)).toMatchObject({ enabled: false });
	expect(await activePlayback(api, 72)).toMatchObject({ enabled: true });
	expect(await activePlayback(api, 73)).toMatchObject({ enabled: false });
	await bench.tick(0);
	expect(await visualizationLevel(api, prepared.fixtures[3])).toBeCloseTo(0, 5);
	expect(await visualizationLevel(api, prepared.fixtures[4])).toBeCloseTo(
		0.4,
		5,
	);
	expect(await visualizationLevel(api, prepared.fixtures[5])).toBeCloseTo(0, 5);
	await poolAction(api, 72, "off", { surface: "virtual" });
	expect(await activePlayback(api, 71)).toMatchObject({ enabled: false });
	expect(await activePlayback(api, 73)).toMatchObject({ enabled: false });
}

async function verifyFirstDeskPageAndOsc({
	api,
	bench,
}: VirtualZoneApiContext) {
	await api.request(
		"PUT",
		`/api/v1/control-desks/${api.session.desk.id}/page`,
		{ page: 2 },
	);
	for (const number of [71, 72, 73]) await poolAction(api, number, "off");
	await api.request(
		"POST",
		`/api/v1/control-desks/${api.session.desk.id}/page-playbacks/1/button`,
		{ button: 1, pressed: true, surface: "virtual" },
	);
	await api.request(
		"POST",
		`/api/v1/control-desks/${api.session.desk.id}/page-playbacks/2/button`,
		{ button: 1, pressed: true, surface: "virtual" },
	);
	expect(await activePlayback(api, 73)).toMatchObject({ enabled: false });
	expect(await activePlayback(api, 71)).toMatchObject({ enabled: true });
	await api.request(
		"PUT",
		`/api/v1/control-desks/${api.session.desk.id}/page`,
		{ page: 1 },
	);
	for (const number of [71, 72, 73]) await poolAction(api, number, "off");

	const firstHardware = await bench.osc();
	try {
		await firstHardware.subscribe("vpb-007-first", api.session.desk.osc_alias);
		await firstHardware.send(
			`/light/${api.session.desk.osc_alias}/page-playback/1/button/1`,
			[true],
		);
		await firstHardware.send(
			`/light/${api.session.desk.osc_alias}/page-playback/2/button/1`,
			[true],
		);
		await expect
			.poll(async () => (await activePlayback(api, 71))?.enabled)
			.toBe(false);
		await expect
			.poll(async () => (await activePlayback(api, 72))?.enabled)
			.toBe(true);
		expect(
			(await audit(api)).some(
				(event) =>
					event.kind === "playback_exclusion_applied" &&
					event.payload?.source === "osc" &&
					event.payload?.activated_playback === 72,
			),
		).toBe(true);
	} finally {
		await firstHardware.close();
	}
}

async function verifySecondDeskIsolation(
	{ api, bench }: VirtualZoneApiContext,
	{ prepared }: AuthoritativeVirtualZoneSetup,
) {
	const second = await api.request<Session>(
		"POST",
		"/api/v1/sessions",
		{ username: "Operator", client_id: crypto.randomUUID() },
		false,
	);
	api.session = second;
	expect(
		(await api.request<any>("GET", "/api/v1/virtual-playback-exclusion-zones"))
			.surfaces,
	).toEqual({});
	for (const number of [71, 72, 73]) await poolAction(api, number, "off");
	const secondHardware = await bench.osc();
	try {
		await secondHardware.subscribe("vpb-007-second", second.desk.osc_alias);
		await secondHardware.send(
			`/light/${second.desk.osc_alias}/page-playback/1/button/1`,
			[true],
		);
		await secondHardware.send(
			`/light/${second.desk.osc_alias}/page-playback/2/button/1`,
			[true],
		);
		await expect
			.poll(async () => (await activePlayback(api, 71))?.enabled)
			.toBe(true);
		await expect
			.poll(async () => (await activePlayback(api, 72))?.enabled)
			.toBe(true);
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
	await verifySecondDeskIsolation(context, setup);
};

export function registerVirtualZoneApiScenario(): void {
	test(
		"VPB-007 @supplemental @osc @restart › overlapping zones are serialized, desk-scoped, and durable on every transport",
		virtualZoneApiSupplement,
	);
}
