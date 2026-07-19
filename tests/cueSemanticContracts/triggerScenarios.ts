import type { BenchUiContext } from "../../apps/control-ui/e2e/bench/fixtures";
import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	pressCommand,
} from "../support/catalog";
import {
	cueListIdForPlayback,
	fixtureCue,
	groupCue,
	installCompactGroups,
	installPlaybackSequence,
	registerPairedCueScenario,
	runtime,
	setSequenceMasterFade,
} from "./support";

const cue005Ui = async (
	{ api, bench, desk, page }: BenchUiContext,
	state: { completed: boolean },
) => {
	await loadCanonicalCopy(
		api,
		bench,
		"cue-005-visible-triggers",
		"compact-rig",
	);
	await installCompactGroups(api);
	await installPlaybackSequence(api, 1, [groupCue(10, [])]);
	await desk.open(bench.baseUrl);
	await pressCommand(page, "GROUP 1 AT 50", "G1 AT 50");
	await pressCommand(page, "RECORD SET 1 CUE 1", "RECORD SET 1 CUE 1");
	await pressCommand(
		page,
		"RECORD SET 1 CUE 2 TIME TIME 0",
		"RECORD SET 1 CUE 2 DELAY 0",
	);
	await pressCommand(
		page,
		"RECORD SET 1 CUE 3 TIME TIME 4",
		"RECORD SET 1 CUE 3 DELAY 4",
	);
	await expect
		.poll(
			async () =>
				(await object<any>(api, "cue_list", await cueListIdForPlayback(api, 1)))
					.body.cues.length,
		)
		.toBe(4);
	const stored = await object<any>(
		api,
		"cue_list",
		await cueListIdForPlayback(api, 1),
	);
	expect(stored.body.cues.find((cue: any) => cue.number === 1).trigger).toEqual(
		{ type: "manual" },
	);
	expect(stored.body.cues.find((cue: any) => cue.number === 2).trigger).toEqual(
		{ type: "follow", delay_millis: 0 },
	);
	expect(stored.body.cues.find((cue: any) => cue.number === 3).trigger).toEqual(
		{ type: "wait", delay_millis: 4_000 },
	);
	state.completed = true;
};

registerPairedCueScenario<{ completed: boolean }>({
	id: "CUE-005",
	title:
		"GO, FOLLOW, and TIME measure from the preceding Cue's latest value endpoint",
	arrange: () => ({ completed: false }),
	api: async ({ api, bench }, state) => {
		const setup = async (name: string, trigger: any, multiValue = false) => {
			await loadCanonicalCopy(api, bench, name, "compact-rig");
			await setSequenceMasterFade(api, 0);
			const fixtures = await fixtureIdsByNumber(api);
			await installPlaybackSequence(api, 1, [
				fixtureCue(
					1,
					multiValue
						? [
								[fixtures[1], "intensity", 0.5, { fade_millis: 1_000 }],
								[
									fixtures[2],
									"intensity",
									0.5,
									{ fade_millis: 3_000, delay_millis: 1_000 },
								],
							]
						: [[fixtures[1], "intensity", 0.5]],
					{ fade_millis: multiValue ? 0 : 2_000 },
				),
				fixtureCue(2, [[fixtures[1], "intensity", 0.8]], { trigger }),
				fixtureCue(3, [[fixtures[1], "intensity", 0.2]]),
			]);
		};

		await setup("cue-005-go", { type: "manual" });
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		await bench.tick(2_000);
		await bench.tick(604_800_000);
		expect((await runtime(api, 1)).current_cue_number).toBe(1);
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		expect((await runtime(api, 1)).current_cue_number).toBe(2);

		await setup("cue-005-follow", { type: "follow", delay_millis: 0 });
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		await bench.tick(1_999);
		expect((await runtime(api, 1)).current_cue_number).toBe(1);
		await bench.tick(1);
		expect((await runtime(api, 1)).current_cue_number).toBe(2);

		await setup("cue-005-time", { type: "wait", delay_millis: 4_000 });
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		await bench.tick(5_999);
		expect((await runtime(api, 1)).current_cue_number).toBe(1);
		await bench.tick(1);
		expect((await runtime(api, 1)).current_cue_number).toBe(2);
		await bench.tick(604_800_000);
		expect((await runtime(api, 1)).current_cue_number).toBe(2);

		await setup(
			"cue-005-latest-value",
			{ type: "follow", delay_millis: 0 },
			true,
		);
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		await bench.tick(3_999);
		expect((await runtime(api, 1)).current_cue_number).toBe(1);
		await bench.tick(1);
		expect((await runtime(api, 1)).current_cue_number).toBe(2);
		state.completed = true;
	},
	ui: cue005Ui,
	assert: async (_context, state) => expect(state.completed).toBe(true),
});
