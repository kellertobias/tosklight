import type { ApiDriver } from "../../../apps/control-ui/e2e/bench/api";
import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import type { Page } from "../../../apps/control-ui/node_modules/@playwright/test/index.js";
import type { FoundationalCase } from "./case";
import {
	command,
	expectSelectedNumbers,
	fixtureIdsByNumber,
	loadCompactRig,
	programmer,
	select,
	setGroupByNumbers,
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

export const commandVisibleUi: FoundationalCase = {
	title:
		"CMD-001 @supplemental › exhaustive visible prefix, geometry, range, Clear, and Escape cases",
	run: async ({ api, bench, desk, page }) => {
		await prepareCommandUi(api, bench, desk, page);
		await exerciseGroupCommandMode(api, page);
		await exerciseFixtureCommandMode(api, page);
	},
};

export const commandApiBoundaries: FoundationalCase = {
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
			await api.setCommandLineText(value);
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
		await api.setCompatibilityCommandTarget("GROUP");
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
		await api.setCompatibilityCommandTarget("FIXTURE");
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
};
