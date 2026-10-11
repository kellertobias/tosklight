import { paneCard } from "./uiSupport";
import { expect } from "../bench/core/fixtures";
import {
	type PairedScenario,
	pairedScenario,
} from "../bench/core/pairedScenario";
import {
	activeAssignedPlayback,
	addVirtualPlaybackPane,
	configuration,
	normalizedSoloRegions,
	prepare,
	saveSoloRegions,
	type VirtualZonePairState,
	paneAction,
	visualizationLevel,
	copyPageAssignments,
} from "./support";

const virtualZoneScenario: PairedScenario<VirtualZonePairState> = {
	id: "VPB-007",
	title:
		"show-owned Virtual Playback-number exclusion zones are inert on creation and authoritative on activation",
	surfaces: ["api"],
	arrange: async ({ api, bench }, surface) => {
		const prepared = await prepare(
			api,
			bench,
			`vpb-007-paired-${surface}`,
			[
				{
					position: 74,
					fixture: 3,
					levels: [0.25],
					name: "Touring A",
					buttons: ["toggle", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
				{
					position: 75,
					fixture: 4,
					levels: [0.5],
					name: "Touring B",
					buttons: ["toggle", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
				{
					position: 76,
					fixture: 5,
					levels: [0.75],
					name: "Touring C",
					buttons: ["toggle", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
			],
			{},
		);
		await copyPageAssignments(api, 1, {
			401: 74,
			402: 75,
			403: 76,
		});
		await api.request("POST", "/api/v2/configuration/update", {request_id:crypto.randomUUID(),patch:{sequence_master_fade_millis: 0,
		}});
		await paneAction(api, {page: 1, position: 401}, "on");
		await paneAction(api, {page: 1, position: 402}, "on");
		return prepared;
	},
	api: async ({ api }, state) => {
		await saveSoloRegions(api, [
			{
				id: "11111111-1111-4111-8111-111111111111",
				name: "Touring pair",
				members: [{page:1,position:401}, {page:1,position:402}],
			},
		]);
		state.savedZones = await normalizedSoloRegions(api);
		state.creationState = [
			Boolean((await activeAssignedPlayback(api, {page: 1, position: 401}))?.enabled),
			Boolean((await activeAssignedPlayback(api, {page: 1, position: 402}))?.enabled),
		];
		for (const number of [401, 402, 401, 402])
			await paneAction(api, {page: 1, position: number}, "toggle");
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		const pane = await addVirtualPlaybackPane(page);
		await pane.getByRole("button",{name:"Select Solo members"}).click();
 await paneCard(pane,{page:1,position:401}).click();
 await paneCard(pane,{page:1,position:402}).click();
 await pane.getByLabel("Solo Region name").fill("Touring pair");
 await pane.getByRole("button",{name:"Create Solo Region",exact:true}).click();
		state.savedZones = await normalizedSoloRegions(api);
		state.creationState = [
			Boolean((await activeAssignedPlayback(api, {page: 1, position: 401}))?.enabled),
			Boolean((await activeAssignedPlayback(api, {page: 1, position: 402}))?.enabled),
		];
		for (const cell of [1,2,1,2]) await paneCard(pane,{page:1,position:400+cell}).getByRole("button",{name:"TOGGLE",exact:true}).click();
	},
	assert: async ({ api, bench }, state) => {
		expect(state.savedZones).toEqual([
			{ name: "Touring pair", members: [{page:1,position:401}, {page:1,position:402}] },
		]);
		expect(state.creationState).toEqual([true, true]);
		expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toMatchObject({
			enabled: false,
		});
		expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toMatchObject({
			enabled: true,
		});
		expect((await activeAssignedPlayback(api, {page: 1, position: 403}))?.enabled ?? false).toBe(
			false,
		);
		await bench.tick(0);
		expect(await visualizationLevel(api, state.fixtures[3])).toBeCloseTo(0, 5);
		expect(await visualizationLevel(api, state.fixtures[4])).toBeCloseTo(
			0.5,
			5,
		);
		expect(await visualizationLevel(api, state.fixtures[5])).toBeCloseTo(0, 5);
	},
};

export function registerVirtualZonePairScenario(): void {
	pairedScenario(virtualZoneScenario);
}
