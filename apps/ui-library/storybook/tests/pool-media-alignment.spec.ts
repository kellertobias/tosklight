import { expect, test } from "@playwright/test";

const STORY =
	"/iframe.html?id=tosklight-windows-pool-media-alignment--square-and-configured-tiles&viewMode=story";

test("pool artwork stays vertically and horizontally centered without clipping", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1200, height: 700 });
	await page.goto(STORY);
	await expect(page.locator(".pool-card-media")).toHaveCount(20);
	await page.evaluate(async () => {
		await document.fonts.ready;
		await Promise.all(
			Array.from(document.images).map((image) => image.decode()),
		);
	});
	const measurements = await page
		.locator(".pool-card-media")
		.evaluateAll((elements) =>
			elements.map((media) => {
				const artwork = media.querySelector(
					".pool-card-icon-image, .pool-card-image, .preset-preview-position, .pool-card-icon",
				);
				if (!artwork) throw new Error("Missing tile artwork");
				const outer = media.getBoundingClientRect();
				const inner = artwork.getBoundingClientRect();
				return {
					name: media.closest("button")?.textContent,
					shape: media
						.closest("[data-tile-shape]")
						?.getAttribute("data-tile-shape"),
					selected: media
						.closest("[data-selected]")
						?.getAttribute("data-selected"),
					dx: inner.x + inner.width / 2 - outer.x - outer.width / 2,
					dy: inner.y + inner.height / 2 - outer.y - outer.height / 2,
					left: inner.left - outer.left,
					top: inner.top - outer.top,
					right: outer.right - inner.right,
					bottom: outer.bottom - inner.bottom,
				};
			}),
		);
	for (const measurement of measurements) {
		const context = JSON.stringify(measurement);
		expect(Math.abs(measurement.dx), context).toBeLessThanOrEqual(0.6);
		expect(Math.abs(measurement.dy), context).toBeLessThanOrEqual(0.6);
		for (const edge of ["left", "top", "right", "bottom"] as const)
			expect(measurement[edge], context).toBeGreaterThanOrEqual(0.5);
	}
});
