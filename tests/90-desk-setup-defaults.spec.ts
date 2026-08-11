import { expect, test } from "./bench/core/fixtures";

test.describe("docs/help/10-Desk-Setup/index.md", () => {
	test("TL-165 @ui › Desk Setup defaults save immediately and keep the requested layout", async ({
		api,
		desk,
		page,
	}) => {
		await desk.open(api.baseUrl);
		await openDefaults(page);

		await expect(
			page.getByRole("button", { name: "Pool colors", exact: true }),
		).toBeVisible();
		await expect(
			page.getByRole("button", { name: "Save changes", exact: true }),
		).toHaveCount(0);

		const defaults = page.locator(".defaults-record-update > article");
		await expect(defaults).toHaveCount(2);
		const recordBox = await defaults.nth(0).boundingBox();
		const updateBox = await defaults.nth(1).boundingBox();
		expect(recordBox).not.toBeNull();
		expect(updateBox).not.toBeNull();
		if (!recordBox || !updateBox)
			throw new Error("Defaults cards are not laid out");
		expect(Math.round(updateBox.y - (recordBox.y + recordBox.height))).toBe(8);

		await page.getByRole("button", { name: "Playback", exact: true }).click();
		const startDefault = page.getByRole("switch", {
			name: "Start after first recording",
		});
		const original = await startDefault.isChecked();
		await startDefault.locator("..").locator(".ui-switch-track").click();
		await expect
			.poll(
				async () => (await deskConfiguration(api)).start_after_first_recording,
			)
			.toBe(!original);

		await page.reload();
		await openDefaults(page);
		await page.getByRole("button", { name: "Playback", exact: true }).click();
		await expect(
			page.getByRole("switch", { name: "Start after first recording" }),
		).toBeChecked({ checked: !original });

		const restored = page.getByRole("switch", {
			name: "Start after first recording",
		});
		await restored.locator("..").locator(".ui-switch-track").click();
		await expect
			.poll(
				async () => (await deskConfiguration(api)).start_after_first_recording,
			)
			.toBe(original);
	});
});

async function openDefaults(page: import("@playwright/test").Page) {
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page
		.locator(".show-modal")
		.getByRole("button", { name: "Enter Setup", exact: true })
		.click();
	await page
		.locator(".setup-window nav")
		.getByRole("button", { name: "Defaults", exact: true })
		.click();
}

async function deskConfiguration(api: {
	request<T>(method: string, path: string): Promise<T>;
}) {
	return (
		await api.request<{
			configuration: { start_after_first_recording: boolean };
		}>("GET", "/api/v2/configuration")
	).configuration;
}
