import { test, expect } from "@playwright/test";
import { build } from "esbuild";
import { fileURLToPath } from "node:url";

test("@ui pool layouts switch between automatic squares and a viewport-filling fixed grid without hiding later addresses", async ({
	page,
}, testInfo) => {
	const result = await build({
		entryPoints: [
			fileURLToPath(new URL("./bench/pools/poolTileLayoutGeometry.fixture.tsx", import.meta.url)),
		],
		bundle: true,
		write: false,
		format: "iife",
		jsx: "automatic",
		outfile: testInfo.outputPath("pool-layout.js"),
		define: {
			"process.env.NODE_ENV": '"production"',
			"import.meta.glob": "fixtureGlob",
		},
		banner: { js: "function fixtureGlob() { return {}; }" },
		loader: {
			".woff2": "dataurl",
			".woff": "dataurl",
			".svg": "dataurl",
			".png": "dataurl",
		},
	});
	await page.setViewportSize({ width: 900, height: 650 });
	await page.setContent('<div id="root"></div>');
	for (const file of result.outputFiles) {
		if (file.path.endsWith(".css"))
			await page.addStyleTag({ content: file.text });
		if (file.path.endsWith(".js"))
			await page.addScriptTag({ content: file.text });
	}
	const grid = page.locator(".pool-window-grid");
	const cells = grid.locator("[data-pool-position]");
	await expect(cells).toHaveCount(200);
	const measure = () =>
		grid.evaluate((element) => {
			const viewport = element.closest<HTMLElement>(".ui-window-scroller")!;
			const card = (position: number) =>
				element
					.querySelector(`[data-pool-position="${position}"]`)!
					.getBoundingClientRect();
			const first = card(0),
				fourth = card(3),
				twelfth = card(11),
				later = card(12);
			const bounds = viewport.getBoundingClientRect();
			return {
				first: {
					width: first.width,
					height: first.height,
					left: first.left,
					top: first.top,
				},
				fourth: { right: fourth.right, top: fourth.top },
				twelfth: { bottom: twelfth.bottom },
				later: { top: later.top },
				viewport: {
					width: viewport.clientWidth,
					height: viewport.clientHeight,
					left: bounds.left,
					right: bounds.right,
					top: bounds.top,
					bottom: bounds.bottom,
				},
				columns:
					getComputedStyle(element).gridTemplateColumns.split(" ").length,
			};
		});
	await expect
		.poll(async () => {
			const m = await measure();
			return Math.abs(m.first.width - m.first.height);
		})
		.toBeLessThan(1);
	expect((await measure()).columns).toBeGreaterThan(2); // explicit automatic ignores legacy fixed columns
	await page.getByRole("button", { name: "Fixed 4 by 3" }).click();
	for (const size of [
		{ width: 900, height: 650 },
		{ width: 670, height: 470 },
		{ width: 1100, height: 800 },
	]) {
		await page.setViewportSize(size);
		await expect
			.poll(async () => {
				const m = await measure();
				return Math.abs(m.twelfth.bottom - (m.viewport.bottom - 2));
			})
			.toBeLessThan(1);
		const m = await measure();
		expect(m.columns).toBe(4);
		expect(Math.abs(m.fourth.top - m.first.top)).toBeLessThan(1);
		expect(Math.abs(m.first.left - m.viewport.left - 2)).toBeLessThan(1);
		expect(Math.abs(m.fourth.right - m.viewport.right + 2)).toBeLessThan(1);
		expect(m.later.top).toBeGreaterThanOrEqual(m.viewport.bottom - 1);
		expect(Math.abs(m.first.width - m.first.height)).toBeGreaterThan(10);
	}
	await grid.evaluate((element) => {
		element.closest(".ui-window-scroller")!.scrollTop = 100000;
	});
	await expect(cells.last()).toBeInViewport();
	await grid.evaluate((element) => {
		element.closest(".ui-window-scroller")!.scrollTop = 0;
	});
	await page.getByRole("button", { name: "Automatic squares" }).click();
	await expect
		.poll(async () => {
			const m = await measure();
			return Math.abs(m.first.width - m.first.height);
		})
		.toBeLessThan(1);
	await expect(cells).toHaveCount(200);
});
