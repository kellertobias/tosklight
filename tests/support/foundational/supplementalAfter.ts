import type { ApiDriver } from "../../../apps/control-ui/e2e/bench/api";
import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import { goCueListPlayback } from "../../../apps/control-ui/e2e/bench/playbackRuntimeAction";
import { recallPreset } from "../../../apps/control-ui/e2e/bench/presetRecall";
import { clearProgrammerValues } from "../../../apps/control-ui/e2e/bench/programmerValues";
import type { FoundationalCase } from "./case";
import {
	command,
	expectProgrammer,
	expectSlotsAfterTick,
	fixtureIdsByNumber,
	fixtureRow,
	gestureFixture,
	groupCard,
	INTENSITY,
	loadCompactRig,
	normalized,
	object,
	openFixtures,
	openGroups,
	overwriteGroupByNumbers,
	pressCommand,
	programmer,
	putObject,
	select,
	setDimmerByTouch,
} from "./helpers";

interface BenchDriver {
	tick(millis: number): Promise<{
		universes: Array<{ universe: number; slots: number[] }>;
	}>;
}

async function prepareSpreadRig(
	api: ApiDriver,
	bench: BenchDriver,
	name: string,
) {
	const showId = await loadCompactRig(api, bench, name);
	await overwriteGroupByNumbers(api, "1", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
	return showId;
}

async function createCuePlayback(api: ApiDriver, number: number) {
	const cueListId = crypto.randomUUID();
	await putObject(api, "cue_list", cueListId, {
		id: cueListId,
		name: `Spread ${number}`,
		priority: 0,
		mode: "sequence",
		looped: false,
		chaser_step_millis: 1000,
		speed_group: null,
		intensity_priority_mode: "htp",
		wrap_mode: "off",
		restart_mode: "first_cue",
		force_cue_timing: false,
		disable_cue_timing: false,
		chaser_xfade_millis: 0,
		speed_multiplier: 1,
		cues: [
			{
				id: crypto.randomUUID(),
				number: 1,
				name: "",
				changes: [],
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				phasers: [],
				group_changes: [],
			},
		],
	});
	await putObject(api, "playback", String(number), {
		number,
		name: `Spread ${number}`,
		target: { type: "cue_list", cue_list_id: cueListId },
	});
	return cueListId;
}

async function verifyBasicSpreadPermutations(
	api: ApiDriver,
	bench: BenchDriver,
) {
	await prepareSpreadRig(api, bench, "prog-002-uniform-api");
	await command(api, "GROUP 1 AT 0");
	await expectProgrammer(api, (state) => {
		expect(normalized(state.group_values["1"]?.[INTENSITY]?.value)).toBe(0);
		expect(state.values).toHaveLength(0);
	});
	await expectSlotsAfterTick(bench, 3_000, Array(12).fill(0));
	await prepareSpreadRig(api, bench, "prog-002-descending-api");
	await command(api, "GROUP 1 AT 100 THRU 0");
	await expectProgrammer(api, (state) => {
		expect(state.group_values["1"]?.[INTENSITY]?.value).toMatchObject({
			kind: "spread",
			value: [1, 0],
		});
		expect(state.values).toHaveLength(0);
	});
	await expectSlotsAfterTick(
		bench,
		3_000,
		[255, 227, 198, 170, 142, 113, 85, 57, 28, 0, 0, 0],
	);
	await prepareSpreadRig(api, bench, "prog-002-multi-point-api");
	await command(api, "GROUP 1 AT 100 THRU 0 THRU 100");
	const multi = await bench.tick(3_000);
	const multiSlots = multi.universes
		.find((universe) => universe.universe === 1)!
		.slots.slice(0, 10);
	expect(multiSlots).toEqual([...multiSlots].reverse());
	expect(multiSlots[0]).toBe(255);
	expect(multiSlots[9]).toBe(255);
	expect(multiSlots[4]).toBe(multiSlots[5]);
	expect(multiSlots.slice(0, 5)).toEqual(
		[...multiSlots.slice(0, 5)].sort(
			(left: number, right: number) => right - left,
		),
	);
	expect(multiSlots.slice(5)).toEqual(
		[...multiSlots.slice(5)].sort(
			(left: number, right: number) => left - right,
		),
	);
}

async function verifyLiveSpreadStorage(api: ApiDriver, bench: BenchDriver) {
	const showId = await prepareSpreadRig(
		api,
		bench,
		"prog-002-live-storage-api",
	);
	const cueListId = await createCuePlayback(api, 1);
	await command(api, "GROUP 1 AT 0 THRU 100");
	await command(api, "RECORD 1.1");
	await command(api, "RECORD SET 1 CUE 1");
	const preset = await object(api, "preset", "1.1");
	expect(preset.body.group_values["1"]?.[INTENSITY]).toMatchObject({
		kind: "spread",
		value: [0, 1],
	});
	expect(Object.keys(preset.body.values)).toHaveLength(0);
	const cueList = await object(api, "cue_list", cueListId);
	expect(cueList.body.cues[0].group_changes).toEqual(
		expect.arrayContaining([
			expect.objectContaining({
				group_id: "1",
				attribute: INTENSITY,
				value: expect.objectContaining({ kind: "spread", value: [0, 1] }),
			}),
		]),
	);
	expect(cueList.body.cues[0].changes).toHaveLength(0);
	await overwriteGroupByNumbers(api, "1", [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
	const expected = [26, 51, 77, 102, 128, 153, 179, 204, 230, 255, 0, 0];
	await expectSlotsAfterTick(bench, 3_000, expected);
	await clearProgrammerValues(api, { surface: "api", showId });
	await command(api, "GROUP 1 AT 1.1");
	await expectProgrammer(api, (state) => {
		expect(state.group_values["1"]?.[INTENSITY]?.value).toMatchObject({
			kind: "spread",
			value: [0, 1],
		});
		expect(state.values).toHaveLength(0);
	});
	await expectSlotsAfterTick(bench, 3_000, expected);
	await clearProgrammerValues(api, { surface: "api", showId });
	await goCueListPlayback(api, {
		surface: "api",
		showId,
		playbackNumber: 1,
		cueListId,
	});
	await expectSlotsAfterTick(bench, 3_000, expected);
}

async function verifyDereferencedSpreadStorage(
	api: ApiDriver,
	bench: BenchDriver,
) {
	const showId = await prepareSpreadRig(
		api,
		bench,
		"prog-002-dereferenced-storage-api",
	);
	const cueListId = await createCuePlayback(api, 2);
	await command(api, "DEGRP 1 AT 0 THRU 100");
	await expectProgrammer(api, (state) => {
		expect(Object.keys(state.group_values)).toHaveLength(0);
		expect(state.values).toHaveLength(10);
	});
	await command(api, "RECORD 1.2");
	await command(api, "RECORD SET 2 CUE 1");
	const preset = await object(api, "preset", "1.2");
	expect(Object.keys(preset.body.group_values)).toHaveLength(0);
	expect(Object.keys(preset.body.values)).toHaveLength(10);
	const cueList = await object(api, "cue_list", cueListId);
	expect(cueList.body.cues[0].group_changes).toHaveLength(0);
	expect(cueList.body.cues[0].changes).toHaveLength(10);
	await overwriteGroupByNumbers(api, "1", [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
	const expected = [0, 28, 57, 85, 113, 142, 170, 198, 227, 255, 0, 0];
	await expectSlotsAfterTick(bench, 3_000, expected);
	await clearProgrammerValues(api, { surface: "api", showId });
	await command(api, "DEGRP 1 AT 1.2");
	await expectProgrammer(api, (state) => {
		expect(Object.keys(state.group_values)).toHaveLength(0);
		expect(state.values).toHaveLength(10);
	});
	await expectSlotsAfterTick(bench, 3_000, expected);
	await clearProgrammerValues(api, { surface: "api", showId });
	await goCueListPlayback(api, {
		surface: "api",
		showId,
		playbackNumber: 2,
		cueListId,
	});
	await expectSlotsAfterTick(bench, 3_000, expected);
}

export const supplementalAfter: FoundationalCase[] = [
	{
		title:
			"PROG-001 @supplemental › API Preset recall preserves and closes gesture boundaries",
		run: async ({ api, bench }) => {
			const showId = await loadCompactRig(api, bench, "prog-001-preset-api");
			const fixtures = await fixtureIdsByNumber(api);
			await gestureFixture(api, fixtures[21]);
			await gestureFixture(api, fixtures[22]);
			await putObject(api, "preset", "1.200", {
				name: "LED intensity",
				family: "Intensity",
				number: 200,
				values: {
					[fixtures[21]]: { intensity: { kind: "normalized", value: 0.4 } },
					[fixtures[22]]: { intensity: { kind: "normalized", value: 0.4 } },
				},
				group_values: {},
			});
			const before = await programmer(api);
			expect(before.selected).toEqual([fixtures[21], fixtures[22]]);
			expect(before.selection_expression).toMatchObject({
				type: "sources",
				items: [
					{ type: "fixture", fixture_id: fixtures[21] },
					{ type: "fixture", fixture_id: fixtures[22] },
				],
			});

			await recallPreset(api, {
				surface: "api",
				showId,
				preset: { objectId: "1.200", family: "Intensity", number: 200 },
			});
			const recalled = await programmer(api);
			expect(recalled.selected).toEqual(before.selected);
			expect(recalled.selection_expression).toEqual(
				before.selection_expression,
			);
			expect(
				recalled.values.filter((value) => value.attribute === INTENSITY),
			).toHaveLength(2);

			await gestureFixture(api, fixtures[23]);
			const replacement = await programmer(api);
			expect(replacement.selected).toEqual([fixtures[23]]);
			expect(
				replacement.values.filter((value) => value.attribute === INTENSITY),
			).toHaveLength(2);
		},
	},
	{
		title:
			"PROG-002 @supplemental › uniform, descending, multi-point, storage, and recall permutations",
		run: async ({ api, bench }) => {
			await verifyBasicSpreadPermutations(api, bench);
			await verifyLiveSpreadStorage(api, bench);
			await verifyDereferencedSpreadStorage(api, bench);
		},
	},
	{
		title:
			"PROG-003 @supplemental › visible higher/lower LTP and scoped release permutations",
		run: async ({ api, bench, desk, page }) => {
			await loadCompactRig(api, bench, "prog-003-ui");
			await desk.open(api.baseUrl);
			await openGroups(page);
			await groupCard(page, 1).click();
			await setDimmerByTouch(page, 50);
			await openFixtures(page);
			await fixtureRow(page, 1).click();
			await setDimmerByTouch(page, 75);
			await expectSlotsAfterTick(
				bench,
				3_000,
				[191, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128],
			);
			await page.getByRole("button", { name: "Release Dimmer" }).click();
			await expectSlotsAfterTick(bench, 0, Array(12).fill(128));

			await loadCompactRig(api, bench, "prog-003-ui-lower");
			await desk.open(api.baseUrl);
			await openGroups(page);
			await groupCard(page, 1).click();
			await setDimmerByTouch(page, 50);
			await openFixtures(page);
			await fixtureRow(page, 1).click();
			await setDimmerByTouch(page, 25);
			await expectSlotsAfterTick(
				bench,
				3_000,
				[64, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128],
			);
			await pressCommand(page, "GROUP 1 AT 50", "G1 AT 50");
			await expectSlotsAfterTick(bench, 3_000, Array(12).fill(128));
			await page.getByRole("button", { name: "Release Dimmer" }).click();
			await expectSlotsAfterTick(
				bench,
				0,
				[64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
			);
			await openFixtures(page);
			await fixtureRow(page, 1).click();
			await page.getByRole("button", { name: "Release Dimmer" }).click();
			await expectSlotsAfterTick(bench, 0, Array(12).fill(0));
		},
	},
	{
		title: "PROG-004 @supplemental › direct API clear-stage boundary",
		run: async ({ api, bench }) => {
			const showId = await loadCompactRig(api, bench, "prog-004-api");
			await command(api, "1 + 2 AT 50");
			await select(api, []);
			await expectProgrammer(api, (state) => {
				expect(state.selected).toHaveLength(0);
				expect(state.values).toHaveLength(2);
			});
			await expectSlotsAfterTick(
				bench,
				3_000,
				[128, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
			);
			await clearProgrammerValues(api, { surface: "api", showId });
			await expectProgrammer(api, (state) =>
				expect(state.values).toHaveLength(0),
			);
			await expectSlotsAfterTick(bench, 0, Array(12).fill(0));
		},
	},
];
