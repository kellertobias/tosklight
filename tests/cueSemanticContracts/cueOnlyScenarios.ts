import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import {
	clearProgrammerValues,
	setProgrammerGroupValue,
} from "../../apps/control-ui/e2e/bench/programmerValues";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
} from "../support/catalog";
import {
	fixtureCue,
	groupCue,
	groupValues,
	installCompactGroups,
	installPlaybackSequence,
	logicalSlots,
	registerPairedCueScenario,
	setCueOnlyFromUi,
	setSequenceMasterFade,
	visualizationLevel,
} from "./support";

const PROGRAMMER_TIMING = {
	fade: true,
	fadeMillis: 0,
	delayMillis: null,
} as const;

registerPairedCueScenario<{ completed: boolean; showId: string }>({
	id: "CUE-002",
	title:
		"Cue-only restoration reconstructs identically for sequential GO and direct jumps",
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			surface === "api" ? "cue-002-cue-only" : "cue-002-visible-cue-only",
			"compact-rig",
		);
		return { completed: false, showId: show.id };
	},
	api: async ({ api, bench }, state) => {
		await setSequenceMasterFade(api, 0);
		const fixtures = await fixtureIdsByNumber(api);
		const installed = await installPlaybackSequence(api, 1, [
			fixtureCue(1, [[fixtures[1], "intensity", 0.3]]),
			fixtureCue(2, [[fixtures[1], "intensity", 0.8]]),
			fixtureCue(3, [
				[fixtures[1], "intensity", 0.3, { automatic_restore: true }],
				[fixtures[2], "intensity", 0.6],
			]),
		]);
		expect(
			(await object<any>(api, "cue_list", installed.id)).body.cues[2].changes[0]
				.automatic_restore,
		).toBe(true);

		const sequential: Array<[number, number]> = [];
		for (let index = 0; index < 3; index += 1) {
			await api.request("POST", "/api/v1/cuelists/1/go", {});
			await bench.tick(0);
			sequential.push([
				await visualizationLevel(api, fixtures[1], "intensity"),
				await visualizationLevel(api, fixtures[2], "intensity"),
			]);
		}
		expect(sequential).toEqual([
			[0.3, 0],
			[0.8, 0],
			[0.3, 0.6],
		]);

		const direct: Array<[number, number]> = [];
		for (const cueNumber of [1, 2, 3]) {
			await api.request("POST", "/api/v1/cuelists/1/off", {});
			await api.request("POST", "/api/v1/cuelists/1/go-to", {
				cue_number: cueNumber,
			});
			await bench.tick(0);
			direct.push([
				await visualizationLevel(api, fixtures[1], "intensity"),
				await visualizationLevel(api, fixtures[2], "intensity"),
			]);
		}
		expect(direct).toEqual(sequential);
		state.completed = true;
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await installCompactGroups(api);
		await setSequenceMasterFade(api, 0);
		const installed = await installPlaybackSequence(
			api,
			1,
			[groupCue(1, [["1", "intensity", 0.3]])],
			{ priority: 100 },
		);
		await setProgrammerGroupValue(api, {
			surface: "api",
			showId: state.showId,
			groupId: "1",
			attribute: "intensity",
			value: { kind: "normalized", value: 0.8 },
			timing: PROGRAMMER_TIMING,
		});
		await desk.open(bench.baseUrl);
		await page.locator(".mode-toggle").click();
		await setCueOnlyFromUi(page, true);
		await page.getByRole("button", { name: "REC", exact: true }).click();
		const card = page
			.locator(".playback-fader-bank article")
			.filter({ hasText: "Playback 1" });
		await card
			.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			})
			.click();
		await expect
			.poll(
				async () =>
					(await object<any>(api, "cue_list", installed.id)).body.cues.length,
			)
			.toBe(2);
		await api.request("POST", `/api/v1/shows/${state.showId}/open`, {
			transition: "hold_current",
		});
		await expect
			.poll(
				async () =>
					(await object<any>(api, "cue_list", installed.id)).body.cues[1]
						.cue_only,
			)
			.toBe(true);

		await clearProgrammerValues(api, {
			surface: "api",
			showId: state.showId,
		});
		await setProgrammerGroupValue(api, {
			surface: "api",
			showId: state.showId,
			groupId: "2",
			attribute: "intensity",
			value: { kind: "normalized", value: 0.6 },
			timing: PROGRAMMER_TIMING,
		});
		await setCueOnlyFromUi(page, false);
		await page.getByRole("button", { name: "REC", exact: true }).click();
		await card
			.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			})
			.click();
		await expect
			.poll(
				async () =>
					(await object<any>(api, "cue_list", installed.id)).body.cues.length,
			)
			.toBe(3);

		const stored = await object<any>(api, "cue_list", installed.id);
		expect(stored.body.cues[1].cue_only).toBe(true);
		expect(stored.body.cues[2].cue_only).toBe(false);
		expect(groupValues(stored.body.cues[2])).toEqual({
			"1:intensity": 0.3,
			"2:intensity": 0.6,
		});
		expect(
			stored.body.cues[2].group_changes.find(
				(change: any) => change.group_id === "1",
			),
		).toMatchObject({ automatic_restore: true });

		await clearProgrammerValues(api, {
			surface: "api",
			showId: state.showId,
		});
		await api.request("POST", "/api/v1/cuelists/1/off", {});
		const states: number[][] = [];
		for (let index = 0; index < 3; index += 1) {
			await api.request("POST", "/api/v1/cuelists/1/go", {});
			states.push(logicalSlots(await bench.tick(0), 8));
		}
		expect(states).toEqual([
			[...Array(4).fill(77), ...Array(4).fill(0)],
			[...Array(4).fill(204), ...Array(4).fill(0)],
			[...Array(4).fill(77), ...Array(4).fill(153)],
		]);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
});
