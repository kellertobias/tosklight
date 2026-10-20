import { expect, test } from "@playwright/test";
async function open(
	page: import("@playwright/test").Page,
	id: string,
	viewport = { width: 1496, height: 761 },
) {
	await page.setViewportSize(viewport);
	await page.goto(`/iframe.html?id=${id}&viewMode=story`);
	await expect(page.locator("[data-documentation-shot]")).toHaveAttribute(
		"data-documentation-ready",
		"true",
	);
	await page.evaluate(() => document.fonts.ready);
}
test("Channels includes both complete rows inside the documentation viewport", async ({
	page,
}) => {
	await open(page, "tosklight-windows-channels--two-row-bank");
	const bank = page.locator(".channels-window");
	const box = await bank.boundingBox();
	expect(box!.y + box!.height).toBeLessThanOrEqual(761);
	const house = bank.locator(".channel-fader").filter({ hasText: "House" });
	await expect(house).toContainText("100%");
	const bounds = await house.boundingBox();
	expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(761);
});
test("Cuelist pool captures contain readable populated examples and real assignment details", async ({
	page,
}) => {
	for (const id of ["pool-help", "pool-compact-help"]) {
		await open(page, `tosklight-windows-cuelists-and-cues--${id}`, {
			width: 1000,
			height: 420,
		});
		const subject = page.locator('[data-help-capture="cuelist-pool"]');
		const cards = subject.locator(".cuelist-card:not(.empty)");
		await expect(cards).toHaveCount(3);
		await expect(cards.nth(0)).toContainText("Main Sequence");
		await expect(cards.nth(1)).toContainText("Color Chase");
		await expect(cards.nth(2)).toContainText("Audience Sweep");
		for (let i = 0; i < 3; i++)
			expect((await cards.nth(i).boundingBox())!.width).toBeGreaterThan(200);
		await expect(cards.nth(0)).toContainText("Playback 1");
	}
});
test("Dynamics capture subject contains the production editor without its unrelated shell", async ({
	page,
}) => {
	await open(page, "tosklight-windows-dynamics--full-application-discussion", {
		width: 1496,
		height: 1100,
	});
	const editor = page.locator("section.dynamic-full-discussion-editor");
	await expect(editor).toBeVisible();
	await expect(editor).toContainText("Ocean Sweep");
	await expect(editor).toContainText("Intensity");
	await expect(editor).toContainText("Tilt");
	await expect(editor.locator(".left-dock,.command-section")).toHaveCount(0);
	const box = await editor.boundingBox();
	expect(box!.y + box!.height).toBeLessThanOrEqual(1100);
});
test("Fixture Sheet help framing contains the selected rows rather than a page of empty rows", async ({
	page,
}) => {
	await open(page, "tosklight-windows-fixture-sheet--selected-steps-help", {
		width: 1496,
		height: 380,
	});
	const window = page.locator(".fixture-window");
	await expect(window).toContainText("Front Profile SL");
	await expect(window).toContainText("Back Wash");
	expect((await window.boundingBox())!.height).toBe(300);
});
test("Setup reference states have a loaded show and resolved extension status", async ({
	page,
}) => {
	await open(page, "tosklight-windows-setup--network-and-inputs-help");
	await expect(page.locator(".native-extension-status")).toContainText(
		"0 packages · 0 running instances",
	);
	await expect(
		page.getByText("Loading extension status…", { exact: true }),
	).toHaveCount(0);
	await open(page, "tosklight-windows-setup--shows-and-recovery-help");
	await expect(page.getByText("Opening Night", { exact: true })).toBeVisible();
	await expect(
		page.getByText("Connected, autosave active", { exact: true }),
	).toBeVisible();
});
