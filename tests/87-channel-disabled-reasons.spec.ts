import type { Page } from "@playwright/test";
import { expect, test } from "./bench/core/fixtures";

test.describe("Channels disabled-state explanations", () => {
	test("TL-166 @ui › empty faders explain their state while available faders stay clear", async ({
		bench,
		desk,
		page,
	}) => {
		await desk.open(bench.baseUrl);
		const dockMode = page.getByRole("button", {
			name: "Desktops / Built-ins",
			exact: true,
		});
		if ((await dockMode.getAttribute("data-dock-mode")) !== "desks") {
			await dockMode.click();
		}
		await page.getByRole("button", { name: /New desktop/ }).click();
		await page.locator(".empty-desk").click({ position: { x: 10, y: 10 } });
		await openCatalogCard(page, "Channels");

		const channels = page.locator(".channels-window");
		await expect(channels).toBeVisible();
		const populated = channels.locator(".channel-fader:not(.empty)").first();
		await expect(populated.locator('input[type="range"]')).toBeEnabled();
		await expect(populated).toContainText("Intensity");
		await expect(populated).not.toContainText(/loading|unavailable|inactive/iu);

		const empty = channels.locator(".channel-fader.empty").first();
		await expect(empty.locator('input[type="range"]')).toBeDisabled();
		await expect(empty).toContainText("Empty position");
	});
});

/** The Open Window catalog groups its cards into tabs, so the bench searches the tabs. */
async function openCatalogCard(page: Page, title: string): Promise<void> {
	const dialog = page.getByRole("dialog", { name: "Open Window" });
	const card = dialog
		.getByRole("button")
		.filter({ has: page.getByText(title, { exact: true }) });
	for (const tab of await dialog.getByRole("tab").all()) {
		await tab.click();
		if (await card.count()) {
			await card.first().click();
			return;
		}
	}
	throw new Error(`The Open Window catalog offers no "${title}" window`);
}
