import { expect, test } from "../../../apps/control-ui/e2e/bench/fixtures";
import type { FoundationalCase } from "./case";
import {
	command,
	expectProgrammer,
	expectSelectedNumbers,
	expectSlotsAfterTick,
	fixtureIdsByNumber,
	fixtureRow,
	groupCard,
	INTENSITY,
	loadCompactRig,
	openBuiltIn,
	openFixtures,
	openGroups,
	overwriteGroupByNumbers,
	pressCommand,
	putObject,
	setDimmerByTouch,
	stageFixture,
} from "./helpers";

export type FoundationalSurface = "api" | "ui";

export const supplementalSurfaceFactories = [
	(surface: FoundationalSurface): FoundationalCase => ({
		title: `PROG-001 @supplemental-${surface} › drag, Preset, and mixed-source selection boundaries`,
		run: async ({ api, bench, desk, page }) => {
			await loadCompactRig(api, bench, `prog-001-${surface}`);
			if (surface === "ui") {
				test.setTimeout(90_000);
				const fixtures = await fixtureIdsByNumber(api);
				await putObject(api, "preset", "1.199", {
					name: "Selection Intensity",
					family: "Intensity",
					number: 199,
					values: {
						[fixtures[21]]: { intensity: { kind: "normalized", value: 0.6 } },
						[fixtures[22]]: { intensity: { kind: "normalized", value: 0.6 } },
					},
					group_values: {},
				});
				await desk.open(api.baseUrl);
				await openBuiltIn(page, "Stage");
				await stageFixture(page, fixtures[1]).click();
				await stageFixture(page, fixtures[2]).click();
				await expectSelectedNumbers(api, [1, 2]);

				const fixture3 = await stageFixture(page, fixtures[3]).boundingBox();
				const fixture4 = await stageFixture(page, fixtures[4]).boundingBox();
				expect(fixture3).toBeTruthy();
				expect(fixture4).toBeTruthy();
				await page.mouse.move(
					Math.min(fixture3!.x, fixture4!.x) - 3,
					Math.min(fixture3!.y, fixture4!.y) - 3,
				);
				await page.mouse.down();
				await page.mouse.move(
					Math.max(
						fixture3!.x + fixture3!.width,
						fixture4!.x + fixture4!.width,
					) + 3,
					Math.max(
						fixture3!.y + fixture3!.height,
						fixture4!.y + fixture4!.height,
					) + 3,
					{ steps: 5 },
				);
				await page.mouse.up();
				await expectSelectedNumbers(api, [1, 2, 3, 4]);

				await openFixtures(page);
				await fixtureRow(page, 5).click();
				await openGroups(page);
				await groupCard(page, 2).click();
				await groupCard(page, 3).click();
				await groupCard(page, 1).click();
				await expectSelectedNumbers(
					api,
					[1, 2, 3, 4, 5, 7, 9, 11, 6, 8, 10, 12],
				);
				await expectProgrammer(api, (state) => {
					expect(state.selection_expression?.type).toBe("sources");
					expect(
						state.selection_expression?.items.map((item: any) => item.type),
					).toEqual([
						"fixture",
						"fixture",
						"fixture",
						"fixture",
						"fixture",
						"live_group",
						"live_group",
						"live_group",
					]);
				});
				await setDimmerByTouch(page, 50);
				await expectSelectedNumbers(
					api,
					[1, 2, 3, 4, 5, 7, 9, 11, 6, 8, 10, 12],
				);
				await setDimmerByTouch(page, 25);

				await openBuiltIn(page, "Stage");
				await stageFixture(page, fixtures[21]).click();
				await stageFixture(page, fixtures[22]).click();
				await expectSelectedNumbers(api, [21, 22]);
				await openBuiltIn(page, "Presets");
				await page
					.locator(".preset-card")
					.filter({ hasText: "Selection Intensity" })
					.click();
				await expectSelectedNumbers(api, [21, 22]);
				await pressCommand(page, "+ 23", "+F23");
				await expectSelectedNumbers(api, [21, 22, 23]);
				await setDimmerByTouch(page, 40);

				await openGroups(page);
				await groupCard(page, 3).click();
				await setDimmerByTouch(page, 25);
				await expectSelectedNumbers(api, [1, 2, 3, 4]);

				await openBuiltIn(page, "Stage");
				await stageFixture(page, fixtures[6]).click();
				await expectSelectedNumbers(api, [6]);
				await page.getByRole("button", { name: "CLR", exact: true }).click();
				await expectSelectedNumbers(api, []);
				await stageFixture(page, fixtures[7]).click();
				await openGroups(page);
				await groupCard(page, 3).click();
				await expectSelectedNumbers(api, [7, 1, 2, 3, 4]);
				await expectSlotsAfterTick(bench, 3_000, Array(12).fill(64));
			} else {
				await command(api, "1 + 2 AT 50");
				await command(api, "AT 25");
				await command(api, "3 AT 75");
				await command(api, "+ 4 AT 100");
				await expectSelectedNumbers(api, [3, 4]);
				await expectSlotsAfterTick(
					bench,
					3_000,
					[64, 64, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0],
				);
			}
		},
	}),
	(surface: FoundationalSurface): FoundationalCase => ({
		title: `PROG-002 @supplemental-${surface} › repeated ascending live-order spread`,
		run: async ({ api, bench, desk, page }) => {
			await loadCompactRig(api, bench, `prog-002-${surface}`);
			await overwriteGroupByNumbers(api, "1", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
			if (surface === "ui") {
				await desk.open(api.baseUrl);
				await pressCommand(page, "GROUP 1 AT 0 THRU 100", "G1 AT 0 THRU 100");
			} else {
				await command(api, "GROUP 1 AT 0 THRU 100");
			}
			await expectProgrammer(api, (state) => {
				expect(state.group_values["1"]?.[INTENSITY]?.value).toMatchObject({
					kind: "spread",
					value: [0, 1],
				});
				expect(state.values).toHaveLength(0);
			});
			await expectSlotsAfterTick(
				bench,
				3_000,
				[0, 28, 57, 85, 113, 142, 170, 198, 227, 255, 0, 0],
			);
			await overwriteGroupByNumbers(
				api,
				"1",
				[12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
			);
			await expectSlotsAfterTick(
				bench,
				0,
				[26, 51, 77, 102, 128, 153, 179, 204, 230, 255, 0, 0],
			);
		},
	}),
];
