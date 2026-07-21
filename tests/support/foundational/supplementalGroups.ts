import { expect, test } from "../../../apps/control-ui/e2e/bench/fixtures";
import type { FoundationalCase } from "./case";
import {
	expectFixtureUnpatched,
	setFixtureAddressThroughSoftware,
	unpatchFixture,
} from "../operator";
import {
	command,
	commandError,
	expectGroup,
	expectGroupMissing,
	expectGroupNumbers,
	expectProgrammer,
	expectSelectedNumbers,
	expectSlotsAfterTick,
	fixtureIdsByNumber,
	fixtureRow,
	groupCard,
	INTENSITY,
	loadCompactRig,
	object,
	openBuiltIn,
	openFixtures,
	openGroups,
	openPatch,
	overwriteGroupByNumbers,
	patchFixtureRow,
	pressCommand,
	recordExistingGroup,
	select,
	selectFixtureRows,
	setDimmerByTouch,
	stageFixture,
} from "./helpers";

export const derivedGroupApi: FoundationalCase = {
	title: "GROUP-003 @supplemental › second API source reorder remains live",
	run: async ({ api, bench }) => {
		await loadCompactRig(api, bench, "group-003-api");
		await command(api, "GROUP 1 DIV 2");
		await command(api, "RECORD GROUP 5");
		await expectGroup(api, "5", (group) => {
			expect(group.body.derived_from).toMatchObject({
				source_group_id: "1",
				rule: { type: "every_nth", n: 2, offset: 0 },
			});
		});
		await command(api, "GROUP 5");
		await expectSelectedNumbers(api, [1, 3, 5, 7, 9, 11]);

		await overwriteGroupByNumbers(
			api,
			"1",
			[12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11],
		);
		await command(api, "GROUP 5");
		await expectSelectedNumbers(api, [12, 2, 5, 7, 9, 11]);

		await overwriteGroupByNumbers(
			api,
			"1",
			[12, 1, 2, 8, 4, 5, 6, 7, 9, 10, 11],
		);
		await command(api, "GROUP 5");
		await expectSelectedNumbers(api, [12, 2, 4, 6, 9, 11]);
		await expectGroupNumbers(api, "4", []);
	},
};

export const frozenGroupApi: FoundationalCase = {
	title:
		"GROUP-004 @supplemental › API frozen Preset storage and unpatched output boundary",
	run: async ({ api, bench }) => {
		await loadCompactRig(api, bench, "group-004-api");
		const fixtures = await fixtureIdsByNumber(api);

		await command(api, "GROUP GROUP 1");
		await command(api, "RECORD GROUP 5");
		await expectGroup(api, "5", (group) => {
			expect(group.body.frozen_from).toMatchObject({ source_group_id: "1" });
			expect(group.body.derived_from).toBeNull();
		});
		await expectGroupNumbers(api, "5", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

		await overwriteGroupByNumbers(
			api,
			"1",
			[12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11],
		);
		await unpatchFixture(api, fixtures[3]);
		await expectGroupNumbers(api, "5", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

		await command(api, "GROUP 5 AT 50");
		await expectProgrammer(api, (programmer) => {
			expect(programmer.group_values["5"]?.[INTENSITY]).toBeDefined();
			expect(programmer.selected).toContain(fixtures[3]);
		});
		await expectSlotsAfterTick(
			bench,
			3_000,
			[128, 128, 0, 128, 128, 128, 128, 128, 128, 128, 128, 128],
		);
		await command(api, "RECORD 1.1");
		const preset = await object(api, "preset", "1.1");
		expect(preset.body.group_values["5"]?.[INTENSITY]).toBeDefined();
	},
};

export const missingGroupApi: FoundationalCase = {
	title:
		"GROUP-005 @supplemental › API deletion and missing-reference errors remain atomic",
	run: async ({ api, bench }) => {
		await loadCompactRig(api, bench, "group-005-api");
		const fixtures = await fixtureIdsByNumber(api);

		await command(api, "DELETE GROUP 4");
		await expectGroupMissing(api, "4");
		await command(api, "GROUP 1 THRU 5");
		await expectSelectedNumbers(api, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

		await select(api, []);
		await command(api, "RECORD GROUP 4");
		await expectGroupNumbers(api, "4", []);
		await command(api, "GROUP 4 AT 50");
		await expectProgrammer(api, (programmer) =>
			expect(programmer.group_values["4"]?.[INTENSITY]).toBeDefined(),
		);
		await expectSlotsAfterTick(bench, 3_000, Array(12).fill(0));

		await select(api, [fixtures[1]]);
		await command(api, "RECORD + GROUP 4");
		await expectGroupNumbers(api, "4", [1]);
		await expectSlotsAfterTick(
			bench,
			3_000,
			[128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
		);

		await command(api, "DELETE GROUP 4");
		await expect(commandError(api, "GROUP 4")).resolves.toContain(
			"group 4 does not exist",
		);
		// Group recording now crosses the typed Programming boundary, which reports the missing
		// Group with a capitalized subject. The rejection wording is otherwise unchanged.
		await expect(commandError(api, "RECORD + GROUP 4")).resolves.toMatch(
			/group 4 does not exist/i,
		);
	},
};

export const derivedGroupUi: FoundationalCase = {
	title:
		"GROUP-003 @supplemental › visible derived Group follows a second source reorder",
	run: async ({ api, bench, desk, page }) => {
		await loadCompactRig(api, bench, "group-003-ui");
		await desk.open(api.baseUrl);
		await pressCommand(page, "GROUP 1 DIV 2", "G1 DIV 2");
		await pressCommand(page, "RECORD GROUP 5", "RECORD GROUP 5");

		const firstOrder = [12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11];
		await selectFixtureRows(api, page, firstOrder);
		await expectSelectedNumbers(api, firstOrder);
		await recordExistingGroup(page, 1, "Overwrite");
		await expectGroupNumbers(api, "1", firstOrder);
		await expectGroupNumbers(api, "5", [12, 2, 5, 7, 9, 11]);
		await expect(
			(await object<any>(api, "group", "5")).body.derived_from,
		).toMatchObject({
			source_group_id: "1",
			rule: { type: "every_nth", n: 2, offset: 0 },
		});

		const secondOrder = [12, 1, 2, 8, 4, 5, 6, 7, 9, 10, 11];
		await selectFixtureRows(api, page, secondOrder);
		await expectSelectedNumbers(api, secondOrder);
		await recordExistingGroup(page, 1, "Overwrite");
		await expectGroupNumbers(api, "1", secondOrder);
		await expectGroupNumbers(api, "5", [12, 2, 4, 6, 9, 11]);
		await expectGroupNumbers(api, "4", []);

		await openGroups(page);
		await groupCard(page, 5).click();
		await expectSelectedNumbers(api, [12, 2, 4, 6, 9, 11]);
	},
};

export const frozenGroupUi: FoundationalCase = {
	title:
		"GROUP-004 @supplemental › frozen Group remains visible across Stage and Fixture panes",
	run: async ({ api, bench, desk, page }) => {
		await loadCompactRig(api, bench, "group-004-ui");
		await desk.open(api.baseUrl);
		const fixtures = await fixtureIdsByNumber(api);

		await openGroups(page);
		await groupCard(page, 1).dblclick();
		await expectProgrammer(api, (state) => {
			expect(state.selected).toHaveLength(12);
			expect(state.selection_expression?.type).toBe("frozen_group");
		});
		await pressCommand(page, "RECORD GROUP 5", "RECORD GROUP 5");
		await expect(
			(await object<any>(api, "group", "5")).body.frozen_from,
		).toMatchObject({ source_group_id: "1" });

		const reordered = [12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11];
		await selectFixtureRows(api, page, reordered);
		await recordExistingGroup(page, 1, "Overwrite");
		await expectGroupNumbers(api, "1", reordered);

		await openPatch(page);
		const fixture3Row = patchFixtureRow(page, 3);
		const address = fixture3Row.locator(".patch-address");
		await setFixtureAddressThroughSoftware({
			page,
			addressCell: address,
			address: null,
		});
		await expect(address).toHaveText("Unpatched");
		await expectFixtureUnpatched(api, fixtures[3]);

		await openGroups(page);
		await expect(groupCard(page, 5)).toHaveClass(/frozen/);
		await expect(groupCard(page, 5)).not.toContainText("missing");
		await groupCard(page, 5).click();
		await setDimmerByTouch(page, 50);
		await expectGroupNumbers(api, "5", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
		await expectProgrammer(api, (state) =>
			expect(state.group_values["5"]?.[INTENSITY]).toBeDefined(),
		);
		await expectSlotsAfterTick(
			bench,
			3_000,
			[128, 128, 0, 128, 128, 128, 128, 128, 128, 128, 128, 128],
		);

		await openFixtures(page);
		await expect(fixtureRow(page, 3)).toBeVisible();
		await openBuiltIn(page, "Stage");
		await expect(stageFixture(page, fixtures[3])).toBeVisible();
	},
};

export const missingGroupUi: FoundationalCase = {
	title:
		"GROUP-005 @supplemental › visible deletion, missing errors, and repeated range skipping",
	run: async ({ api, bench, desk, page }) => {
		test.setTimeout(60_000);
		await loadCompactRig(api, bench, "group-005-ui");
		await command(api, "DELETE GROUP 4");
		await expectGroupMissing(api, "4");
		await desk.open(api.baseUrl);

		await pressCommand(page, "GROUP 1 THRU 5", "G1 THRU 5");
		await expectProgrammer(api, (state) => {
			expect(
				state.selection_expression?.items.map((item: any) => item.group_id),
			).toEqual(["1", "2", "3"]);
		});
		await expectGroupMissing(api, "4");
		await page.getByRole("button", { name: "CLR", exact: true }).click();
		await expectSelectedNumbers(api, []);

		await openGroups(page);
		await page.locator(".global-store-button").click();
		await groupCard(page, 4).click();
		await expect(page.locator(".record-mode-dialog")).toHaveCount(0);
		await expectGroupNumbers(api, "4", []);
		await expect(page.locator(".global-store-button")).toHaveText("REC");
		await expect(groupCard(page, 4)).toContainText("Group 4");

		await groupCard(page, 4).click();
		await expectProgrammer(api, (state) => {
			expect(state.selection_expression).toMatchObject({
				type: "sources",
				items: [{ type: "live_group", group_id: "4" }],
			});
		});
		await expect(
			page
				.locator(".vertical-touch-fader-stack")
				.filter({ hasText: "Enc 1 · Dimmer" }),
		).toBeVisible();
		await setDimmerByTouch(page, 50);
		await expectProgrammer(api, (state) =>
			expect(state.group_values["4"]?.[INTENSITY]).toBeDefined(),
		);
		await expectSlotsAfterTick(bench, 3_000, Array(12).fill(0));

		await openFixtures(page);
		await fixtureRow(page, 1).click();
		await expectSelectedNumbers(api, [1]);
		await openGroups(page);
		await page.locator(".global-store-button").click();
		await groupCard(page, 4).click();
		await expect(page.locator(".record-mode-dialog")).toHaveCount(0);
		await expectGroupNumbers(api, "4", [1]);
		await expectSlotsAfterTick(
			bench,
			0,
			[128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
		);

		await pressCommand(page, "DELETE GROUP 4", "DELETE GROUP 4");
		await expectGroupMissing(api, "4");
		await pressCommand(page, "GROUP 4", "G4");
		await expect(page.getByLabel("Command line")).toHaveClass(/error/);

		await pressCommand(page, "GROUP 1 THRU 5", "G1 THRU 5");
		await expectProgrammer(api, (state) => {
			expect(
				state.selection_expression?.items.map((item: any) => item.group_id),
			).toEqual(["1", "2", "3"]);
		});
		await expectGroupMissing(api, "4");
	},
};
