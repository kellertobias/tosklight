import type { PairedScenario } from "../bench/core/pairedScenario";
import { expect, test } from "../bench/core/fixtures";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
} from "../support/catalog";
import {
	assignPlayback,
	type FixtureValue,
	fixtureCue,
	installPlaybackSequence,
	rgbValues,
	runtime,
	setSequenceMasterFade,
	visualizationLevel,
} from "./support";

const apiMERGE003Contract: Omit<PairedScenario<{ completed: boolean }>, "ui"> = {
	id: "MERGE-003",
	title:
		"full normal overwrite auto-Offs while partial, disabled, Flash, and Temp restore",
	surfaces: ["api"],
	arrange: () => ({ completed: false }),
	api: async ({ api, bench }, state) => {
		const prepare = async (name: string, includeIntensity: boolean) => {
			await loadCanonicalCopy(api, bench, name, "compact-rig");
			await setSequenceMasterFade(api, 0);
			const fixture = (await fixtureIdsByNumber(api))[21];
			const underlying = await installPlaybackSequence(
				api,
				{ page: 1, position: 1 },
				[
					fixtureCue(1, [
						...(includeIntensity
							? [[fixture, "intensity", 1] as FixtureValue]
							: []),
						[fixture, "red", 0],
						[fixture, "green", 0],
						[fixture, "blue", 1],
					]),
				],
				{ name: "Underlying blue", auto_off: true },
			);
			await installPlaybackSequence(
				api,
				{ page: 1, position: 2 },
				[
					fixtureCue(1, [
						[fixture, "red", 1],
						[fixture, "green", 0],
						[fixture, "blue", 0],
					]),
				],
				{ name: "Replacing red", auto_off: false },
			);
			return { fixture, underlying };
		};

		let prepared = await prepare("merge-003-full", false);
		await api.playbackAction({ page: 1, position: 1 }, "on", {});
		await bench.tick(1);
		await api.playbackAction({ page: 1, position: 2 }, "on", {});
		await bench.tick(0);
		expect(await runtime(api, { page: 1, position: 1 })).toMatchObject({ enabled: false });
		expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);

		let definition = await object<any>(api, "playback_page", "1");
		await assignPlayback(api, { page: 1, position: 1 }, { ...definition.body.assignments[1], auto_off: false });
		await api.playbackAction({ page: 1, position: 2 }, "off", {});
		await api.playbackAction({ page: 1, position: 1 }, "on", {});
		await bench.tick(1);
		await api.playbackAction({ page: 1, position: 2 }, "on", {});
		await bench.tick(0);
		expect(await runtime(api, { page: 1, position: 1 })).toMatchObject({ enabled: true });
		await api.playbackAction({ page: 1, position: 2 }, "off", {});
		await bench.tick(0);
		expect(await rgbValues(api, prepared.fixture)).toEqual([0, 0, 1]);

		definition = await object<any>(api, "playback_page", "1");
		await assignPlayback(api, { page: 1, position: 1 }, { ...definition.body.assignments[1], auto_off: true });
		await api.playbackAction({ page: 1, position: 1 }, "on", {});
		await api.playbackAction({ page: 1, position: 2 }, "flash", { pressed: true });
		await bench.tick(0);
		expect(await runtime(api, { page: 1, position: 1 })).toMatchObject({ enabled: true });
		expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);
		await api.playbackAction({ page: 1, position: 2 }, "flash", { pressed: false });
		await bench.tick(0);
		expect(await rgbValues(api, prepared.fixture)).toEqual([0, 0, 1]);
		await api.playbackAction({ page: 1, position: 2 }, "temp", {});
		await bench.tick(0);
		expect(await runtime(api, { page: 1, position: 1 })).toMatchObject({ enabled: true });
		expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);
		await api.playbackAction({ page: 1, position: 2 }, "temp", {});
		await bench.tick(0);
		expect(await rgbValues(api, prepared.fixture)).toEqual([0, 0, 1]);

		prepared = await prepare("merge-003-partial", true);
		await api.playbackAction({ page: 1, position: 1 }, "on", {});
		await bench.tick(1);
		await api.playbackAction({ page: 1, position: 2 }, "on", {});
		await bench.tick(0);
		expect(await runtime(api, { page: 1, position: 1 })).toMatchObject({ enabled: true });
		expect(await visualizationLevel(api, prepared.fixture, "intensity")).toBe(
			1,
		);
		expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
};

test(`${apiMERGE003Contract.id} @api › ${apiMERGE003Contract.title}`, async ({ api, bench, show, request }) => {
	const context = { api, bench, show, request };
	const state = await apiMERGE003Contract.arrange(context, "api");
	await apiMERGE003Contract.api(context, state);
	await apiMERGE003Contract.assert(context, state, "api");
});
