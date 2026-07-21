import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import { deleteCue } from "../../apps/control-ui/e2e/bench/cueDeletion";
import {
	clearProgrammerValues,
	setProgrammerFixtureValue,
} from "../../apps/control-ui/e2e/bench/programmerValues";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	putObject,
} from "../support/catalog";
import {
	CUE_SEMANTIC_CONTRACTS,
	fixtureCue,
	groupCue,
	installCompactGroups,
	installPlaybackSequence,
	logicalSlots,
	registerPairedCueScenario,
	rgbValues,
	runtime,
	setSequenceMasterFade,
	visualizationLevel,
} from "./support";

const PROGRAMMER_TIMING = {
	fade: true,
	fadeMillis: 0,
	delayMillis: null,
} as const;

registerPairedCueScenario<{ completed: boolean; showId: string }>({
	id: "CUE-010",
	title:
		"tracking and LTP ownership stay per attribute and reveal the underlying programmer",
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			surface === "api"
				? "cue-010-attribute-tracking"
				: "cue-010-attribute-tracking-ui",
			"compact-rig",
		);
		return { completed: false, showId: show.id };
	},
	api: async ({ api, bench }, state) => {
		await setSequenceMasterFade(api, 0);
		const fixtures = await fixtureIdsByNumber(api);
		const rgb = fixtures[21];
		await installPlaybackSequence(
			api,
			1,
			[
				fixtureCue(1, [[rgb, "intensity", 1]]),
				fixtureCue(2, [[rgb, "intensity", 0.5]]),
				fixtureCue(3, [
					[rgb, "red", 0],
					[rgb, "green", 0],
					[rgb, "blue", 1],
				]),
				fixtureCue(4, [[fixtures[2], "intensity", 0.4]]),
			],
			{ priority: 100 },
		);
		for (const [attribute, value] of [
			["red", 0],
			["green", 1],
			["blue", 0],
		] as const)
			await setProgrammerFixtureValue(api, {
				surface: "api",
				showId: state.showId,
				fixtureId: rgb,
				attribute,
				value: { kind: "normalized", value },
				timing: PROGRAMMER_TIMING,
			});
		await bench.tick(1);

		await api.request("POST", "/api/v1/cuelists/1/go", {});
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		await bench.tick(0);
		expect(await visualizationLevel(api, rgb, "intensity")).toBe(0.5);
		expect(await rgbValues(api, rgb)).toEqual([0, 1, 0]);

		await api.request("POST", "/api/v1/cuelists/1/go", {});
		await bench.tick(0);
		expect(await visualizationLevel(api, rgb, "intensity")).toBe(0.5);
		expect(await rgbValues(api, rgb)).toEqual([0, 0, 1]);
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		await bench.tick(0);
		expect(await visualizationLevel(api, rgb, "intensity")).toBe(0.5);
		expect(await rgbValues(api, rgb)).toEqual([0, 0, 1]);

		await api.request("POST", "/api/v1/cuelists/1/off", {});
		await bench.tick(0);
		expect(await rgbValues(api, rgb)).toEqual([0, 1, 0]);
		state.completed = true;
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await setSequenceMasterFade(api, 0);
		const fixtures = await fixtureIdsByNumber(api);
		const rgb = fixtures[21];
		await installPlaybackSequence(
			api,
			1,
			[
				fixtureCue(1, [[rgb, "intensity", 1]]),
				fixtureCue(2, [[rgb, "intensity", 0.5]]),
				fixtureCue(3, [
					[rgb, "red", 0],
					[rgb, "green", 0],
					[rgb, "blue", 1],
				]),
				fixtureCue(4, [[fixtures[2], "intensity", 0.4]]),
			],
			{ priority: 100 },
		);
		const definition = await object<any>(api, "playback", "1");
		await putObject(
			api,
			"playback",
			"1",
			{ ...definition.body, buttons: ["go_minus", "go", "off"] },
			definition.revision,
		);
		for (const [attribute, value] of [
			["red", 0],
			["green", 1],
			["blue", 0],
		] as const)
			await setProgrammerFixtureValue(api, {
				surface: "api",
				showId: state.showId,
				fixtureId: rgb,
				attribute,
				value: { kind: "normalized", value },
				timing: PROGRAMMER_TIMING,
			});
		await bench.tick(1);
		await desk.open(bench.baseUrl);
		await page.locator(".mode-toggle").click();
		const card = page
			.locator(".playback-fader-bank article")
			.filter({ hasText: "Playback 1" });
		const go = card.getByRole("button", { name: "GO +", exact: true });
		await go.click();
		await go.click();
		await bench.tick(0);
		expect(await visualizationLevel(api, rgb, "intensity")).toBe(0.5);
		expect(await rgbValues(api, rgb)).toEqual([0, 1, 0]);
		await go.click();
		await bench.tick(0);
		expect(await visualizationLevel(api, rgb, "intensity")).toBe(0.5);
		expect(await rgbValues(api, rgb)).toEqual([0, 0, 1]);
		await go.click();
		await bench.tick(0);
		expect(await rgbValues(api, rgb)).toEqual([0, 0, 1]);
		await card.getByRole("button", { name: "OFF", exact: true }).click();
		await bench.tick(0);
		expect(await rgbValues(api, rgb)).toEqual([0, 1, 0]);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
});

test.describe(CUE_SEMANTIC_CONTRACTS, () => {
	test("CUE-013 @supplemental-api › inactive deletion is output-neutral and both sole-Cue safeguards are atomic", async ({
		api,
		bench,
	}) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			"cue-013-inactive-delete",
			"compact-rig",
		);
		await installCompactGroups(api);
		await setSequenceMasterFade(api, 0);
		const installed = await installPlaybackSequence(api, 1, [
			groupCue(1, [["1", "intensity", 1]]),
			groupCue(2, [["2", "intensity", 1]]),
			groupCue(3, [["3", "intensity", 1]]),
		]);
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		const beforeRuntime = await runtime(api, 1);
		const beforeSlots = logicalSlots(await bench.tick(0), 12);
		const before = await object<any>(api, "cue_list", installed.id);
		await putObject(
			api,
			"cue_list",
			installed.id,
			{
				...before.body,
				cues: before.body.cues.filter((cue: any) => cue.number !== 3),
			},
			before.revision,
		);
		expect(await runtime(api, 1)).toMatchObject({
			current_cue_number: 1,
			activated_at: beforeRuntime.activated_at,
		});
		expect(logicalSlots(await bench.tick(0), 12)).toEqual(beforeSlots);

		await deleteCue(api, {
			surface: "api",
			address: { type: "pool", playbackNumber: 1 },
			cueNumber: 1,
		});
		expect(
			(await object<any>(api, "cue_list", installed.id)).body.cues.map(
				(cue: any) => cue.number,
			),
		).toEqual([2]);
		expect(await runtime(api, 1)).toMatchObject({
			current_cue_number: 1,
			deleted_cue_hold: { deleted_number: 1, next_number: 2 },
			normal_next_cue_number: 2,
		});
		expect(logicalSlots(await bench.tick(0), 12)).toEqual(beforeSlots);
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		expect(await runtime(api, 1)).toMatchObject({ current_cue_number: 2 });
		expect(logicalSlots(await bench.tick(0), 12)).toEqual([
			...Array(4).fill(0),
			...Array(4).fill(255),
			...Array(4).fill(0),
		]);

		const fixtures = await fixtureIdsByNumber(api);
		const sole = await installPlaybackSequence(api, 2, [
			fixtureCue(1, [[fixtures[1], "intensity", 0.2]]),
		]);
		const soleBefore = await object<any>(api, "cue_list", sole.id);
		await expect(
			deleteCue(api, {
				surface: "api",
				address: { type: "pool", playbackNumber: 2 },
				cueNumber: 1,
			}),
		).rejects.toThrow();
		expect((await object<any>(api, "cue_list", sole.id)).body).toEqual(
			soleBefore.body,
		);
		await clearProgrammerValues(api, { surface: "api", showId: show.id });
		await expect(
			api.executeCommandLine("RECORD - SET 2 CUE 1"),
		).rejects.toThrow();
		expect((await object<any>(api, "cue_list", sole.id)).body).toEqual(
			soleBefore.body,
		);
	});
});
