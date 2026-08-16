import { expect, test } from "./bench/core/fixtures";

test.describe("docs/help/10-Desk-Setup/index.md", () => {
	test("TL-165 @ui › Desk Setup defaults save immediately and keep the requested layout", async ({
		api,
		desk,
		page,
	}) => {
		await desk.open(api.baseUrl);
		await page.setViewportSize({ width: 1280, height: 720 });
		await openDefaults(page);

		await expect(
			page.getByRole("tab", { name: "Pool colors", exact: true }),
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
		expect(Math.round(updateBox.y)).toBe(Math.round(recordBox.y));
		expect(Math.round(updateBox.x - (recordBox.x + recordBox.width))).toBe(8);
		await expectNoSetupContentOverflow(page);

		await page.getByRole("tab", { name: "Pool colors", exact: true }).click();
		await expect(
			page.getByRole("heading", { name: "Pool color defaults", exact: true }),
		).toBeVisible();
		await expect(
			page.getByRole("heading", { name: "Defaults", exact: true }),
		).toHaveCount(0);
		expect(
			await page
				.locator(".pool-color-defaults-grid")
				.evaluate(
					(node) =>
						getComputedStyle(node).gridTemplateColumns.split(" ").length,
				),
		).toBe(3);
		await expectNoSetupContentOverflow(page);

		const setupNavigation = page.locator(".setup-window nav");
		await setupNavigation
			.getByRole("button", { name: "Highlight", exact: true })
			.click();
		expect(
			await page
				.locator(".highlight-look-grid")
				.evaluate(
					(node) =>
						getComputedStyle(node).gridTemplateColumns.split(" ").length,
				),
		).toBeGreaterThan(1);
		await expectNoSetupContentOverflow(page);

		await setupNavigation
			.getByRole("button", { name: "Network & Inputs", exact: true })
			.click();
		await page.getByRole("tab", { name: "Sound", exact: true }).click();
		const soundButtons = page.locator(".sound-input-actions > button");
		await expect(soundButtons).toHaveCount(2);
		const microphoneBox = await soundButtons.nth(0).boundingBox();
		const refreshBox = await soundButtons.nth(1).boundingBox();
		expect(microphoneBox).not.toBeNull();
		expect(refreshBox).not.toBeNull();
		expect(
			Math.round(
				(refreshBox?.x ?? 0) -
					((microphoneBox?.x ?? 0) + (microphoneBox?.width ?? 0)),
			),
		).toBeGreaterThanOrEqual(8);

		await setupNavigation
			.getByRole("button", { name: "Defaults", exact: true })
			.click();

		await page.getByRole("tab", { name: "Playback", exact: true }).click();
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
		await page.getByRole("tab", { name: "Playback", exact: true }).click();
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

	test("TL-251 @ui › setup navigation exposes a finger-sized scrollbar when touch scrolling is enabled", async ({
		api,
		desk,
		page,
	}) => {
		await desk.open(api.baseUrl);
		await page.setViewportSize({ width: 900, height: 420 });
		await page.evaluate(() =>
			document.documentElement.classList.add("touch-scrollbars"),
		);
		await openDefaults(page);

		const navigationScroll = page.locator(".setup-navigation-scroll");
		await expect(navigationScroll).toHaveClass(/overflowing/);
		const track = navigationScroll.locator(".ui-touch-scrollbar");
		const thumb = track.getByRole("button", { name: "Scroll window" });
		await expect(track).toBeVisible();
		await expect(thumb).toBeVisible();
		const trackBox = await track.boundingBox();
		const thumbBox = await thumb.boundingBox();
		expect(trackBox?.width ?? 0).toBeGreaterThanOrEqual(22);
		expect(thumbBox?.height ?? 0).toBeGreaterThanOrEqual(48);
	});
});

async function expectNoSetupContentOverflow(
	page: import("@playwright/test").Page,
) {
	await expect
		.poll(() =>
			page
				.locator(".setup-content-scroll > .ui-window-scroller")
				.evaluate((node) => node.scrollHeight - node.clientHeight),
		)
		.toBeLessThanOrEqual(1);
}

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
