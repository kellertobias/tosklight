import type { ApiDriver } from "../../../apps/control-ui/e2e/bench/api";
import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import type { Page } from "../../../apps/control-ui/node_modules/@playwright/test/index.js";
import { expectSelectedNumbers } from "./apiState";
import { executeProgrammerCommand, storeGroup } from "../operator";

export async function pressCommand(
	page: Page,
	value: string,
	visibleValue = value,
): Promise<void> {
	await executeProgrammerCommand(
		{ via: "software", page },
		value,
		{
			expectedCommandLine: visibleValue,
			expectedCompletion: /^(FIXTURE|GROUP)$/,
		},
	);
}

export async function pressCommandAndWait(
	page: Page,
	value: string,
	visibleValue = value,
): Promise<void> {
	await pressCommand(page, value, visibleValue);
	await expect(page.getByLabel("Command line")).toHaveValue(
		/^(FIXTURE|GROUP)$/,
	);
}

export async function enterCommandWithoutEscape(
	page: Page,
	value: string,
	visibleValue = value,
): Promise<void> {
	await executeProgrammerCommand(
		{ via: "software", page },
		value,
		{
			reset: false,
			expectedCommandLine: visibleValue,
			expectedCompletion: "FIXTURE",
		},
	);
}

export async function openBuiltIn(page: Page, name: string): Promise<void> {
	const entry = page.locator(".dock-entry").filter({ hasText: name }).first();
	if (!(await entry.isVisible()))
		await page.getByRole("button", { name: "BUILT-INS", exact: true }).click();
	await expect(entry).toBeVisible();
	await entry.click();
}

export async function openGroups(page: Page): Promise<void> {
	if (!(await page.locator(".group-pool-window").isVisible())) {
		await page.getByRole("button", { name: "SHIFT", exact: true }).click();
		await page.getByRole("button", { name: "1", exact: true }).click();
	}
	await expect(page.locator(".group-pool-window")).toBeVisible();
}

export async function openFixtures(page: Page): Promise<void> {
	await openBuiltIn(page, "Fixtures");
	await expect(page.locator(".fixture-window")).toBeVisible();
}

export async function openPatch(page: Page): Promise<void> {
	if (await page.locator(".patch-table").isVisible()) return;
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Show Patch", exact: true }).click();
	await expect(page.locator(".patch-table")).toBeVisible();
}

export function patchFixtureRow(page: Page, number: number) {
	return page
		.locator(".patch-table tbody tr")
		.filter({
			has: page
				.locator("td:nth-child(2)")
				.filter({ hasText: new RegExp(`^${number}$`) }),
		})
		.first();
}

export function groupCard(page: Page, number: number) {
	return page.locator(".group-pool-window .group-card").nth(number - 1);
}

export async function recordExistingGroup(
	page: Page,
	number: number,
	mode: "Merge" | "Overwrite",
): Promise<void> {
	await storeGroup({ via: "pool", page, group: number, mode });
}

export async function expectVisibleGroupOrder(
	page: Page,
	number: number,
	fixtures: number[],
): Promise<void> {
	await openGroups(page);
	const card = groupCard(page, number);
	const box = await card.boundingBox();
	expect(box).toBeTruthy();
	await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
	await page.mouse.down();
	await page.waitForTimeout(700);
	await page.mouse.up();
	const order = page.locator(".group-context-menu .group-order");
	await expect(order).toBeVisible();
	for (const [index, fixture] of fixtures.entries())
		await expect(order).toContainText(`${index + 1}. Fixture ${fixture}`);
	await page
		.locator(".group-context-menu")
		.getByRole("button", { name: "Cancel", exact: true })
		.click();
}

export function fixtureRow(page: Page, number: number) {
	return page
		.locator(".fixture-window .ui-data-table-row:not(.header)")
		.filter({
			has: page.getByRole("cell", { name: String(number), exact: true }),
		})
		.first();
}

export async function selectFixtureRows(
	api: ApiDriver,
	page: Page,
	fixtures: number[],
): Promise<void> {
	await openFixtures(page);
	for (const [index, fixture] of fixtures.entries()) {
		await fixtureRow(page, fixture).click();
		await expectSelectedNumbers(api, fixtures.slice(0, index + 1));
	}
}

export function stageFixture(page: Page, fixtureId: string) {
	return page.locator(`.stage-fixture[data-fixture-id="${fixtureId}"]`);
}

export async function setDimmerByTouch(
	page: Page,
	value: number,
): Promise<void> {
	const encoder = page
		.locator(".vertical-touch-fader-stack")
		.filter({ hasText: "Enc 1 · Dimmer" });
	await encoder.getByRole("button", { name: "Set value" }).click();
	const dialog = page.getByRole("dialog", { name: "Enc 1 · Dimmer value" });
	await expect(dialog).toBeVisible();
	await page.keyboard.type(String(value));
	await page.keyboard.press("Enter");
	await expect(dialog).toBeHidden();
}
