import type { Locator } from "@playwright/test";
import { expect, test } from "./bench/core/fixtures";

test.use({ viewport: { width: 1200, height: 900 } });

/** The grid's resolved column tracks and the content width they have to fill. */
async function tracks(grid: Locator) {
	return grid.evaluate((node) => {
		const style = getComputedStyle(node);
		const columns = style.gridTemplateColumns
			.split(" ")
			.map((track) => Number.parseFloat(track))
			.filter((track) => Number.isFinite(track));
		const gap = Number.parseFloat(style.columnGap) || 0;
		const padding =
			(Number.parseFloat(style.paddingLeft) || 0) +
			(Number.parseFloat(style.paddingRight) || 0);
		return {
			columns,
			covered:
				columns.reduce((sum, track) => sum + track, 0) +
				gap * Math.max(0, columns.length - 1),
			available: node.clientWidth - padding,
		};
	});
}

/**
 * The preset pool is drawn by the shared pool grid, whose tracks divide the room they are given.
 * A stylesheet override used to cap a preset track at a fixed 120 px and pack the tracks to the
 * left, so the pool left a ragged gap and kept the same geometry however large its pane became.
 */
test.describe("the preset pool scales with its pane", () => {
	test("PRESET-GRID-001 @ui › the preset grid fills its pane at any width", async ({
		api,
		desk,
		page,
	}) => {
		await desk.open(api.baseUrl);

		const presetPane = page
			.locator(".desk-pane")
			.filter({ has: page.locator(".preset-pool-window") });
		await expect(presetPane).toBeVisible();
		const grid = presetPane.locator(".ui-button-grid.card-pool").first();
		await expect(grid).toBeVisible();
		await expect(grid.locator("> *").first()).toBeVisible();

		for (const width of [1200, 1800, 1400]) {
			await page.setViewportSize({ width, height: 900 });
			// The grid measures itself through a ResizeObserver, so let the tracks settle.
			await expect
				.poll(async () => {
					const { covered, available } = await tracks(grid);
					return Math.round(available - covered);
				}, { message: `preset track coverage at a ${width} px viewport` })
				.toBeLessThanOrEqual(1);

			const { columns, available } = await tracks(grid);
			expect(columns.length).toBeGreaterThan(0);
			expect(available).toBeGreaterThan(0);
		}
	});
});
