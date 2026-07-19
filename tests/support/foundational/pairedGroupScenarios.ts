import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../../apps/control-ui/e2e/bench/pairedScenario";
import { storeGroup } from "../operator";
import {
	command,
	expectGroup,
	expectGroupNumbers,
	expectProgrammer,
	expectSlotsAfterTick,
	fixtureIdsByNumber,
	fixtureRow,
	groupCard,
	INTENSITY,
	loadCompactRig,
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
			await storeGroup({
				via: "programmer",
				surface: { via: "command-line", api },
				group: 5,
			});
			await overwriteGroupByNumbers(api, "1", state.sourceOrder);
			await unpatchFixture(api, state.fixtures[3]);
			await command(api, "GROUP 5 AT 50");
		},
		ui: async ({ api, desk, page }, state) => {
			await desk.open(api.baseUrl);
			await openGroups(page);
			await groupCard(page, 1).dblclick();
			await storeGroup({
				via: "programmer",
				surface: { via: "software", page },
				group: 5,
			});
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
