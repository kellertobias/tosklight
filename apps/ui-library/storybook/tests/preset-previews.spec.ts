import { expect, test } from "@playwright/test";

const STORY =
	"/iframe.html?id=tosklight-windows-pools--intent-preset-previews&viewMode=story";

test("preset pools preview stored Color and Position intentions", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1200, height: 900 });
	await page.goto(STORY);
	const [colorPool, positionPool] = [
		page.locator(".preset-pool-window.pool-family-color"),
		page.locator(".preset-pool-window.pool-family-position"),
	];
	const colorTile = (number: number) =>
		colorPool.locator(".preset-card").nth(number - 1);
	const positionTile = (number: number) =>
		positionPool.locator(".preset-card").nth(number - 1);

	// One segment per distinct colour, in hue order; a spread shows its sampled colours.
	const swatch = (number: number) =>
		colorTile(number).locator('[data-preset-preview="color"]');
	await expect(swatch(1)).toHaveAttribute("data-preview-colors", "#ff0000");
	await expect(swatch(2)).toHaveAttribute(
		"data-preview-colors",
		"#ff0000 #0000ff",
	);
	await expect(swatch(2)).toHaveAccessibleName("Color preview, 2 colours");
	await expect(swatch(3)).toHaveAttribute(
		"data-preview-colors",
		"#ff0000 #ffff00 #00ff00 #00ffff #0000ff",
	);
	await expect(swatch(5).locator(".preset-preview-segment.uv")).toHaveCount(1);

	// An explicit icon wins, and a preset stored before intentions keeps its plain tile.
	await expect(colorTile(6).locator(".preset-preview")).toHaveCount(0);
	await expect(colorTile(6).locator(".pool-card-media")).toHaveText("★");
	await expect(colorTile(7).locator(".preset-preview")).toHaveCount(0);

	const square = (number: number) =>
		positionTile(number).locator('[data-preset-preview="position"]');
	await expect(square(1).locator("circle")).toHaveCount(5);
	await expect(square(2).locator("circle")).toHaveCount(10);
	await expect(square(2)).toHaveAccessibleName(
		"Pan/Tilt position preview, 10 of 30 aims",
	);
	await expect(square(3)).toHaveAttribute("data-preview-space", "target");
	await expect(square(4).locator("circle")).toHaveCount(5);
	await expect(positionTile(5).locator(".preset-preview")).toHaveCount(0);
	await expect(positionTile(6).locator(".preset-preview")).toHaveCount(0);

	// The preview is a small square inside the tile's media box, never larger than the box.
	const media = await positionTile(2).locator(".pool-card-media").boundingBox();
	const box = await square(2).boundingBox();
	expect(media && box).toBeTruthy();
	if (!media || !box) return;
	expect(Math.abs(box.width - box.height)).toBeLessThanOrEqual(1);
	expect(box.width).toBeLessThanOrEqual(media.width + 0.5);
	expect(box.height).toBeLessThanOrEqual(media.height + 0.5);
});
