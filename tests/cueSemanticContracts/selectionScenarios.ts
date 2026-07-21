import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import {
	clearProgrammerValues,
	setProgrammerFixtureValue,
} from "../../apps/control-ui/e2e/bench/programmerValues";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	pressCommand,
} from "../support/catalog";
import {
	assertCompactGroupSequence,
	fixtureCue,
	groupCue,
	groupValues,
	installCompactGroups,
	installPlaybackSequence,
	playbackState,
	registerPairedCueScenario,
	runtime,
	setSequenceMasterFade,
} from "./support";

const PROGRAMMER_TIMING = {
	fade: true,
	fadeMillis: 0,
	delayMillis: null,
} as const;

registerPairedCueScenario<{ completed: boolean; showId: string }>({
	id: "CUE-006",
	title:
		"explicit playback selection supplies the implicit Cuelist without following execution order",
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			surface === "api"
				? "cue-006-active-playback"
				: "cue-006-active-playback-ui",
			"compact-rig",
		);
		return { completed: false, showId: show.id };
	},
	api: async ({ api, bench }, state) => {
		await setSequenceMasterFade(api, 0);
		const fixtures = await fixtureIdsByNumber(api);
		const first = await installPlaybackSequence(api, 1, [
			fixtureCue(1, [[fixtures[1], "intensity", 0.2]]),
		]);
		const second = await installPlaybackSequence(api, 2, [
			fixtureCue(1, [[fixtures[2], "intensity", 0.3]]),
		]);
		await api.request("POST", "/api/v1/cuelists/2/select", {});
		expect((await playbackState(api)).selected_playback).toBe(2);
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		expect((await playbackState(api)).selected_playback).toBe(2);

		await setProgrammerFixtureValue(api, {
			surface: "api",
			showId: state.showId,
			fixtureId: fixtures[3],
			attribute: "intensity",
			value: { kind: "normalized", value: 0.7 },
			timing: PROGRAMMER_TIMING,
		});
		await api.executeCommandLine("RECORD CUE 7");
		expect(
			(await object<any>(api, "cue_list", second.id)).body.cues.map(
				(cue: any) => cue.number,
			),
		).toEqual([1, 7]);
		expect(
			(await object<any>(api, "cue_list", first.id)).body.cues.map(
				(cue: any) => cue.number,
			),
		).toEqual([1]);

		await clearProgrammerValues(api, {
			surface: "api",
			showId: state.showId,
		});
		await setProgrammerFixtureValue(api, {
			surface: "api",
			showId: state.showId,
			fixtureId: fixtures[4],
			attribute: "intensity",
			value: { kind: "normalized", value: 0.6 },
			timing: PROGRAMMER_TIMING,
		});
		await api.executeCommandLine("RECORD SET 1 CUE 8");
		expect(
			(await object<any>(api, "cue_list", first.id)).body.cues.map(
				(cue: any) => cue.number,
			),
		).toEqual([1, 8]);
		expect((await playbackState(api)).selected_playback).toBe(2);
		state.completed = true;
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await setSequenceMasterFade(api, 0);
		const fixtures = await fixtureIdsByNumber(api);
		const first = await installPlaybackSequence(
			api,
			1,
			[fixtureCue(1, [[fixtures[1], "intensity", 0.2]])],
			{ name: "Selection One" },
		);
		const second = await installPlaybackSequence(
			api,
			2,
			[fixtureCue(1, [[fixtures[2], "intensity", 0.3]])],
			{ name: "Selection Two" },
		);
		await desk.open(bench.baseUrl);
		api.session = await desk.session();
		await page.locator(".mode-toggle").click();
		await page.keyboard.press("Shift+KeyZ");
		await expect(page.getByLabel("Command line")).toHaveValue("SELECT");
		const firstCard = page
			.locator(".playback-fader-bank article")
			.filter({ hasText: "Selection One" });
		const secondCard = page
			.locator(".playback-fader-bank article")
			.filter({ hasText: "Selection Two" });
		await secondCard.getByRole("button", { name: "GO +", exact: true }).click();
		await expect(secondCard).toHaveAttribute("data-selected-playback", "true");
		expect((await playbackState(api)).active).toHaveLength(0);
		await firstCard.getByRole("button", { name: "GO +", exact: true }).click();
		await expect
			.poll(async () => runtime(api, 1))
			.toMatchObject({ current_cue_number: 1, enabled: true });
		expect((await playbackState(api)).selected_playback).toBe(2);
		await setProgrammerFixtureValue(api, {
			surface: "api",
			showId: state.showId,
			fixtureId: fixtures[3],
			attribute: "intensity",
			value: { kind: "normalized", value: 0.7 },
			timing: PROGRAMMER_TIMING,
		});
		await page.locator(".mode-toggle").click();
		await pressCommand(page, "RECORD CUE 7", "RECORD CUE 7");
		await expect
			.poll(async () =>
				(await object<any>(api, "cue_list", second.id)).body.cues.map(
					(cue: any) => cue.number,
				),
			)
			.toEqual([1, 7]);
		expect(
			(await object<any>(api, "cue_list", first.id)).body.cues.map(
				(cue: any) => cue.number,
			),
		).toEqual([1]);
		expect((await playbackState(api)).selected_playback).toBe(2);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
});

registerPairedCueScenario<{ completed: boolean }>({
	id: "CUE-007",
	title:
		"explicit zeroes block a later inserted on Cue from tracking past Cue 4",
	arrange: () => ({ completed: false }),
	api: async ({ api, bench }, state) => {
		await loadCanonicalCopy(api, bench, "cue-007-explicit-off", "compact-rig");
		await installCompactGroups(api);
		await setSequenceMasterFade(api, 0);
		const installed = await installPlaybackSequence(api, 1, [
			groupCue(1, [["1", "intensity", 1]]),
			groupCue(2, [["1", "intensity", 0]]),
			groupCue(3, [["2", "intensity", 1]]),
			groupCue(3.5, [["1", "intensity", 1]]),
			groupCue(4, [["1", "intensity", 0]]),
			groupCue(5, [["3", "intensity", 1]]),
		]);
		const stored = await object<any>(api, "cue_list", installed.id);
		expect(
			groupValues(stored.body.cues.find((cue: any) => cue.number === 2)),
		).toEqual({ "1:intensity": 0 });
		expect(
			groupValues(stored.body.cues.find((cue: any) => cue.number === 4)),
		).toEqual({ "1:intensity": 0 });
		const expected = [
			[1, 0, 0],
			[0, 0, 0],
			[0, 1, 0],
			[1, 1, 0],
			[0, 1, 0],
			[0, 1, 1],
		];
		await assertCompactGroupSequence(bench, expected, () =>
			api.request("POST", "/api/v1/cuelists/1/go", {}),
		);
		state.completed = true;
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await loadCanonicalCopy(
			api,
			bench,
			"cue-007-explicit-off-ui",
			"compact-rig",
		);
		await installCompactGroups(api);
		await setSequenceMasterFade(api, 0);
		await installPlaybackSequence(api, 1, [
			groupCue(1, [["1", "intensity", 1]]),
			groupCue(2, [["1", "intensity", 0]]),
			groupCue(3, [["2", "intensity", 1]]),
			groupCue(3.5, [["1", "intensity", 1]]),
			groupCue(4, [["1", "intensity", 0]]),
			groupCue(5, [["3", "intensity", 1]]),
		]);
		await desk.open(bench.baseUrl);
		await page.locator(".mode-toggle").click();
		const go = page
			.locator(".playback-fader-bank article")
			.filter({ hasText: "Playback 1" })
			.getByRole("button", { name: "GO +", exact: true });
		await assertCompactGroupSequence(
			bench,
			[
				[1, 0, 0],
				[0, 0, 0],
				[0, 1, 0],
				[1, 1, 0],
				[0, 1, 0],
				[0, 1, 1],
			],
			() => go.click(),
		);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
});
