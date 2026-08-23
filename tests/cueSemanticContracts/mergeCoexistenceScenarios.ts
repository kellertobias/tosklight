import { ApiDriver } from "../bench/core/api";
import { expect, test } from "../bench/core/fixtures";
import { setProgrammerPriority } from "../bench/programmer/programmerPriority";
import {
	clearProgrammerValues,
	releaseProgrammerFixtureValue,
	setProgrammerFixtureValue,
} from "../bench/programmer/programmerValues";
import { fixtureIdsByNumber, loadCanonicalCopy } from "../support/catalog";
import {
	CUE_SEMANTIC_CONTRACTS,
	fixtureCue,
	installPlaybackSequence,
	registerPairedCueScenario,
	rgbValues,
	setSequenceMasterFade,
	slot,
	visualizationAfterTick,
	visualizationLevel,
} from "./support";

const PROGRAMMER_TIMING = {
	fade: true,
	fadeMillis: 0,
	delayMillis: null,
} as const;

test.describe(CUE_SEMANTIC_CONTRACTS, () => {
	test("MERGE-001 @api › every surface writes one Programmer and the most recent value wins", async ({
		api,
		bench,
	}) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			"merge-001-one-programmer",
			"compact-rig",
		);
		await setSequenceMasterFade(api, 0);
		const fixtures = await fixtureIdsByNumber(api);
		// Two connections to the same desk. They are surfaces of one Programmer, not two
		// Programmers bidding against each other.
		const mainWindow = new ApiDriver(api.baseUrl);
		const wing = new ApiDriver(api.baseUrl);
		await mainWindow.login("Operator");
		await wing.login("Operator");

		await setFixtureValue(mainWindow, show.id, fixtures[1], "intensity", 0.4);
		await bench.tick(1);
		await setFixtureValue(wing, show.id, fixtures[1], "intensity", 0.7);
		expect(slot(await bench.tick(0), 1)).toBe(179);

		// LTP, not HTP: a lower value typed later still wins, because there is no second
		// Programmer for it to be measured against.
		await setFixtureValue(wing, show.id, fixtures[1], "intensity", 0.2);
		expect(slot(await bench.tick(0), 1)).toBe(51);
		await setFixtureValue(mainWindow, show.id, fixtures[1], "intensity", 0.9);
		expect(slot(await bench.tick(0), 1)).toBe(230);

		// Priority is the desk's, so setting it from either surface sets the same one and does
		// not divide the Programmer in two.
		await setProgrammerPriority(mainWindow, { surface: "api", priority: 20 });
		await setFixtureValue(wing, show.id, fixtures[1], "intensity", 0.5);
		expect(slot(await bench.tick(0), 1)).toBe(128);

		const rgb = fixtures[21];
		await setFixtureValue(mainWindow, show.id, rgb, "red", 0.4);
		await bench.tick(1);
		await setFixtureValue(wing, show.id, rgb, "red", 0.8);
		expect(await visualizationAfterTick(api, bench, rgb, "red", 0)).toBe(0.8);

		// One Programmer holds the value, so there is one row to find it in.
		const diagnostics = await api.request<any>("GET", "/api/v2/diagnostics");
		expect(
			diagnostics.active_programmers.filter((programmer: any) =>
				programmer.values.some(
					(value: any) => value.fixture_id === rgb && value.attribute === "red",
				),
			),
		).toHaveLength(1);

		// Releasing it from either surface releases the desk's value outright; there is no
		// second copy underneath to be revealed.
		await releaseProgrammerFixtureValue(wing, {
			surface: "api",
			showId: show.id,
			fixtureId: rgb,
			attribute: "red",
		});
		expect(await visualizationAfterTick(api, bench, rgb, "red", 0)).toBe(0);
	});
});

registerPairedCueScenario<{ completed: boolean; showId: string }>({
	id: "MERGE-002",
	title:
		"independent Sequences coexist and retrigger only their stored addresses",
	surfaces: ["api"],
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			surface === "api" ? "merge-002-independent" : "merge-002-independent-ui",
			"compact-rig",
		);
		return { completed: false, showId: show.id };
	},
	api: async ({ api, bench }, state) => {
		await setSequenceMasterFade(api, 0);
		const fixtures = await fixtureIdsByNumber(api);
		const aFixture = fixtures[21];
		const bFixture = fixtures[22];
		await installPlaybackSequence(
			api,
			1,
			[
				fixtureCue(1, [
					[aFixture, "intensity", 0.6],
					[aFixture, "red", 0],
					[aFixture, "green", 0],
					[aFixture, "blue", 1],
				]),
			],
			{ name: "Sequence A", priority: 100 },
		);
		await installPlaybackSequence(
			api,
			2,
			[
				fixtureCue(1, [
					[bFixture, "intensity", 0.4],
					[bFixture, "red", 1],
					[bFixture, "green", 0.7],
					[bFixture, "blue", 0.4],
				]),
			],
			{ name: "Sequence B", priority: 100 },
		);
		await api.playbackNumberAction(1, "go", {});
		await bench.tick(1);
		await api.playbackNumberAction(2, "go", {});
		await bench.tick(0);
		expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0.6);
		expect(await visualizationLevel(api, bFixture, "intensity")).toBe(0.4);

		await setFixtureValue(api, state.showId, aFixture, "intensity", 0.3);
		for (const [attribute, value] of [
			["red", 1],
			["green", 0],
			["blue", 0],
		] as const)
			await setFixtureValue(api, state.showId, aFixture, attribute, value);
		await bench.tick(0);
		expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0.6);
		expect(await rgbValues(api, aFixture)).toEqual([1, 0, 0]);
		expect(await rgbValues(api, bFixture)).toEqual([1, 0.7, 0.4]);

		await bench.tick(1);
		await api.playbackNumberAction(1, "go-to", { cue_number: 1 });
		await bench.tick(0);
		expect(await rgbValues(api, aFixture)).toEqual([0, 0, 1]);
		expect(await rgbValues(api, bFixture)).toEqual([1, 0.7, 0.4]);

		await setFixtureValue(api, state.showId, bFixture, "blue", 0.8);
		await bench.tick(1);
		await api.playbackNumberAction(1, "go-to", { cue_number: 1 });
		await bench.tick(0);
		expect(await visualizationLevel(api, bFixture, "blue")).toBe(0.8);

		await setProgrammerPriority(api, { surface: "api", priority: 110 });
		await setFixtureValue(api, state.showId, aFixture, "red", 1);
		await bench.tick(1);
		await api.playbackNumberAction(1, "go-to", { cue_number: 1 });
		await bench.tick(0);
		expect(await visualizationLevel(api, aFixture, "red")).toBe(1);
		await setProgrammerPriority(api, { surface: "api", priority: 90 });
		await api.playbackNumberAction(1, "go-to", { cue_number: 1 });
		await bench.tick(0);
		expect(await visualizationLevel(api, aFixture, "red")).toBe(0);

		await clearProgrammerValues(api, {
			surface: "api",
			showId: state.showId,
		});
		await api.playbackNumberAction(2, "off", {});
		await bench.tick(0);
		expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0.6);
		expect(await visualizationLevel(api, bFixture, "intensity")).toBe(0);
		await api.playbackNumberAction(1, "off", {});
		await bench.tick(0);
		expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
});

function setFixtureValue(
	api: ApiDriver,
	showId: string,
	fixtureId: string,
	attribute: string,
	value: number,
) {
	return setProgrammerFixtureValue(api, {
		surface: "api",
		showId,
		fixtureId,
		attribute,
		value: { kind: "normalized", value },
		timing: PROGRAMMER_TIMING,
	});
}
