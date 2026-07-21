import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import {
	clearProgrammerValues,
	releaseProgrammerFixtureValue,
} from "../apps/control-ui/e2e/bench/programmerValues";
import {
	command,
	expectProgrammer,
	expectSlotsAfterTick,
	fixtureIdsByNumber,
	fixtureRow,
	groupCard,
	INTENSITY,
	loadCompactRig,
	openFixtures,
	openGroups,
	pressCommandAndWait,
	programmer,
	select,
	setDimmerByTouch,
	slotsFromFrame,
} from "./support/foundational/helpers";
import {
	registerDimmerAndDerivedGroupPairedScenarios,
	registerFrozenAndEmptyGroupPairedScenarios,
	registerProgrammerPairedScenarios,
} from "./support/foundational/pairedScenarios";
import { supplementalAfter } from "./support/foundational/supplementalAfter";
import { supplementalBefore } from "./support/foundational/supplementalBefore";
import { supplementalSurfaceFactories } from "./support/foundational/supplementalSurfaces";

const FOUNDATIONAL_SCENARIOS =
	"docs/testing/01-foundational-dimmers-and-groups.md";

test.describe(FOUNDATIONAL_SCENARIOS, () => {
	registerDimmerAndDerivedGroupPairedScenarios();
	registerFrozenAndEmptyGroupPairedScenarios();
	registerProgrammerPairedScenarios();

	test("PROG-002 @ui › fixture ranges and retained selections spread through the desk command line", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		await loadCompactRig(api, bench, "prog-002-fixture-command-ui");
		await desk.open(api.baseUrl);

		await pressCommandAndWait(
			page,
			"1 THRU 5 AT 20 THRU 50",
			"F1 THRU 5 AT 20 THRU 50",
		);
		await expectSlotsAfterTick(
			bench,
			3_000,
			[51, 70, 89, 108, 128, 0, 0, 0, 0, 0, 0, 0],
		);

		await pressCommandAndWait(page, "1 THRU 5", "F1 THRU 5");
		await pressCommandAndWait(page, "AT 0 THRU 50", "AT 0 THRU 50");
		await expectSlotsAfterTick(
			bench,
			3_000,
			[0, 32, 64, 96, 128, 0, 0, 0, 0, 0, 0, 0],
		);
	});

	pairedScenario<{ overrideSlots: number[]; fixture: string; showId: string }>({
		id: "PROG-003",
		title:
			"newer fixture intensity wins LTP and releases back to its Group value",
		arrange: async ({ api, bench }, surface) => {
			const showId = await loadCompactRig(
				api,
				bench,
				`prog-003-paired-${surface}`,
			);
			return {
				overrideSlots: [],
				fixture: (await fixtureIdsByNumber(api))[1],
				showId,
			};
		},
		api: async ({ api, bench }, state) => {
			await command(api, "GROUP 1 AT 50");
			await command(api, "1 AT 25");
			state.overrideSlots = slotsFromFrame(await bench.tick(3_000), 12);
			await releaseProgrammerFixtureValue(api, {
				surface: "api",
				showId: state.showId,
				fixtureId: state.fixture,
				attribute: INTENSITY,
			});
		},
		ui: async ({ api, bench, desk, page }, state) => {
			await desk.open(api.baseUrl);
			await openGroups(page);
			await groupCard(page, 1).click();
			await setDimmerByTouch(page, 50);
			await openFixtures(page);
			await fixtureRow(page, 1).click();
			await setDimmerByTouch(page, 25);
			state.overrideSlots = slotsFromFrame(await bench.tick(3_000), 12);
			await page.getByRole("button", { name: "Release Dimmer" }).click();
		},
		assert: async ({ api, bench }, state) => {
			expect(state.overrideSlots).toEqual([
				64, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128,
			]);
			await expectProgrammer(api, (programmerState) => {
				expect(programmerState.group_values["1"]?.[INTENSITY]).toBeDefined();
				expect(
					programmerState.values.some(
						(value) =>
							value.fixture_id === state.fixture &&
							value.attribute === INTENSITY,
					),
				).toBe(false);
			});
			await expectSlotsAfterTick(bench, 0, Array(12).fill(128));
		},
	});

	pairedScenario<{
		showId: string;
		afterFirstClear: { selected: number; values: number; slots: number[] };
	}>({
		id: "PROG-004",
		title: "Clear removes selection first and programmer values second",
		arrange: async ({ api, bench }, surface) => {
			const showId = await loadCompactRig(
				api,
				bench,
				`prog-004-paired-${surface}`,
			);
			return {
				showId,
				afterFirstClear: { selected: -1, values: -1, slots: [] },
			};
		},
		api: async ({ api, bench }, state) => {
			await command(api, "1 + 2 AT 50");
			await bench.tick(3_000);
			await select(api, []);
			const first = await programmer(api);
			state.afterFirstClear = {
				selected: first.selected.length,
				values: first.values.length,
				slots: slotsFromFrame(await bench.tick(0), 12),
			};
			await clearProgrammerValues(api, {
				surface: "api",
				showId: state.showId,
			});
		},
		ui: async ({ api, bench, desk, page }, state) => {
			await desk.open(api.baseUrl);
			await pressCommandAndWait(page, "1 + 2 AT 50", "F1 + F2 AT 50");
			await bench.tick(3_000);
			const clear = page.getByRole("button", { name: "CLR", exact: true });
			await clear.click();
			await expect(clear).toHaveClass(/clear-warning/);
			await expectProgrammer(api, (programmerState) => {
				if (
					programmerState.selected.length !== 0 ||
					programmerState.values.length !== 2
				)
					throw new Error("first Clear stage has not settled");
				state.afterFirstClear = {
					selected: programmerState.selected.length,
					values: programmerState.values.length,
					slots: [],
				};
			});
			state.afterFirstClear.slots = slotsFromFrame(await bench.tick(0), 12);
			await clear.click();
			await expect(clear).toHaveClass(/clear-idle/);
		},
		assert: async ({ api, bench }, state) => {
			expect(state.afterFirstClear).toEqual({
				selected: 0,
				values: 2,
				slots: [128, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
			});
			await expectProgrammer(api, (programmerState) => {
				expect(programmerState.selected).toHaveLength(0);
				expect(programmerState.values).toHaveLength(0);
				expect(Object.keys(programmerState.group_values)).toHaveLength(0);
			});
			await expectSlotsAfterTick(bench, 0, Array(12).fill(0));
		},
	});
});

test.describe(FOUNDATIONAL_SCENARIOS, () => {
	for (const scenario of supplementalBefore) test(scenario.title, scenario.run);
	for (const surface of ["api", "ui"] as const) {
		for (const factory of supplementalSurfaceFactories) {
			const scenario = factory(surface);
			test(scenario.title, scenario.run);
		}
	}
	for (const scenario of supplementalAfter) test(scenario.title, scenario.run);
});
