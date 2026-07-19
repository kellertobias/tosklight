import type { ApiDriver } from "../../../apps/control-ui/e2e/bench/api";
import { expect, test } from "../../../apps/control-ui/e2e/bench/fixtures";
import type { Page } from "../../../apps/control-ui/node_modules/@playwright/test/index.js";
import type { FoundationalCase } from "./case";
import {
	command,
	commandError,
	expectGroup,
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
	openBuiltIn,
	openFixtures,
	openGroups,
	openPatch,
	overwriteGroupByNumbers,
	patchFixtureRow,
	pressCommand,
	programmer,
	putObject,
	recordExistingGroup,
	select,
	selectFixtureRows,
	setDimmerByTouch,
	setGroupByNumbers,
	stageFixture,
	unpatchFixture,
} from "./helpers";

interface DeskDriver {
	open(baseUrl: string): Promise<unknown>;
}

function commandUiActions(api: ApiDriver, page: Page) {
	const commandLine = page.getByLabel("Command line");
	const press = (key: string) =>
		page.getByRole("button", { name: key, exact: true }).click();
	const enter = async (
		keys: string[],
		visible: string,
		selected: number[],
		target: "FIXTURE" | "GROUP",
	) => {
		for (const key of keys) await press(key);
		await expect(commandLine).toHaveValue(visible);
		await press("ENT");
		await expect(commandLine).toHaveValue(target);
		await expectSelectedNumbers(api, selected);
	};
	const clear = async (target: "FIXTURE" | "GROUP") => {
		await press("CLR");
		await expect(commandLine).toHaveValue(target);
		await expectSelectedNumbers(api, []);
	};
	return { commandLine, press, enter, clear };
}

async function prepareCommandUi(
	api: ApiDriver,
	bench: unknown,
	desk: DeskDriver,
	page: Page,
) {
	await loadCompactRig(api, bench, "cmd-001-ui");
	await setGroupByNumbers(api, "4", "Middle Pair", [9, 10]);
	await setGroupByNumbers(api, "5", "Back Dimmers", [5, 6, 7, 8]);
	await desk.open(api.baseUrl);
	const controlSection = await page.locator(".control-section").boundingBox();
	const programmerRight = await page
		.locator(".control-right-pane")
		.boundingBox();
	expect(programmerRight?.width).toBeCloseTo(384, 0);
	expect(
		controlSection!.x +
			controlSection!.width -
			(programmerRight!.x + programmerRight!.width),
	).toBeLessThanOrEqual(6);
	await page.getByRole("button", { name: /Prog\. Fade/ }).click();
	const fadeDialog = page.getByRole("dialog", { name: "Prog. Fade value" });
	await expect(
		fadeDialog.getByRole("slider", { name: "Prog. Fade" }),
	).toBeVisible();
	await expect(fadeDialog.getByLabel("Number input keypad")).toBeVisible();
	await fadeDialog
		.getByRole("button", { name: "Close attribute value" })
		.click();
	await expect(page.getByRole("slider", { name: "Prog. Fade" })).toHaveCount(0);
	await page.locator(".mode-toggle").click();
	await expect(page.getByRole("slider", { name: "Prog. Fade" })).toBeVisible();
	await expect(page.getByRole("slider", { name: "Cue Fade" })).toBeVisible();
	const playbackRight = await page.locator(".control-right-pane").boundingBox();
	expect(playbackRight?.width).toBeCloseTo(384, 0);
	expect(playbackRight?.x).toBeCloseTo(programmerRight!.x, 0);
	await page.locator(".mode-toggle").click();
	await expect(page.getByRole("slider", { name: "Prog. Fade" })).toHaveCount(0);
}

async function exerciseGroupCommandMode(api: ApiDriver, page: Page) {
	const { commandLine, press, enter, clear } = commandUiActions(api, page);
	await expect(commandLine).toHaveValue("FIXTURE");
	await press("GRP");
	await expect(commandLine).toHaveValue("GROUP");
	await press("ENT");
	await expect(commandLine).toHaveValue("GROUP");
	await expectSelectedNumbers(api, []);
	await enter(
		["1", "+", "2"],
		"G1 + G2",
		[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
		"GROUP",
	);
	await clear("GROUP");
	await press("GRP");
	await expect(commandLine).toHaveValue("FIXTURE");
	await enter(["1", "+", "2"], "F1 + G2", [1, 3, 5, 7, 9, 11], "GROUP");
	await clear("GROUP");
	await enter(["GRP", "1", "+", "GRP", "2"], "F1 + F2", [1, 2], "GROUP");
	await clear("GROUP");
	await enter(
		["3", "TRU", "5"],
		"G3 THRU 5",
		[1, 2, 3, 4, 9, 10, 5, 6, 7, 8],
		"GROUP",
	);
	await clear("GROUP");
	await enter(
		["3", "TRU", "5", "+", "GRP", "6"],
		"G3 THRU 5 + F6",
		[1, 2, 3, 4, 9, 10, 5, 6, 7, 8],
		"GROUP",
	);
	await clear("GROUP");
}

async function exerciseFixtureCommandMode(api: ApiDriver, page: Page) {
	const { commandLine, press, enter, clear } = commandUiActions(api, page);
	await press("GRP");
	await expect(commandLine).toHaveValue("FIXTURE");
	await press("ENT");
	await expect(commandLine).toHaveValue("FIXTURE");
	await expectSelectedNumbers(api, []);
	await enter(["1", "+", "2"], "F1 + F2", [1, 2], "FIXTURE");
	await clear("FIXTURE");
	await enter(
		["GRP", "1", "+", "2"],
		"G1 + F2",
		[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
		"FIXTURE",
	);
	await clear("FIXTURE");
	await enter(
		["GRP", "1", "+", "GRP", "2"],
		"G1 + G2",
		[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
		"FIXTURE",
	);
	await clear("FIXTURE");
	await press("GRP");
	await press("GRP");
	await expect(commandLine).toHaveValue("DEGRP");
	await enter(
		["3", "+", "GRP", "5"],
		"DEGRP 3 + G5",
		[1, 2, 3, 4, 5, 6, 7, 8],
		"FIXTURE",
	);
	await clear("FIXTURE");
	await enter(
		["GRP", "3", "TRU", "5"],
		"G3 THRU 5",
		[1, 2, 3, 4, 9, 10, 5, 6, 7, 8],
		"FIXTURE",
	);
	await clear("FIXTURE");
	await enter(
		["GRP", "3", "TRU", "5", "+", "6"],
		"G3 THRU 5 + F6",
		[1, 2, 3, 4, 9, 10, 5, 6, 7, 8],
		"FIXTURE",
	);
	await clear("FIXTURE");
	await page.getByRole("button", { name: "ESC", exact: true }).click();
	await expect(commandLine).toHaveValue("FIXTURE");
}

export const supplementalBefore: FoundationalCase[] = [
	{
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
				expect((await object(api, "group", "3")).revision).toBe(
					group3.revision,
				);
			}
		},
	},
	{
		title:
			"DIM-002 @supplemental › repeated API fade endpoint and UDP stability",
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
	},
	{
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
	},
	{
		title:
			"CMD-001 @supplemental › exhaustive visible prefix, geometry, range, Clear, and Escape cases",
		run: async ({ api, bench, desk, page }) => {
			await prepareCommandUi(api, bench, desk, page);
			await exerciseGroupCommandMode(api, page);
			await exerciseFixtureCommandMode(api, page);
		},
	},
	{
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
	},
	{
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
			await expectGroupNumbers(
				api,
				"5",
				[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
			);

			await overwriteGroupByNumbers(
				api,
				"1",
				[12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11],
			);
			await unpatchFixture(api, fixtures[3]);
			await expectGroupNumbers(
				api,
				"5",
				[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
			);

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
	},
	{
		title:
			"PROG-001 @supplemental › Preset numbers are local to each family pool",
		run: async ({ api, bench }) => {
			await loadCompactRig(api, bench, "prog-001-family-local-preset-numbers");
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
			await api.command("preset.apply", { family: "Color", number: 1 });
			await api.command("preset.apply", { family: "Position", number: 1 });
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
	},
	{
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
			await expect(commandError(api, "RECORD + GROUP 4")).resolves.toContain(
				"group 4 does not exist",
			);
		},
	},
	{
		title:
			"PROG-003 @supplemental › API higher/lower LTP and scoped release permutations",
		run: async ({ api, bench }) => {
			await loadCompactRig(api, bench, "prog-003-api");
			const fixtures = await fixtureIdsByNumber(api);

			await command(api, "GROUP 1 AT 50");
			await command(api, "1 AT 75");
			await api.command("programmer.set", {
				fixture_id: fixtures[1],
				attribute: "pan",
				value: 0.33,
			});
			await expectSlotsAfterTick(
				bench,
				3_000,
				[191, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128],
			);

			await api.command("programmer.release", {
				fixture_id: fixtures[1],
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

			await loadCompactRig(api, bench, "prog-003-lower-api");
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
			await api.command("programmer.group.set", {
				group_id: "1",
				attribute: "pan",
				value: 0.4,
			});
			await api.command("programmer.group.release", {
				group_id: "1",
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
			await expectSlotsAfterTick(
				bench,
				0,
				[64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
			);
		},
	},
	{
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
				expect(
					programmer.values.map((value) => normalized(value.value)),
				).toEqual([0.5, 0.5]);
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
	},
	{
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
	},
	{
		title:
			"CMD-001 @supplemental › exhaustive API default-mode, range, and dereference cases",
		run: async ({ api, bench }) => {
			await loadCompactRig(api, bench, "cmd-001-api");
			await setGroupByNumbers(api, "4", "Middle Pair", [9, 10]);
			await setGroupByNumbers(api, "5", "Back Dimmers", [5, 6, 7, 8]);

			type ExpectedSource = ["fixture" | "live_group", number | string];
			const enter = async (
				value: string,
				expectedNumbers: number[],
				expectedSources: ExpectedSource[],
			) => {
				await api.command("programmer.command_line", { value });
				expect((await programmer(api)).command_line).toBe(value);
				await command(api, value);
				await expectSelectedNumbers(api, expectedNumbers);
				const state = await programmer(api);
				expect(state.selection_expression?.type).toBe("sources");
				const sources = state.selection_expression.items.map((source: any) =>
					source.type === "fixture"
						? ["fixture", source.fixture_id]
						: ["live_group", source.group_id],
				);
				const fixtures = await fixtureIdsByNumber(api);
				expect(sources).toEqual(
					expectedSources.map(([type, id]) => [
						type,
						type === "fixture" ? fixtures[id as number] : String(id),
					]),
				);
				await select(api, []);
			};

			// Cases 1–8: Group is the persistent default. Bare terms are live Groups while explicit
			// Fixture terms remain scoped to only their own address term.
			await api.command("programmer.command_target", { value: "GROUP" });
			await select(api, []);
			await enter(
				"G1 + G2",
				[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
				[
					["live_group", "1"],
					["live_group", "2"],
				],
			);
			await enter(
				"F1 + G2",
				[1, 3, 5, 7, 9, 11],
				[
					["fixture", 1],
					["live_group", "2"],
				],
			);
			await enter(
				"F1 + F2",
				[1, 2],
				[
					["fixture", 1],
					["fixture", 2],
				],
			);
			await enter(
				"G3 THRU 5",
				[1, 2, 3, 4, 9, 10, 5, 6, 7, 8],
				[
					["live_group", "3"],
					["live_group", "4"],
					["live_group", "5"],
				],
			);
			await enter(
				"G3 THRU 5 + F6",
				[1, 2, 3, 4, 9, 10, 5, 6, 7, 8],
				[
					["live_group", "3"],
					["live_group", "4"],
					["live_group", "5"],
					["fixture", 6],
				],
			);

			// Cases 9–16: Fixture is the persistent default. A single explicit Group prefix remains
			// live; DEGRP expands only its own term to fixture references.
			await api.command("programmer.command_target", { value: "FIXTURE" });
			await enter(
				"F1 + F2",
				[1, 2],
				[
					["fixture", 1],
					["fixture", 2],
				],
			);
			await enter(
				"G1 + F2",
				[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
				[
					["live_group", "1"],
					["fixture", 2],
				],
			);
			await enter(
				"G1 + G2",
				[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
				[
					["live_group", "1"],
					["live_group", "2"],
				],
			);
			await enter(
				"DEGRP 3 + G5",
				[1, 2, 3, 4, 5, 6, 7, 8],
				[
					["fixture", 1],
					["fixture", 2],
					["fixture", 3],
					["fixture", 4],
					["live_group", "5"],
				],
			);
			await enter(
				"G3 THRU 5",
				[1, 2, 3, 4, 9, 10, 5, 6, 7, 8],
				[
					["live_group", "3"],
					["live_group", "4"],
					["live_group", "5"],
				],
			);
			await enter(
				"G3 THRU 5 + F6",
				[1, 2, 3, 4, 9, 10, 5, 6, 7, 8],
				[
					["live_group", "3"],
					["live_group", "4"],
					["live_group", "5"],
					["fixture", 6],
				],
			);

			expect((await programmer(api)).selected).toHaveLength(0);
		},
	},
	{
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
	},
	{
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
			await page.getByRole("button", { name: "SET", exact: true }).click();
			await address.click();
			const editor = page.locator(".patch-edit-modal");
			await expect(
				editor.getByRole("heading", { name: "Set fixture address" }),
			).toBeVisible();
			await editor.getByLabel("Fixture address").fill("");
			await editor.getByRole("button", { name: "Set", exact: true }).click();
			await expect(address).toHaveText("Unpatched");
			await expect
				.poll(
					async () =>
						(await object<any>(api, "patched_fixture", fixtures[3])).body
							.universe,
				)
				.toBeNull();

			await openGroups(page);
			await expect(groupCard(page, 5)).toHaveClass(/frozen/);
			await expect(groupCard(page, 5)).not.toContainText("missing");
			await groupCard(page, 5).click();
			await setDimmerByTouch(page, 50);
			await expectGroupNumbers(
				api,
				"5",
				[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
			);
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
	},
	{
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
	},
];
