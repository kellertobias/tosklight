import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../../apps/control-ui/e2e/bench/pairedScenario";
import { storeGroup } from "../operator";
import {
	command,
	expectGroup,
	expectGroupNumbers,
	expectProgrammer,
	expectSelectedNumbers,
	expectSlotsAfterTick,
	fixtureIdsByNumber,
	INTENSITY,
	loadCompactRig,
	normalized,
	overwriteGroupByNumbers,
	pressCommandAndWait,
	recordExistingGroup,
	selectFixtureRows,
} from "./helpers";

export function registerDimmerAndDerivedGroupPairedScenarios() {
	pairedScenario({
		id: "DIM-001",
		title:
			"ordered Group edits retain their live value and append re-added fixtures",
		arrange: async ({ api, bench }, surface) => {
			await loadCompactRig(api, bench, `dim-001-paired-${surface}`);
			return {};
		},
		api: async ({ api }) => {
			for (const value of [
				"GROUP 3 AT 50",
				"GROUP 3 + 5 + 6",
				"RECORD GROUP 3",
				"GROUP 3 - 2 + 2",
				"RECORD GROUP 3",
			])
				await command(api, value);
		},
		ui: async ({ api, desk, page }) => {
			await desk.open(api.baseUrl);
			for (const [value, visible] of [
				["GROUP 3 AT 50", "G3 AT 50"],
				["GROUP 3 + 5 + 6", "G3 + F5 + F6"],
				["RECORD GROUP 3", "RECORD GROUP 3"],
				["GROUP 3 - 2 + 2", "G3 - F2 + F2"],
				["RECORD GROUP 3", "RECORD GROUP 3"],
			] as const)
				await pressCommandAndWait(page, value, visible);
		},
		assert: async ({ api, bench }) => {
			await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6, 2]);
			await expectProgrammer(api, (state) => {
				expect(state.group_values["3"]?.[INTENSITY]).toBeDefined();
				expect(state.values).toHaveLength(0);
			});
			await expectSlotsAfterTick(bench, 3_000, [128, 128, 128, 128, 128, 128]);
		},
	});

	pairedScenario({
		id: "DIM-002",
		title: "Lightning Desk command reaches the exact rendered output boundary",
		arrange: async ({ api, bench }, surface) => {
			await loadCompactRig(api, bench, `dim-002-paired-${surface}`);
			return {};
		},
		api: async ({ api }) => command(api, "GROUP 1 AT 50"),
		ui: async ({ api, desk, page }) => {
			await desk.open(api.baseUrl);
			await pressCommandAndWait(page, "GROUP 1 AT 50", "G1 AT 50");
		},
		assert: async ({ api, bench }) => {
			await expectProgrammer(api, (state) =>
				expect(normalized(state.group_values["1"][INTENSITY].value)).toBe(0.5),
			);
			await expectSlotsAfterTick(bench, 2_999, Array(12).fill(127));
			await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
			await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
		},
	});

	pairedScenario({
		id: "CMD-001",
		title:
			"Fixture and Group default modes toggle while explicit prefixes stay scoped",
		arrange: async ({ api, bench }, surface) => {
			await loadCompactRig(api, bench, `cmd-001-paired-${surface}`);
			return {};
		},
		api: async ({ api }) => {
			await api.setCompatibilityCommandTarget("GROUP");
			await api.setCompatibilityCommandTarget("FIXTURE");
			await command(api, "G1 + F2");
		},
		ui: async ({ api, desk, page }) => {
			await desk.open(api.baseUrl);
			const commandLine = page.getByLabel("Command line");
			await page.getByRole("button", { name: "GRP", exact: true }).click();
			await expect(commandLine).toHaveValue("GROUP");
			await page.getByRole("button", { name: "ENT", exact: true }).click();
			await expect(commandLine).toHaveValue("GROUP");
			await page.getByRole("button", { name: "GRP", exact: true }).click();
			await expect(commandLine).toHaveValue("FIXTURE");
			await page.getByRole("button", { name: "ENT", exact: true }).click();
			await expect(commandLine).toHaveValue("FIXTURE");
			await pressCommandAndWait(page, "GROUP 1 + 2", "G1 + F2");
		},
		assert: async ({ api }) => {
			const fixtures = await fixtureIdsByNumber(api);
			await expectSelectedNumbers(api, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
			await expectProgrammer(api, (state) => {
				expect(state.selection_expression).toMatchObject({
					type: "sources",
					items: [
						{ type: "live_group", group_id: "1" },
						{ type: "fixture", fixture_id: fixtures[2] },
					],
				});
			});
		},
	});

	pairedScenario({
		id: "GROUP-003",
		title: "derived Group follows source ordering",
		arrange: async ({ api, bench }, surface) => {
			await loadCompactRig(api, bench, `group-003-paired-${surface}`);
			return { sourceOrder: [12, 1, 2, 8, 4, 5, 6, 7, 9, 10, 11] };
		},
		api: async ({ api }, state) => {
			await command(api, "GROUP 1 DIV 2");
			await storeGroup({
				via: "programmer",
				surface: { via: "command-line", api },
				group: 5,
			});
			await overwriteGroupByNumbers(api, "1", state.sourceOrder);
		},
		ui: async ({ api, desk, page }, state) => {
			await desk.open(api.baseUrl);
			await pressCommandAndWait(page, "GROUP 1 DIV 2", "G1 DIV 2");
			await storeGroup({
				via: "programmer",
				surface: { via: "software", page },
				group: 5,
			});
			await selectFixtureRows(api, page, state.sourceOrder);
			await recordExistingGroup(page, 1, "Overwrite");
		},
		assert: async ({ api }, state) => {
			await expectGroupNumbers(api, "1", state.sourceOrder);
			await expectGroupNumbers(api, "5", [12, 2, 4, 6, 9, 11]);
			await expectGroup(api, "5", (group) => {
				expect(group.body.derived_from).toMatchObject({
					source_group_id: "1",
					rule: { type: "every_nth", n: 2, offset: 0 },
				});
				expect(group.body.frozen_from).toBeNull();
			});
			await expectGroupNumbers(api, "4", []);
		},
	});
}
