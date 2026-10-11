import type { PairedScenario } from "../bench/core/pairedScenario";
import { expect, test } from "../bench/core/fixtures";
import {
	clearProgrammerValues,
	setProgrammerFixtureValue,
} from "../bench/programmer/programmerValues";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
} from "../support/catalog";
import {
	assertCompactGroupSequence,
	fixtureCue,
	groupCue,
	groupValues,
	installCompactGroups,
	installPlaybackSequence,
	playbackState,
	setSequenceMasterFade,
} from "./support";

const PROGRAMMER_TIMING = {
	fade: true,
	fadeMillis: 0,
	delayMillis: null,
} as const;

const apiCUE006Contract: Omit<PairedScenario<{ completed: boolean; showId: string }>, "ui"> = {
	id: "CUE-006",
	title:
		"explicit playback selection supplies the implicit Cuelist without following execution order",
	surfaces: ["api"],
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
		const first = await installPlaybackSequence(api, { page: 1, position: 1 }, [
			fixtureCue(1, [[fixtures[1], "intensity", 0.2]]),
		]);
		const second = await installPlaybackSequence(api, { page: 1, position: 2 }, [
			fixtureCue(1, [[fixtures[2], "intensity", 0.3]]),
		]);
		await api.playbackAction({ page: 1, position: 2 }, "select", {});
		expect((await playbackState(api)).selected_playback).toEqual({ page: 1, position: 2 });
		await api.playbackAction({ page: 1, position: 1 }, "go", {});
		expect((await playbackState(api)).selected_playback).toEqual({ page: 1, position: 2 });

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
		).toEqual(["1", "7"]);
		expect(
			(await object<any>(api, "cue_list", first.id)).body.cues.map(
				(cue: any) => cue.number,
			),
		).toEqual(["1"]);

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
		await api.executeCommandLine("RECORD PBK 1 CUE 8");
		expect(
			(await object<any>(api, "cue_list", first.id)).body.cues.map(
				(cue: any) => cue.number,
			),
		).toEqual(["1", "8"]);
		expect((await playbackState(api)).selected_playback).toEqual({ page: 1, position: 2 });
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
};

test(`${apiCUE006Contract.id} @api › ${apiCUE006Contract.title}`, async ({ api, bench, show, request }) => {
	const context = { api, bench, show, request };
	const state = await apiCUE006Contract.arrange(context, "api");
	await apiCUE006Contract.api(context, state);
	await apiCUE006Contract.assert(context, state, "api");
});

const apiCUE007Contract: Omit<PairedScenario<{ completed: boolean }>, "ui"> = {
	id: "CUE-007",
	title:
		"explicit zeroes block a later inserted on Cue from tracking past Cue 4",
	surfaces: ["api"],
	arrange: () => ({ completed: false }),
	api: async ({ api, bench }, state) => {
		await loadCanonicalCopy(api, bench, "cue-007-explicit-off", "compact-rig");
		await installCompactGroups(api);
		const fixtures = await fixtureIdsByNumber(api);
		for (const [id, numbers] of [
			["1", [1, 2, 3, 4]],
			["2", [5, 6, 7, 8]],
			["3", [9, 10, 11, 12]],
		] as const) {
			expect((await object<any>(api, "group", id)).body.fixtures).toEqual(
				numbers.map((number) => fixtures[number]),
			);
		}
		await setSequenceMasterFade(api, 0);
		const installed = await installPlaybackSequence(api, { page: 1, position: 1 }, [
			groupCue(1, [["1", "intensity", 1]]),
			groupCue(2, [["1", "intensity", 0]]),
			groupCue(3, [["2", "intensity", 1]]),
			groupCue(3.5, [["1", "intensity", 1]]),
			groupCue(4, [["1", "intensity", 0]]),
			groupCue(5, [["3", "intensity", 1]]),
		]);
		const stored = await object<any>(api, "cue_list", installed.id);
		for (const number of ["2", "4"]) {
			const cue = stored.body.cues.find((item: any) => item.number === number);
			expect(cue, `stored Cue ${number}`).toBeDefined();
			expect(groupValues(cue)).toEqual({ "1:intensity": 0 });
		}
		const expected = [
			[1, 0, 0],
			[0, 0, 0],
			[0, 1, 0],
			[1, 1, 0],
			[0, 1, 0],
			[0, 1, 1],
		];
		await assertCompactGroupSequence(bench, expected, () =>
			api.playbackAction({ page: 1, position: 1 }, "go", {}),
		);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
};

test(`${apiCUE007Contract.id} @api › ${apiCUE007Contract.title}`, async ({ api, bench, show, request }) => {
	const context = { api, bench, show, request };
	const state = await apiCUE007Contract.arrange(context, "api");
	await apiCUE007Contract.api(context, state);
	await apiCUE007Contract.assert(context, state, "api");
});
