import { expect } from "../../apps/control-ui/e2e/bench/core/fixtures";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	putObject,
} from "../support/catalog";
import {
	type FixtureValue,
	fixtureCue,
	installPlaybackSequence,
	registerPairedCueScenario,
	rgbValues,
	runtime,
	setSequenceMasterFade,
	visualizationLevel,
} from "./support";

registerPairedCueScenario<{ completed: boolean }>({
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
				1,
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
				2,
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
		await api.playbackNumberAction(1, "on", {});
		await bench.tick(1);
		await api.playbackNumberAction(2, "on", {});
		await bench.tick(0);
		expect(await runtime(api, 1)).toMatchObject({ enabled: false });
		expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);

		let definition = await object<any>(api, "playback", "1");
		await putObject(
			api,
			"playback",
			"1",
			{ ...definition.body, auto_off: false },
			definition.revision,
		);
		await api.playbackNumberAction(2, "off", {});
		await api.playbackNumberAction(1, "on", {});
		await bench.tick(1);
		await api.playbackNumberAction(2, "on", {});
		await bench.tick(0);
		expect(await runtime(api, 1)).toMatchObject({ enabled: true });
		await api.playbackNumberAction(2, "off", {});
		await bench.tick(0);
		expect(await rgbValues(api, prepared.fixture)).toEqual([0, 0, 1]);

		definition = await object<any>(api, "playback", "1");
		await putObject(
			api,
			"playback",
			"1",
			{ ...definition.body, auto_off: true },
			definition.revision,
		);
		await api.playbackNumberAction(1, "on", {});
		await api.playbackNumberAction(2, "flash", { pressed: true });
		await bench.tick(0);
		expect(await runtime(api, 1)).toMatchObject({ enabled: true });
		expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);
		await api.playbackNumberAction(2, "flash", { pressed: false });
		await bench.tick(0);
		expect(await rgbValues(api, prepared.fixture)).toEqual([0, 0, 1]);
		await api.playbackNumberAction(2, "temp", {});
		await bench.tick(0);
		expect(await runtime(api, 1)).toMatchObject({ enabled: true });
		expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);
		await api.playbackNumberAction(2, "temp", {});
		await bench.tick(0);
		expect(await rgbValues(api, prepared.fixture)).toEqual([0, 0, 1]);

		prepared = await prepare("merge-003-partial", true);
		await api.playbackNumberAction(1, "on", {});
		await bench.tick(1);
		await api.playbackNumberAction(2, "on", {});
		await bench.tick(0);
		expect(await runtime(api, 1)).toMatchObject({ enabled: true });
		expect(await visualizationLevel(api, prepared.fixture, "intensity")).toBe(
			1,
		);
		expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
});
