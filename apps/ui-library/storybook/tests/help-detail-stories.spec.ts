import { expect, test } from "@playwright/test";
async function open(page: import("@playwright/test").Page, story: string) {
	await page.setViewportSize({ width: 1000, height: 420 });
	await page.goto(
		`/iframe.html?id=tosklight-windows-tile-details--${story}&viewMode=story`,
	);
	await expect(page.locator("[data-documentation-shot]")).toHaveAttribute(
		"data-documentation-ready",
		"true",
	);
	await page.evaluate(() => document.fonts.ready);
}
test("Group detail photographs show real unselected, partial, complete and source-state markers", async ({
	page,
}) => {
	await open(page, "group-counts");
	const cards = page.locator("[data-help-capture] .pool-card");
	await expect(cards.nth(0).locator(".group-fixture-count")).toHaveText("4");
	await expect(
		cards.nth(0).locator(".group-fixture-count-selected"),
	).toHaveCount(0);
	await expect(cards.nth(1).locator(".group-fixture-count")).toHaveText("2/4");
	await expect(
		cards.nth(1).locator(".group-fixture-count-selected"),
	).toHaveText("2");
	await expect(cards.nth(2).locator(".group-fixture-count")).toHaveText("2");
	await expect(
		cards.nth(2).locator(".group-fixture-count-selected"),
	).toHaveText("2");
	const image = await page.locator("[data-help-capture]").screenshot();
	expect(image.readUInt32BE(16)).toBe(660);
	expect(image.readUInt32BE(20)).toBe(252);
	await open(page, "group-markers");
	await expect(
		page.locator(".pool-card-state-marker.derived").first(),
	).toBeVisible();
	await expect(
		page.locator(".pool-card-state-marker.frozen").first(),
	).toBeVisible();
});
test("Preset details show effective fixture counts and actual Color and Position preview graphics", async ({
	page,
}) => {
	await open(page, "color-preview-counts");
	const cards = page.locator("[data-help-capture] .pool-card");
	await expect(
		cards.nth(0).locator(".pool-card-information > small"),
	).toHaveText("2");
	await expect(
		cards.nth(1).locator(".pool-card-information > small"),
	).toHaveText("0");
	await expect(
		cards.nth(2).locator(".pool-card-information > small"),
	).toHaveText("3");
	await expect(cards.nth(1).locator(".pool-card-preview")).toBeVisible();
	await expect(cards.nth(2).locator(".pool-card-preview")).toBeVisible();
	await expect(cards.nth(3).locator(".pool-card-icon")).toBeVisible();
	await open(page, "position-previews");
	await expect(
		cards.nth(0).locator(".pool-card-information > small"),
	).toHaveText("2");
	await expect(
		cards.nth(1).locator(".pool-card-information > small"),
	).toHaveText("0");
	await expect(
		cards.nth(2).locator(".pool-card-information > small"),
	).toHaveText("3");
	await expect(
		cards.nth(0).locator(".pool-card-preview svg circle").first(),
	).toBeVisible();
});
test("Playback detail uses production cue inserts, single buttons and measured two-tile Solo outline", async ({
	page,
}) => {
	await open(page, "playback-cue-and-solo");
	const tiles = page.locator('[data-ui-component="playback-tile"]');
	await expect(tiles).toHaveCount(3);
	for (let i = 0; i < 3; i++) {
		await expect(tiles.nth(i).getByRole("button")).toHaveCount(1);
		await expect(tiles.nth(i).locator(".playback-tile-summary")).toContainText(
			`${i + 1} · ${["Warm Welcome", "Deep Blue", "Audience Sweep"][i]}`,
		);
	}
	await expect(tiles.nth(1).locator(".playback-tile-action-label")).toHaveText(
		"SOLO · TOGGLE",
	);
	const fence = page.locator(
		'svg.playback-solo-fence-overlay path[data-solo-region="looks"]',
	);
	await expect(fence).toBeVisible();
	await expect(fence).toHaveAttribute("stroke", "#22c55e");
	const measured = await fence.evaluate((element) => {
		const bounds = (element as SVGGraphicsElement).getBBox();
		return { x: bounds.x, width: bounds.width };
	});
	const first = await tiles.nth(0).boundingBox(),
		second = await tiles.nth(1).boundingBox(),
		third = await tiles.nth(2).boundingBox();
	expect(measured.x).toBeLessThan(3);
	expect(measured.width).toBeGreaterThan(first!.width + second!.width);
	expect(measured.width).toBeLessThan(third!.x - first!.x);
	await open(page, "playback-shallow");
	await expect(tiles.nth(0).locator(".playback-tile-cue-label")).toHaveText(
		"1 · Warm Welcome",
	);
	await expect(tiles.nth(0).locator(".playback-tile-cue-label")).toBeVisible();
	const footer = tiles.nth(0).locator(".playback-tile-footer");
	const action = await footer
		.locator(".playback-tile-action-label")
		.boundingBox();
	const cue = await footer.locator(".playback-tile-summary").boundingBox();
	expect(Math.abs(action!.y - cue!.y)).toBeLessThan(8);
});
