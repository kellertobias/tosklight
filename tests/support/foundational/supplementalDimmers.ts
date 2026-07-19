import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import type { FoundationalCase } from "./case";
import {
	command,
	commandError,
	expectGroupMissing,
	expectGroupNumbers,
	expectProgrammer,
	expectSelectedNumbers,
	expectSlotsAfterTick,
	expectVisibleGroupOrder,
	fixtureIdsByNumber,
	fixtureRow,
	gestureFixture,
	gestureGroup,
	groupCard,
	INTENSITY,
	loadCompactRig,
	normalized,
	object,
	objects,
	openFixtures,
	openGroups,
	pressCommand,
	putObject,
	recordExistingGroup,
	select,
	setDimmerByTouch,
} from "./helpers";

export const dimmerApiBoundaries: FoundationalCase = {
	title:
		"DIM-001 @supplemental › exhaustive API add, subtract, deletion, and dependency boundaries",
	run: async ({ api, bench }) => {
		const prepare = async (name: string) => {
			await loadCompactRig(api, bench, name);
			const fixtures = await fixtureIdsByNumber(api);
			await command(api, "GROUP 3 AT 50");
			await expectProgrammer(api, (programmer) => {
				expect(programmer.group_values["3"]?.[INTENSITY]).toBeDefined();
				expect(programmer.values).toHaveLength(0);
				expect(programmer.selection_expression).toMatchObject({
					type: "live_group",
					group_id: "3",
				});
			});
			await expectSlotsAfterTick(bench, 3_000, [128, 128, 128, 128, 0, 0]);
			return fixtures;
		};

		// The three documented add-to-end workflows start from independent show copies.
		for (const workflow of [
			"merge",
			"live-overwrite",
			"command-merge",
		] as const) {
			const fixtures = await prepare(`dim-001-${workflow}-api`);
			if (workflow === "live-overwrite") {
				await gestureGroup(api, "3");
				await gestureFixture(api, fixtures[5]);
				await gestureFixture(api, fixtures[6]);
				await expectProgrammer(api, (programmer) => {
					expect(programmer.selection_expression).toMatchObject({
						type: "sources",
						items: [
							{ type: "live_group", group_id: "3" },
							{ type: "fixture", fixture_id: fixtures[5] },
							{ type: "fixture", fixture_id: fixtures[6] },
						],
					});
				});
				await command(api, "RECORD GROUP 3");
			} else {
				await gestureFixture(api, fixtures[5]);
				await gestureFixture(api, fixtures[6]);
				await expectSelectedNumbers(api, [5, 6]);
				await command(api, "RECORD + GROUP 3");
			}
			await expectGroupNumbers(api, "3", [1, 2, 3, 4, 5, 6]);
			await expectProgrammer(api, (programmer) =>
				expect(programmer.group_values["3"]?.[INTENSITY]).toBeDefined(),
			);
		}

		// Removal retains relative order; a later add and a remove-then-add in one expression append.
		await prepare("dim-001-primary-api");
		await command(api, "G3 + F5 + F6");
		await command(api, "RECORD GROUP 3");
		await command(api, "G3 - F2");
		await command(api, "RECORD GROUP 3");
		await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6]);
		await expectSlotsAfterTick(bench, 0, [128, 0, 128, 128, 128, 128]);
		await command(api, "G3 + F2");
		await command(api, "RECORD GROUP 3");
		await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6, 2]);
		await expectSlotsAfterTick(bench, 0, [128, 128, 128, 128, 128, 128]);

		await prepare("dim-001-left-to-right-api");
		await command(api, "G3 + F5 + F6");
		await command(api, "RECORD GROUP 3");
		await command(api, "G3 - F2 + F2");
		await expectSelectedNumbers(api, [1, 3, 4, 5, 6, 2]);
		await command(api, "RECORD GROUP 3");
		await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6, 2]);

		// Subtract accepts one fixture or an ordered multi-fixture gesture without rebuilding Group 3.
		const subtractFixtures = await prepare("dim-001-subtract-api");
		await command(api, "G3 + F5 + F6");
		await command(api, "RECORD GROUP 3");
		await gestureFixture(api, subtractFixtures[2]);
		await command(api, "RECORD - GROUP 3");
		await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6]);
		await gestureFixture(api, subtractFixtures[5]);
		await gestureFixture(api, subtractFixtures[6]);
		await command(api, "RECORD - GROUP 3");
		await expectGroupNumbers(api, "3", [1, 3, 4]);

		// Empty-selection subtract and DELETE are equivalent both when accepted and dependency-blocked.
		for (const operation of ["RECORD - GROUP 3", "DELETE GROUP 3"] as const) {
			await prepare(
				`dim-001-delete-${operation.startsWith("RECORD") ? "subtract" : "delete"}-api`,
			);
			await select(api, []);
			await command(api, operation);
			await expectGroupMissing(api, "3");
		}
		let rejection = "";
		for (const operation of ["RECORD - GROUP 3", "DELETE GROUP 3"] as const) {
			await prepare(
				`dim-001-dependent-${operation.startsWith("RECORD") ? "subtract" : "delete"}-api`,
			);
			const group3 = await object(api, "group", "3");
			await putObject(api, "group", "6", {
				id: "6",
				name: "Depends on 3",
				fixtures: [],
				derived_from: { source_group_id: "3", rule: { type: "all" } },
				frozen_from: null,
				programming: {},
				master: 1,
				playback_fader: null,
			});
			const before = JSON.stringify(await objects(api, "group"));
			await select(api, []);
			const error = await commandError(api, operation);
			expect(error).toContain("derived group 6 depends on it");
			if (rejection) expect(error).toContain(rejection);
			else rejection = "cannot delete group 3";
			expect(JSON.stringify(await objects(api, "group"))).toBe(before);
			expect((await object(api, "group", "3")).revision).toBe(group3.revision);
		}
	},
};

export const dimmerFadeApi: FoundationalCase = {
	title: "DIM-002 @supplemental › repeated API fade endpoint and UDP stability",
	run: async ({ api, bench }) => {
		await loadCompactRig(api, bench, "dim-002-api");
		await command(api, "GROUP 1 AT 50");
		await expectProgrammer(api, (programmer) =>
			expect(normalized(programmer.group_values["1"][INTENSITY].value)).toBe(
				0.5,
			),
		);
		await expectSlotsAfterTick(bench, 2_999, Array(12).fill(127));
		await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
		await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
	},
};

export const dimmerFadeUi: FoundationalCase = {
	title: "DIM-002 @supplemental › repeated visible keypad fade endpoint",
	run: async ({ api, bench, desk, page }) => {
		await loadCompactRig(api, bench, "dim-002-ui");
		await desk.open(api.baseUrl);
		await pressCommand(page, "GROUP 1 AT 50", "G1 AT 50");
		await expectProgrammer(api, (programmer) =>
			expect(normalized(programmer.group_values["1"][INTENSITY].value)).toBe(
				0.5,
			),
		);
		await expectSlotsAfterTick(bench, 2_999, Array(12).fill(127));
		await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
		await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
	},
};

export const dimmerDialogsUi: FoundationalCase = {
	title:
		"DIM-001 @supplemental › visible Merge and Overwrite dialogs retain live ordering",
	run: async ({ api, bench, desk, page }) => {
		await loadCompactRig(api, bench, "dim-001-ui");
		await desk.open(api.baseUrl);

		await openGroups(page);
		await groupCard(page, 3).click();
		await setDimmerByTouch(page, 50);
		await expectProgrammer(api, (state) => {
			expect(state.group_values["3"]?.[INTENSITY]).toBeDefined();
			expect(state.values).toHaveLength(0);
		});

		await openFixtures(page);
		await fixtureRow(page, 5).click();
		await fixtureRow(page, 6).click();
		await expectSelectedNumbers(api, [5, 6]);
		await openGroups(page);
		await recordExistingGroup(page, 3, "Merge");
		await expectGroupNumbers(api, "3", [1, 2, 3, 4, 5, 6]);
		await expectSlotsAfterTick(bench, 3_000, [128, 128, 128, 128, 128, 128]);

		await pressCommand(page, "GROUP 3 - 2", "G3 - F2");
		await recordExistingGroup(page, 3, "Overwrite");
		await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6]);
		await expectVisibleGroupOrder(page, 3, [1, 3, 4, 5, 6]);
		await expectSlotsAfterTick(bench, 0, [128, 0, 128, 128, 128, 128]);

		await pressCommand(page, "GROUP 3 + 2", "G3 + F2");
		await recordExistingGroup(page, 3, "Overwrite");
		await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6, 2]);
		await expectVisibleGroupOrder(page, 3, [1, 3, 4, 5, 6, 2]);
		await expectSlotsAfterTick(bench, 0, [128, 128, 128, 128, 128, 128]);
	},
};
