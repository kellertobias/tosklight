import type { Page } from "@playwright/test";
import { expect, test } from "./bench/core/fixtures";

/**
 * Selecting a run of fixtures is one gesture, not one click per lantern: shift takes everything
 * between the row an operator last touched and the row they are clicking now.
 *
 * The desk already does this. What it did not have was anything holding it to it, which is what
 * this is: the behaviour is easy to lose in a refactor of the sheet's click handling and nothing
 * would have said so.
 */
test("TL-400 @ui › shift takes the whole run between two fixture-sheet rows", async ({
	bench,
	desk,
	page,
}) => {
	await desk.open(bench.baseUrl);
	await openFixtureSheet(page);

	const rows = page.locator(".ui-data-table [data-table-index]");
	await expect(rows.first()).toBeVisible();
	expect(
		await rows.count(),
		"the sheet is showing enough rows to select a run across",
	).toBeGreaterThanOrEqual(12);
	const selected = page.locator(".ui-data-table-row.selected");

	await rows.nth(0).click();
	await expect(selected).toHaveCount(1);

	// Seven rows, not two: the range is everything between, not just the two ends.
	await rows.nth(6).click({ modifiers: ["Shift"] });
	await expect(selected).toHaveCount(7);

	// The row just clicked becomes the new anchor, so a second shift measures from there and
	// reads the same run backwards.
	await rows.nth(2).click({ modifiers: ["Shift"] });
	await expect(selected).toHaveCount(5);

	// A plain click is still a plain click: it replaces rather than extending.
	await rows.nth(9).click();
	await expect(selected).toHaveCount(1);

	await rows.nth(11).click({ modifiers: ["Shift"] });
	await expect(selected).toHaveCount(3);
});

/** The Open Window catalog groups its cards into tabs, so the bench searches the tabs. */
async function openFixtureSheet(page: Page): Promise<void> {
	const dockMode = page.getByRole("button", {
		name: "Desktops / Built-ins",
		exact: true,
	});
	if ((await dockMode.getAttribute("data-dock-mode")) !== "desks") {
		await dockMode.click();
	}
	await page.getByRole("button", { name: /New desktop/ }).click();
	await page.locator(".empty-desk").click({ position: { x: 10, y: 10 } });
	const dialog = page.getByRole("dialog", { name: "Open Window" });
	const card = dialog
		.getByRole("button")
		.filter({ has: page.getByText("Fixture sheet", { exact: true }) });
	for (const tab of await dialog.getByRole("tab").all()) {
		await tab.click();
		if (await card.count()) {
			await card.first().click();
			await page.waitForTimeout(1500);
			return;
		}
	}
	throw new Error("The Open Window catalog offers no Fixture sheet");
}
