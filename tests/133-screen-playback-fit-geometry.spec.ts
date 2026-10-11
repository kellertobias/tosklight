import { test, expect } from "@playwright/test";
import { build } from "esbuild";
import path from "node:path";
import { writeFile } from "node:fs/promises";

test("TL714 Screen pane fits 15 faders with three actions and 15 buttons, recovers after enlargement", async ({ page }, testInfo) => {
	const result = await build({
		entryPoints: [path.resolve("tests/bench/playbacks/screenPlaybackFitGeometry.fixture.tsx")],
		bundle: true, write: false, format: "iife", jsx: "automatic",
		outfile: testInfo.outputPath("screen-fit-fixture.js"),
		define: { "process.env.NODE_ENV": '"production"' },
		loader: { ".woff2": "dataurl", ".woff": "dataurl", ".svg": "dataurl", ".png": "dataurl" },
	});
	await page.setViewportSize({ width: 900, height: 500 });
	await page.setContent('<div id="root"></div>');
	for (const file of result.outputFiles) {
		if (file.path.endsWith(".css")) await page.addStyleTag({ content: file.text });
		if (file.path.endsWith(".js")) await page.addScriptTag({ content: file.text });
	}
	const bank = page.locator(".playback-fader-bank");
	await expect(bank.locator("article")).toHaveCount(30);

	await expect(page.getByRole("status")).toHaveCount(0);
	const geometry = await bank.evaluate((element) => {
		const bounds = element.getBoundingClientRect();
		const cards = [...element.querySelectorAll("article")];
		const targets = [...element.querySelectorAll<HTMLElement>(
			".vertical-touch-fader, .vertical-touch-fader-actions button, .single-button-playback-action",
		)];
		return {
			width: bounds.width, height: bounds.height,
			scrollWidth: element.scrollWidth, scrollHeight: element.scrollHeight,
			inert: element instanceof HTMLElement && element.inert,
			positions: cards.map((card) => Number(card.getAttribute("data-playback-slot"))),
			controls: targets.map((target) => {
				const box = target.getBoundingClientRect();
				const card = target.closest("article")!.getBoundingClientRect();
				return { width: box.width, height: box.height,
					insideCard: box.left >= card.left - .5 && box.right <= card.right + .5 &&
						box.top >= card.top - .5 && box.bottom <= card.bottom + .5,
					insideBank: box.left >= bounds.left - .5 && box.right <= bounds.right + .5 &&
						box.top >= bounds.top - .5 && box.bottom <= bounds.bottom + .5 };
			}),
		};
	});
	expect(geometry.positions).toEqual([
		...Array.from({ length: 15 }, (_, i) => i + 1),
		...Array.from({ length: 15 }, (_, i) => i + 101),
	]);
	expect(geometry.controls).toHaveLength(75);
	for (const control of geometry.controls) {
		expect(control.width).toBeGreaterThanOrEqual(44);
		expect(control.height).toBeGreaterThanOrEqual(44);
		expect(control.insideCard).toBe(true);
		expect(control.insideBank).toBe(true);
	}
	expect(geometry.scrollWidth).toBeLessThanOrEqual(Math.ceil(geometry.width));
	expect(geometry.scrollHeight).toBeLessThanOrEqual(Math.ceil(geometry.height));
	expect(geometry.inert).toBe(false);
	await page.setViewportSize({ width: 500, height: 500 });
	await expect(page.getByRole("status")).toContainText("more space for 44px touch controls");
	await expect.poll(() => bank.evaluate((element) => element instanceof HTMLElement && element.inert)).toBe(true);
	await page.setViewportSize({ width: 900, height: 500 });
	await expect(page.getByRole("status")).toHaveCount(0);
	await expect.poll(() => bank.evaluate((element) => element instanceof HTMLElement && element.inert)).toBe(false);
	const geometryPath = testInfo.outputPath("screen-pane-actual-card-geometry.json");
	await writeFile(geometryPath, JSON.stringify(geometry, null, 2));
	await testInfo.attach("screen-pane-actual-card-geometry", {
		path: geometryPath, contentType: "application/json",
	});
});
