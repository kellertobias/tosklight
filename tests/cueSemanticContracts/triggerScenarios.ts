import type { PairedScenario } from "../bench/core/pairedScenario";
import { expect, test } from "../bench/core/fixtures";
import { fixtureIdsByNumber, loadCanonicalCopy } from "../support/catalog";
import {
	fixtureCue,
	installPlaybackSequence,
	runtime,
	setSequenceMasterFade,
} from "./support";

const apiCUE005Contract: Omit<PairedScenario<{ completed: boolean }>, "ui"> = {
	id: "CUE-005",
	title:
		"GO waits for the operator, FOLLOW waits for completion, and TIME counts from Cue start",
	surfaces: ["api"],
	arrange: () => ({ completed: false }),
	api: async ({ api, bench }, state) => {
		const setup = async (name: string, trigger: any, multiValue = false) => {
			await loadCanonicalCopy(api, bench, name, "compact-rig");
			await setSequenceMasterFade(api, 0);
			const fixtures = await fixtureIdsByNumber(api);
			await installPlaybackSequence(api, { page: 1, position: 1 }, [
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
		await api.playbackAction({ page: 1, position: 1 }, "go", {});
		await bench.tick(2_000);
		await bench.tick(604_800_000);
		expect((await runtime(api, { page: 1, position: 1 })).current_cue_number).toBe("1");
		await api.playbackAction({ page: 1, position: 1 }, "go", {});
		expect((await runtime(api, { page: 1, position: 1 })).current_cue_number).toBe("2");

		await setup("cue-005-follow", { type: "follow", delay_millis: 0 });
		await api.playbackAction({ page: 1, position: 1 }, "go", {});
		await bench.tick(1_999);
		expect((await runtime(api, { page: 1, position: 1 })).current_cue_number).toBe("1");
		await bench.tick(1);
		expect((await runtime(api, { page: 1, position: 1 })).current_cue_number).toBe("2");

		await setup("cue-005-time", { type: "wait", delay_millis: 4_000 });
		await api.playbackAction({ page: 1, position: 1 }, "go", {});
		await bench.tick(3_999);
		expect((await runtime(api, { page: 1, position: 1 })).current_cue_number).toBe("1");
		await bench.tick(1);
		expect((await runtime(api, { page: 1, position: 1 })).current_cue_number).toBe("2");
		await bench.tick(604_800_000);
		expect((await runtime(api, { page: 1, position: 1 })).current_cue_number).toBe("2");

		await setup(
			"cue-005-latest-value",
			{ type: "follow", delay_millis: 0 },
			true,
		);
		await api.playbackAction({ page: 1, position: 1 }, "go", {});
		await bench.tick(3_999);
		expect((await runtime(api, { page: 1, position: 1 })).current_cue_number).toBe("1");
		await bench.tick(1);
		expect((await runtime(api, { page: 1, position: 1 })).current_cue_number).toBe("2");
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
};

test(`${apiCUE005Contract.id} @api › ${apiCUE005Contract.title}`, async ({ api, bench, show, request }) => {
	const context = { api, bench, show, request };
	const state = await apiCUE005Contract.arrange(context, "api");
	await apiCUE005Contract.api(context, state);
	await apiCUE005Contract.assert(context, state, "api");
});
