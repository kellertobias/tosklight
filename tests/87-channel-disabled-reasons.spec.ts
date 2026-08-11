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
		await page.getByRole("button", { name: "Channels", exact: true }).click();

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
