import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import { recallPreset } from "../../../apps/control-ui/e2e/bench/presetRecall";
import {
	releaseProgrammerFixtureValue,
	releaseProgrammerGroupValue,
	setProgrammerFixtureValue,
	setProgrammerGroupValue,
} from "../../../apps/control-ui/e2e/bench/programmerValues";
import type { FoundationalCase } from "./case";
import {
	command,
	expectProgrammer,
	expectSelectedNumbers,
	expectSlotsAfterTick,
	fixtureIdsByNumber,
	INTENSITY,
	loadCompactRig,
	normalized,
	object,
	pressCommand,
	putObject,
	select,
} from "./helpers";

export const presetFamilyApi: FoundationalCase = {
	title:
		"PROG-001 @supplemental › Preset numbers are local to each family pool",
	run: async ({ api, bench }) => {
		const showId = await loadCompactRig(
			api,
			bench,
			"prog-001-family-local-preset-numbers",
		);
		const fixtures = await fixtureIdsByNumber(api);
		const fixture = fixtures[1];

		await putObject(api, "preset", "2.1", {
			name: "Color one",
			family: "Color",
			number: 1,
			values: {
				[fixture]: { "color.red": { kind: "normalized", value: 1 } },
			},
			group_values: {},
		});
		await putObject(api, "preset", "3.1", {
			name: "Position one",
			family: "Position",
			number: 1,
			values: { [fixture]: { pan: { kind: "normalized", value: 0.25 } } },
			group_values: {},
		});

		const colorOne = await object(api, "preset", "2.1");
		const positionOne = await object(api, "preset", "3.1");
		expect(colorOne.body).toMatchObject({ family: "Color", number: 1 });
		expect(positionOne.body).toMatchObject({ family: "Position", number: 1 });

		await select(api, [fixture]);
		await recallPreset(api, {
			surface: "api",
			showId,
			preset: { objectId: "2.1", family: "Color", number: 1 },
		});
		await recallPreset(api, {
			surface: "api",
			showId,
			preset: { objectId: "3.1", family: "Position", number: 1 },
		});
		await expectProgrammer(api, (programmer) => {
			expect(programmer.values).toEqual(
				expect.arrayContaining([
					expect.objectContaining({
						fixture_id: fixture,
						attribute: "color.red",
					}),
					expect.objectContaining({ fixture_id: fixture, attribute: "pan" }),
				]),
			);
		});
	},
};

export const ltpApi: FoundationalCase = {
	title:
		"PROG-003 @supplemental › API higher/lower LTP and scoped release permutations",
	run: async ({ api, bench }) => {
		const showId = await loadCompactRig(api, bench, "prog-003-api");
		const fixtures = await fixtureIdsByNumber(api);

		await command(api, "GROUP 1 AT 50");
		await command(api, "1 AT 75");
		await setProgrammerFixtureValue(api, {
			surface: "api",
			showId,
			fixtureId: fixtures[1],
			attribute: "pan",
			value: { kind: "normalized", value: 0.33 },
			timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
		});
		await expectSlotsAfterTick(
			bench,
			3_000,
			[191, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128],
		);

		await releaseProgrammerFixtureValue(api, {
			surface: "api",
			showId,
			fixtureId: fixtures[1],
			attribute: INTENSITY,
		});
		await expectProgrammer(api, (programmer) => {
			expect(
				programmer.values.some(
					(value) =>
						value.fixture_id === fixtures[1] && value.attribute === INTENSITY,
				),
			).toBe(false);
			expect(
				programmer.values.some(
					(value) =>
						value.fixture_id === fixtures[1] && value.attribute === "pan",
				),
			).toBe(true);
			expect(programmer.group_values["1"]?.[INTENSITY]).toBeDefined();
		});
		await expectSlotsAfterTick(bench, 0, Array(12).fill(128));

		const lowerShowId = await loadCompactRig(api, bench, "prog-003-lower-api");
		const lowerFixtures = await fixtureIdsByNumber(api);
		await command(api, "GROUP 1 AT 50");
		await command(api, "1 AT 25");
		await expectSlotsAfterTick(
			bench,
			3_000,
			[64, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128],
		);

		await command(api, "GROUP 1 AT 50");
		await expectSlotsAfterTick(
			bench,
			3_000,
			[128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128],
		);
		await setProgrammerGroupValue(api, {
			surface: "api",
			showId: lowerShowId,
			groupId: "1",
			attribute: "pan",
			value: { kind: "normalized", value: 0.4 },
			timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
		});
		await releaseProgrammerGroupValue(api, {
			surface: "api",
			showId: lowerShowId,
			groupId: "1",
			attribute: INTENSITY,
		});
		await expectProgrammer(api, (programmer) => {
			expect(programmer.group_values["1"]?.[INTENSITY]).toBeUndefined();
			expect(programmer.group_values["1"]?.pan).toBeDefined();
			expect(
				programmer.values.some(
					(value) =>
						value.fixture_id === lowerFixtures[1] &&
						value.attribute === INTENSITY,
				),
			).toBe(true);
		});
		await expectSlotsAfterTick(bench, 0, [64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
	},
};

export const clearUi: FoundationalCase = {
	title:
		"PROG-004 @supplemental › visible Clear styling, replacement, and continuation boundaries",
	run: async ({ api, bench, desk, page }) => {
		await loadCompactRig(api, bench, "prog-004-ui");
		await desk.open(api.baseUrl);
		const clear = page.getByRole("button", { name: "CLR", exact: true });
		await expect(clear).toHaveClass(/clear-idle/);

		await pressCommand(page, "GROUP 1", "G1");
		await expectProgrammer(api, (programmer) =>
			expect(programmer.selected).toHaveLength(12),
		);
		await expect(clear).toHaveClass(/clear-active/);
		await clear.click();
		await expectProgrammer(api, (programmer) => {
			expect(programmer.selected).toHaveLength(0);
			expect(programmer.values).toHaveLength(0);
			expect(Object.keys(programmer.group_values)).toHaveLength(0);
		});
		await expect(clear).toHaveClass(/clear-idle/);

		await pressCommand(page, "1 + 2 AT 75", "F1 + F2 AT 75");
		await expectSelectedNumbers(api, [1, 2]);
		await expect(clear).toHaveClass(/clear-active/);
		await pressCommand(page, "AT 50");
		await expectProgrammer(api, (programmer) => {
			expect(programmer.values.map((value) => normalized(value.value))).toEqual(
				[0.5, 0.5],
			);
		});
		await expectSlotsAfterTick(
			bench,
			3_000,
			[128, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
		);

		await clear.click();
		await expectProgrammer(api, (programmer) => {
			expect(programmer.selected).toHaveLength(0);
			expect(programmer.values).toHaveLength(2);
		});
		await expect(clear).toHaveClass(/clear-warning/);
		await expectSlotsAfterTick(
			bench,
			0,
			[128, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
		);

		await clear.click();
		await expectProgrammer(api, (programmer) => {
			expect(programmer.selected).toHaveLength(0);
			expect(programmer.values).toHaveLength(0);
			expect(Object.keys(programmer.group_values)).toHaveLength(0);
		});
		await expect(clear).toHaveClass(/clear-idle/);
		await expectSlotsAfterTick(bench, 0, Array(12).fill(0));

		await loadCompactRig(api, bench, "prog-004-ui-replacement");
		await desk.open(api.baseUrl);
		await pressCommand(page, "1 + 2 AT 75", "F1 + F2 AT 75");
		await pressCommand(page, "3 AT 80", "F3 AT 80");
		await expectSelectedNumbers(api, [3]);
		await expectSlotsAfterTick(
			bench,
			3_000,
			[191, 191, 204, 0, 0, 0, 0, 0, 0, 0, 0, 0],
		);

		await loadCompactRig(api, bench, "prog-004-ui-continuation");
		await desk.open(api.baseUrl);
		await pressCommand(page, "1 + 2 AT 75", "F1 + F2 AT 75");
		await pressCommand(page, "+ 3 AT 50", "+F3 AT 50");
		await expectSelectedNumbers(api, [1, 2, 3]);
		await expectSlotsAfterTick(
			bench,
			3_000,
			[128, 128, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0],
		);
	},
};
