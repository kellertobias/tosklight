import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../../apps/control-ui/e2e/bench/pairedScenario";
import {
	command,
	enterCommandWithoutEscape,
	expectGroup,
	expectGroupNumbers,
	expectProgrammer,
	expectSelectedNumbers,
	expectSlotsAfterTick,
	fixtureIdsByNumber,
	fixtureRow,
	groupCard,
	INTENSITY,
	loadCompactRig,
	normalized,
	object,
	openFixtures,
	openGroups,
	openPatch,
	overwriteGroupByNumbers,
	patchFixtureRow,
	pressCommandAndWait,
	programmer,
	recordExistingGroup,
	select,
	selectFixtureRows,
	setDimmerByTouch,
	slotsFromFrame,
	unpatchFixture,
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
			await api.command("programmer.command_target", { value: "GROUP" });
			await api.command("programmer.command_target", { value: "FIXTURE" });
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
			await command(api, "RECORD GROUP 5");
			await overwriteGroupByNumbers(api, "1", state.sourceOrder);
		},
		ui: async ({ api, desk, page }, state) => {
			await desk.open(api.baseUrl);
			await pressCommandAndWait(page, "GROUP 1 DIV 2", "G1 DIV 2");
			await pressCommandAndWait(page, "RECORD GROUP 5", "RECORD GROUP 5");
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

export function registerFrozenAndEmptyGroupPairedScenarios() {
	pairedScenario({
		id: "GROUP-004",
		title:
			"frozen Group survives source edits and keeps unpatched fixtures programmable",
		arrange: async ({ api, bench }, surface) => {
			await loadCompactRig(api, bench, `group-004-paired-${surface}`);
			return {
				fixtures: await fixtureIdsByNumber(api),
				sourceOrder: [12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11],
			};
		},
		api: async ({ api }, state) => {
			await command(api, "GROUP GROUP 1");
			await command(api, "RECORD GROUP 5");
			await overwriteGroupByNumbers(api, "1", state.sourceOrder);
			await unpatchFixture(api, state.fixtures[3]);
			await command(api, "GROUP 5 AT 50");
		},
		ui: async ({ api, desk, page }, state) => {
			await desk.open(api.baseUrl);
			await openGroups(page);
			await groupCard(page, 1).dblclick();
			await pressCommandAndWait(page, "RECORD GROUP 5", "RECORD GROUP 5");
			await selectFixtureRows(api, page, state.sourceOrder);
			await recordExistingGroup(page, 1, "Overwrite");
			await openPatch(page);
			const row = patchFixtureRow(page, 3);
			const address = row.locator(".patch-address");
			await page.getByRole("button", { name: "SET", exact: true }).click();
			await address.click();
			const editor = page.locator(".patch-edit-modal");
			await editor.getByLabel("Fixture address").fill("");
			await editor.getByRole("button", { name: "Set", exact: true }).click();
			await expect(address).toHaveText("Unpatched");
			await openGroups(page);
			await groupCard(page, 5).click();
			await setDimmerByTouch(page, 50);
		},
		assert: async ({ api, bench }, state) => {
			await expectGroup(api, "5", (group) => {
				expect(group.body.frozen_from).toMatchObject({ source_group_id: "1" });
				expect(group.body.derived_from).toBeNull();
			});
			await expectGroupNumbers(
				api,
				"5",
				[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
			);
			const fixture3 = await object<any>(
				api,
				"patched_fixture",
				state.fixtures[3],
			);
			expect(fixture3.body.universe).toBeNull();
			expect(fixture3.body.address).toBeNull();
			await expectProgrammer(api, (programmerState) =>
				expect(programmerState.group_values["5"]?.[INTENSITY]).toBeDefined(),
			);
			await expectSlotsAfterTick(
				bench,
				3_000,
				[128, 128, 0, 128, 128, 128, 128, 128, 128, 128, 128, 128],
			);
		},
	});

	pairedScenario<{
		fixtures: Record<number, string>;
		rangedGroups: string[];
		emptySlots: number[];
	}>({
		id: "GROUP-005",
		title: "stored empty Groups remain distinct from missing references",
		arrange: async ({ api, bench }, surface) => {
			await loadCompactRig(api, bench, `group-005-paired-${surface}`);
			await command(api, "DELETE GROUP 4");
			return {
				fixtures: await fixtureIdsByNumber(api),
				rangedGroups: [],
				emptySlots: [],
			};
		},
		api: async ({ api, bench }, state) => {
			await command(api, "GROUP 1 THRU 5");
			state.rangedGroups =
				(await programmer(api)).selection_expression?.items.map(
					(item: any) => item.group_id,
				) ?? [];
			await select(api, []);
			await command(api, "RECORD GROUP 4");
			await command(api, "GROUP 4 AT 50");
			state.emptySlots = slotsFromFrame(await bench.tick(3_000), 12);
			await select(api, [state.fixtures[1]]);
			await command(api, "RECORD + GROUP 4");
		},
		ui: async ({ api, bench, desk, page }, state) => {
			await desk.open(api.baseUrl);
			await pressCommandAndWait(page, "GROUP 1 THRU 5", "G1 THRU 5");
			await expectProgrammer(api, (programmerState) => {
				state.rangedGroups =
					programmerState.selection_expression?.items.map(
						(item: any) => item.group_id,
					) ?? [];
				expect(state.rangedGroups).toEqual(["1", "2", "3"]);
			});
			await page.getByRole("button", { name: "CLR", exact: true }).click();
			await openGroups(page);
			await page.locator(".global-store-button").click();
			await groupCard(page, 4).click();
			await expect(page.locator(".record-mode-dialog")).toHaveCount(0);
			await expectGroupNumbers(api, "4", []);
			await expect(page.locator(".global-store-button")).toHaveText("REC");
			await expect(groupCard(page, 4)).toContainText("Group 4");
			await groupCard(page, 4).click();
			await expectProgrammer(api, (programmerState) => {
				expect(programmerState.selection_expression).toMatchObject({
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
			state.emptySlots = slotsFromFrame(await bench.tick(3_000), 12);
			await openFixtures(page);
			await fixtureRow(page, 1).click();
			await openGroups(page);
			await page.locator(".global-store-button").click();
			await groupCard(page, 4).click();
		},
		assert: async ({ api, bench }, state) => {
			expect(state.rangedGroups).toEqual(["1", "2", "3"]);
			expect(state.emptySlots).toEqual(Array(12).fill(0));
			await expectGroupNumbers(api, "4", [1]);
			await expectProgrammer(api, (programmerState) =>
				expect(programmerState.group_values["4"]?.[INTENSITY]).toBeDefined(),
			);
			await expectSlotsAfterTick(
				bench,
				0,
				[128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
			);
		},
	});
}

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
