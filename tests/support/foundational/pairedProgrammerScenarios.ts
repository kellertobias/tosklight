import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../../apps/control-ui/e2e/bench/pairedScenario";
import {
	command,
	enterCommandWithoutEscape,
	expectGroupNumbers,
	expectProgrammer,
	expectSelectedNumbers,
	expectSlotsAfterTick,
	INTENSITY,
	loadCompactRig,
	overwriteGroupByNumbers,
	pressCommandAndWait,
	recordExistingGroup,
	selectFixtureRows,
	slotsFromFrame,
} from "./helpers";

export function registerProgrammerPairedScenarios() {
	pairedScenario({
		id: "PROG-001",
		title:
			"values retain selection until replacement while leading Plus continues it",
		arrange: async ({ api, bench }, surface) => {
			await loadCompactRig(api, bench, `prog-001-paired-${surface}`);
			return {};
		},
		api: async ({ api }) => {
			for (const value of ["1 + 2 AT 50", "AT 25", "3 AT 75", "+ 4 AT 100"])
				await command(api, value);
		},
		ui: async ({ api, desk, page }) => {
			await desk.open(api.baseUrl);
			for (const [value, visible] of [
				["1 + 2 AT 50", "F1 + F2 AT 50"],
				["AT 25", "AT 25"],
				["3 AT 75", "F3 AT 75"],
				["+ 4 AT 100", "+F4 AT 100"],
			] as const)
				await enterCommandWithoutEscape(page, value, visible);
		},
		assert: async ({ api, bench }) => {
			await expectSelectedNumbers(api, [3, 4]);
			await expectProgrammer(api, (state) => {
				const intensity = state.values.filter(
					(value) => value.attribute === INTENSITY,
				);
				expect(intensity).toHaveLength(4);
			});
			await expectSlotsAfterTick(
				bench,
				3_000,
				[64, 64, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0],
			);
		},
	});

	pairedScenario<{ initialSlots: number[]; order: number[] }>({
		id: "PROG-002",
		title: "relative values spread across the live ordered Group",
		arrange: async ({ api, bench }, surface) => {
			await loadCompactRig(api, bench, `prog-002-paired-${surface}`);
			await overwriteGroupByNumbers(api, "1", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
			return {
				initialSlots: [],
				order: [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
			};
		},
		api: async ({ api, bench }, state) => {
			await command(api, "GROUP 1 AT 0 THRU 100");
			state.initialSlots = slotsFromFrame(await bench.tick(3_000), 12);
			await overwriteGroupByNumbers(api, "1", state.order);
		},
		ui: async ({ api, bench, desk, page }, state) => {
			await desk.open(api.baseUrl);
			await pressCommandAndWait(
				page,
				"GROUP 1 AT 0 THRU 100",
				"G1 AT 0 THRU 100",
			);
			state.initialSlots = slotsFromFrame(await bench.tick(3_000), 12);
			await selectFixtureRows(api, page, state.order);
			await recordExistingGroup(page, 1, "Overwrite");
		},
		assert: async ({ api, bench }, state) => {
			expect(state.initialSlots).toEqual([
				0, 28, 57, 85, 113, 142, 170, 198, 227, 255, 0, 0,
			]);
			await expectGroupNumbers(api, "1", state.order);
			await expectProgrammer(api, (programmerState) => {
				expect(
					programmerState.group_values["1"]?.[INTENSITY]?.value,
				).toMatchObject({
					kind: "spread",
					value: [0, 1],
				});
				expect(programmerState.values).toHaveLength(0);
			});
			await expectSlotsAfterTick(
				bench,
				0,
				[26, 51, 77, 102, 128, 153, 179, 204, 230, 255, 0, 0],
			);
		},
	});
}
