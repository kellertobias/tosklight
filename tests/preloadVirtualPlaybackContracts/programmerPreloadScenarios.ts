import {
	type BenchContractContext,
	type BenchUiContext,
	expect,
	test,
} from "../../apps/control-ui/e2e/bench/fixtures";
import {
	type PairedScenario,
	pairedScenario,
} from "../../apps/control-ui/e2e/bench/pairedScenario";
import { programmer } from "../support/catalog";
import {
	activePlayback,
	distinctGroupFixtures,
	longPressPreload,
	openPlaybackMode,
	type PreloadProgrammerPairState,
	playbackCard,
	poolAction,
	preloadProgrammerObservation,
	prepare,
	setCaptureMask,
	visualizationLevel,
} from "./support";

const preload001Scenario: PairedScenario<PreloadProgrammerPairState> = {
	id: "PRELOAD-001",
	title: "programmer-only Preload is blind, timed from GO, and releasable",
	arrange: async ({ api, bench }, surface) => {
		const prepared = await prepare(
			api,
			bench,
			`preload-001-paired-${surface}`,
			[
				{
					number: 31,
					fixture: 12,
					levels: [0.4],
					name: "Live physical sequence",
					buttons: ["go", "none", "none"],
					buttonCount: 1,
					hasFader: false,
				},
			],
			{ 1: 31 },
		);
		await setCaptureMask(api, true, false, false, 3_000, 7_000);
		const groupFixtures = await distinctGroupFixtures(api, "1", "2");
		return {
			...prepared,
			groupFixtures,
			beforeLevels: [
				await visualizationLevel(api, groupFixtures[0]),
				await visualizationLevel(api, groupFixtures[1]),
			],
		};
	},
	api: async ({ api, bench }, state) => {
		await api.command("preload.enter", {});
		await api.executeCommandLine("GROUP 1 AT 50");
		await api.executeCommandLine("GROUP 2 AT 70 TIME 1");
		await poolAction(api, 31, "button", {
			button: 1,
			pressed: true,
			surface: "physical",
		});
		state.pending = await preloadProgrammerObservation(
			api,
			state.groupFixtures,
		);
		state.applicationTimestamp = (await api.command<any>("preload.go", {}))
			.payload!.application_timestamp;
		await bench.tick(3_000);
		await api.command("preload.release", {});
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
		await desk.command("GROUP 1 AT 50", "G1 AT 50");
		await desk.command("GROUP 2 AT 70 TIME 1", "G2 AT 70 TIME 1");
		await openPlaybackMode(page);
		await playbackCard(page, 1)
			.getByRole("button", { name: "GO +", exact: true })
			.click();
		state.pending = await preloadProgrammerObservation(
			api,
			state.groupFixtures,
		);
		await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
		state.applicationTimestamp = (await programmer(api)).preload_group_active[
			"1"
		].intensity.changed_at;
		await bench.tick(3_000);
		await longPressPreload(page);
	},
	assert: async ({ api }, state) => {
		expect(state.pending).toEqual({
			blind: true,
			groupIds: ["1", "2"],
			groupValues: [],
			firstFadeMillis: 3_000,
			secondFadeMillis: 1_000,
			playbackActions: [],
			liveLevels: state.beforeLevels,
		});
		expect(state.applicationTimestamp).toEqual(expect.any(String));
		expect(await activePlayback(api, 31)).toMatchObject({
			enabled: true,
			current_cue_number: 1,
		});
		const finalProgrammer = await programmer(api);
		expect(finalProgrammer.preload_group_pending).toEqual({});
		expect(finalProgrammer.preload_group_active).toEqual({});
		expect(await visualizationLevel(api, state.groupFixtures[0])).toBeCloseTo(
			state.beforeLevels[0],
			5,
		);
		expect(await visualizationLevel(api, state.groupFixtures[1])).toBeCloseTo(
			state.beforeLevels[1],
			5,
		);
	},
};

const preload001ApiSupplement = async ({
	api,
	bench,
}: BenchContractContext) => {
	const prepared = await prepare(
		api,
		bench,
		"preload-001-programmer",
		[
			{
				number: 30,
				fixture: 12,
				levels: [0.2, 0.8],
				name: "Live physical sequence",
			},
		],
		{ 1: 30 },
	);
	await setCaptureMask(api, true, false, false, 3_000, 7_000);
	const [group1Fixture, group2Fixture] = await distinctGroupFixtures(
		api,
		"1",
		"2",
	);
	const before1 = await visualizationLevel(api, group1Fixture);
	const before2 = await visualizationLevel(api, group2Fixture);

	await api.command("preload.enter", {});
	await api.executeCommandLine("GROUP 1 AT 50");
	await api.executeCommandLine("GROUP 2 AT 70 TIME 1");
	const pending = await programmer(api);
	expect(pending.blind).toBe(true);
	expect(pending.preload_group_pending["1"].intensity.fade_millis).toBe(3_000);
	expect(pending.preload_group_pending["2"].intensity).toMatchObject({
		fade_millis: 1_000,
	});
	expect(pending.group_values).toEqual({});
	expect(await visualizationLevel(api, group1Fixture)).toBeCloseTo(before1, 5);
	expect(await visualizationLevel(api, group2Fixture)).toBeCloseTo(before2, 5);

	await poolAction(api, 30, "go", { surface: "physical" });
	expect((await activePlayback(api, 30))?.current_cue_number).toBe(1);
	expect((await programmer(api)).preload_playback_pending).toEqual([]);
	await bench.tick(2_000);
	expect(await visualizationLevel(api, group1Fixture)).toBeCloseTo(before1, 5);
	expect(await visualizationLevel(api, group2Fixture)).toBeCloseTo(before2, 5);

	const committed = (await api.command<any>("preload.go", {})).payload!;
	const activeProgrammer = await programmer(api);
	expect(activeProgrammer.preload_group_active["1"].intensity.changed_at).toBe(
		committed.application_timestamp,
	);
	expect(activeProgrammer.preload_group_active["2"].intensity.changed_at).toBe(
		committed.application_timestamp,
	);
	expect(committed.playback_actions).toEqual([]);
	await bench.tick(1_000);
	expect(await visualizationLevel(api, group1Fixture)).toBeCloseTo(1 / 6, 2);
	expect(await visualizationLevel(api, group2Fixture)).toBeCloseTo(0.7, 2);
	await bench.tick(2_000);
	expect(await visualizationLevel(api, group1Fixture)).toBeCloseTo(0.5, 2);
	expect(await visualizationLevel(api, group2Fixture)).toBeCloseTo(0.7, 2);

	expect((await api.command<any>("preload.release", {})).payload).toMatchObject(
		{ released: true },
	);
	expect(await visualizationLevel(api, group1Fixture)).toBeCloseTo(before1, 5);
	expect(await visualizationLevel(api, group2Fixture)).toBeCloseTo(before2, 5);
	expect((await activePlayback(api, 30))?.current_cue_number).toBe(1);
	expect(prepared.fixtures[12]).toBeTruthy();
};

const preload001UiSupplement = async ({
	api,
	bench,
	desk,
	page,
}: BenchUiContext) => {
	await prepare(
		api,
		bench,
		"preload-001-ui",
		[
			{
				number: 31,
				fixture: 12,
				levels: [0.4],
				name: "Live GO",
				buttons: ["go", "none", "none"],
				buttonCount: 1,
				hasFader: false,
			},
		],
		{ 1: 31 },
	);
	await setCaptureMask(api, true, false, false, 3_000, 7_000);
	await desk.open(bench.baseUrl);
	await desk.recordStep(
		"ARM PROGRAMMER PRELOAD",
		"Only programmer changes are blind; the physical GO remains live.",
	);
	await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
	await desk.command("GROUP 1 AT 50", "G1 AT 50");
	await desk.command("GROUP 2 AT 70 TIME 1", "G2 AT 70 TIME 1");
	await expect(page.getByTitle(/Pending Preload: PROG 2/)).toBeVisible();
	await openPlaybackMode(page);
	await playbackCard(page, 1)
		.getByRole("button", { name: "GO +", exact: true })
		.click();
	await expect
		.poll(async () => (await activePlayback(api, 31))?.current_cue_number)
		.toBe(1);
	await desk.recordStep(
		"COMMIT AT ONE MARK",
		"The explicit 1 s value and the 3 s Programmer Fade fallback start now.",
	);
	await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
	await bench.tick(3_000);
	const state = await programmer(api);
	expect(state.preload_group_active["1"].intensity.value.value).toBeCloseTo(
		0.5,
		5,
	);
	expect(state.preload_group_active["2"].intensity.fade_millis).toBe(1_000);
};

export function registerProgrammerPreloadScenarios(): void {
	pairedScenario(preload001Scenario);
	test(
		"PRELOAD-001 @supplemental › API timing and source ownership at exact virtual-time checkpoints",
		preload001ApiSupplement,
	);
	test(
		"PRELOAD-001 @supplemental-ui › the command line exposes detailed pending programmer timing",
		preload001UiSupplement,
	);
}
