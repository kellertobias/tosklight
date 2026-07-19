import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import {
	type PairedScenario,
	pairedScenario,
} from "../../apps/control-ui/e2e/bench/pairedScenario";
import {
	activePlayback,
	addVirtualPlaybackPane,
	configuration,
	normalizedVirtualZones,
	poolAction,
	prepare,
	type VirtualZonePairState,
	visualizationLevel,
} from "./support";

const virtualZoneScenario: PairedScenario<VirtualZonePairState> = {
	id: "VPB-007",
	title:
		"named Virtual Playback exclusion zones are inert on creation and authoritative on activation",
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
			{ 1: 74, 2: 75, 3: 76 },
		);
		await api.request("PUT", "/api/v1/configuration", {
			...(await configuration(api)),
			sequence_master_fade_millis: 0,
		});
		await poolAction(api, 74, "on", { surface: "virtual" });
		await poolAction(api, 75, "on", { surface: "virtual" });
		return prepared;
	},
	api: async ({ api }, state) => {
		await api.request(
			"PUT",
			"/api/v1/virtual-playback-exclusion-zones/vpb-paired-surface",
			{
				zones: [{ id: "touring-pair", name: "Touring pair", slots: [1, 2] }],
			},
		);
		state.savedZones = await normalizedVirtualZones(api);
		state.creationState = [
			Boolean((await activePlayback(api, 74))?.enabled),
			Boolean((await activePlayback(api, 75))?.enabled),
		];
		for (const number of [74, 75, 74, 75])
			await poolAction(api, number, "toggle", { surface: "virtual" });
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
			Boolean((await activePlayback(api, 74))?.enabled),
			Boolean((await activePlayback(api, 75))?.enabled),
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
		expect(await activePlayback(api, 74)).toMatchObject({ enabled: false });
		expect(await activePlayback(api, 75)).toMatchObject({ enabled: true });
		expect((await activePlayback(api, 76))?.enabled ?? false).toBe(false);
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
