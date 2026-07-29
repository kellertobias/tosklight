import { expect } from "../bench/core/fixtures";
import {
	type PairedScenario,
	pairedScenario,
} from "../bench/core/pairedScenario";
import {
	activeVirtualPlayback,
	addVirtualPlaybackPane,
	configuration,
	normalizedVirtualZones,
	prepare,
	saveVirtualZoneSurface,
	type VirtualZonePairState,
	virtualAction,
	visualizationLevel,
	writeVirtualPage,
} from "./support";

const virtualZoneScenario: PairedScenario<VirtualZonePairState> = {
	id: "VPB-007",
	title:
		"named Virtual Playback exclusion zones are inert on creation and authoritative on activation",
	surfaces: ["api"],
	arrange: async ({ api, bench }, surface) => {
		const prepared = await prepare(
			api,
			bench,
			`vpb-007-paired-${surface}`,
			[
				{
					number: 74,
					fixture: 3,
					levels: [0.25],
					name: "Touring A",
					buttons: ["toggle", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
				{
					number: 75,
					fixture: 4,
					levels: [0.5],
					name: "Touring B",
					buttons: ["toggle", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
				{
					number: 76,
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
		await writeVirtualPage(api, 1, {
			1001: 74,
			1002: 75,
			1003: 76,
		});
		await api.request("PUT", "/api/v2/configuration", {
			...(await configuration(api)),
			sequence_master_fade_millis: 0,
		});
		await virtualAction(api, 1, 1001, "on");
		await virtualAction(api, 1, 1002, "on");
		return prepared;
	},
	api: async ({ api }, state) => {
		await saveVirtualZoneSurface(api, "vpb-paired-surface", [
			{ id: "touring-pair", name: "Touring pair", slots: [1, 2] },
		]);
		state.savedZones = await normalizedVirtualZones(api);
		state.creationState = [
			Boolean((await activeVirtualPlayback(api, 1, 1001))?.enabled),
			Boolean((await activeVirtualPlayback(api, 1, 1002))?.enabled),
		];
		for (const number of [1001, 1002, 1001, 1002])
			await virtualAction(api, 1, number, "toggle");
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		const pane = await addVirtualPlaybackPane(page);
		await page.keyboard.down("Shift");
		await pane
			.getByRole("button", { name: /Virtual playback page 1 cell 1 Touring A/ })
			.click();
		await pane
			.getByRole("button", { name: /Virtual playback page 1 cell 2 Touring B/ })
			.click();
		await page.keyboard.up("Shift");
		await pane.getByRole("button", { name: "Create Exclusion Zone" }).click();
		const create = page.getByRole("dialog", { name: "Create Exclusion Zone" });
		await create.getByLabel("Zone name").fill("Touring pair");
		await create.getByRole("button", { name: "Create zone" }).click();
		await expect(create).toBeHidden();
		state.savedZones = await normalizedVirtualZones(api);
		state.creationState = [
			Boolean((await activeVirtualPlayback(api, 1, 1001))?.enabled),
			Boolean((await activeVirtualPlayback(api, 1, 1002))?.enabled),
		];
		for (const cell of [1, 2, 1, 2])
			await pane
				.getByRole("button", {
					name: new RegExp(`Virtual playback page 1 cell ${cell}`),
				})
				.click();
	},
	assert: async ({ api, bench }, state) => {
		expect(state.savedZones).toEqual([{ name: "Touring pair", slots: [1, 2] }]);
		expect(state.creationState).toEqual([true, true]);
		expect(await activeVirtualPlayback(api, 1, 1001)).toMatchObject({
			enabled: false,
		});
		expect(await activeVirtualPlayback(api, 1, 1002)).toMatchObject({
			enabled: true,
		});
		expect(
			(await activeVirtualPlayback(api, 1, 1003))?.enabled ?? false,
		).toBe(false);
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
