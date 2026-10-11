import { test, expect } from "@playwright/test";
import { build } from "esbuild";
import path from "node:path";

test("TL714 fullscreen Screen fills width with compact unobstructed exit, normal chrome remains", async ({
	page,
}, testInfo) => {
	const result = await build({
		entryPoints: [
			path.resolve(
				"tests/bench/playbacks/screenFullscreenGeometry.fixture.tsx",
			),
		],
		bundle: true,
		write: false,
		format: "iife",
		jsx: "automatic",
		outfile: testInfo.outputPath("fullscreen-fixture.js"),
		define: { "process.env.NODE_ENV": '"production"' },
		loader: {
			".woff2": "dataurl",
			".woff": "dataurl",
			".svg": "dataurl",
			".png": "dataurl",
		},
	});
	const configurations = [
		{ fullscreen: true, rail: true, draft: false },
		{ fullscreen: true, rail: true, draft: true },
		{ fullscreen: true, rail: false, draft: false },
		{ fullscreen: false, rail: true, draft: false },
	];
	for (const viewport of [
		{ width: 1728, height: 1084 },
		{ width: 2560, height: 1440 },
	]) {
		for (const configuration of configurations) {
			await page.goto("about:blank");
			await page.setViewportSize(viewport);
			await page.evaluate((configuration) => {
				location.hash = new URLSearchParams({
					fullscreen: String(configuration.fullscreen),
					rail: String(configuration.rail),
					draft: String(configuration.draft),
				}).toString();
			}, configuration);
			await page.setContent('<div id="root"></div>');
			for (const file of result.outputFiles) {
				if (file.path.endsWith(".css"))
					await page.addStyleTag({ content: file.text });
				if (file.path.endsWith(".js"))
					await page.addScriptTag({ content: file.text });
			}
			await expect(page.locator("article")).toHaveCount(20);
			const chrome = page.getByRole("button", {
				name: configuration.fullscreen ? "Exit fullscreen" : "Enter fullscreen",
			});
			await expect(chrome).toBeVisible();
			const geometry = await page.evaluate(() => {
				const box = (selector: string) => {
					const r = document.querySelector(selector)!.getBoundingClientRect();
					return {
						left: r.left,
						right: r.right,
						top: r.top,
						bottom: r.bottom,
						width: r.width,
						height: r.height,
					};
				};
				return {
					section: box(".screen-playbacks"),
					bank: box(".playback-fader-bank"),
					chrome: box(".native-drag-strip"),
					controls: [
						...document.querySelectorAll(
							".playback-fader-bank button, .screen-page-controls button",
						),
					].map((element) => {
						const r = element.getBoundingClientRect();
						return {
							left: r.left,
							right: r.right,
							top: r.top,
							bottom: r.bottom,
						};
					}),
				};
			});
			expect(geometry.section.left).toBe(0);
			expect(geometry.section.width).toBe(viewport.width);
			expect(geometry.bank.width).toBeGreaterThan(
				viewport.width - (configuration.rail ? 130 : 60),
			);
			expect(geometry.bank.top).toBe(
				(configuration.fullscreen ? 6 : 38) + (configuration.draft ? 44 : 0),
			);
			if (configuration.fullscreen) {
				expect(geometry.chrome.width).toBeLessThanOrEqual(44);
				for (const control of geometry.controls) {
					const intersects =
						control.left < geometry.chrome.right &&
						control.right > geometry.chrome.left &&
						control.top < geometry.chrome.bottom &&
						control.bottom > geometry.chrome.top;
					expect(intersects).toBe(false);
				}
			} else {
				expect(
					await page
						.getByRole("button", { name: "Move window" })
						.getAttribute("data-tauri-drag-region"),
				).not.toBeNull();
			}
			await testInfo.attach(
				`fullscreen-${configuration.fullscreen}-rail-${configuration.rail}-draft-${configuration.draft}-${viewport.width}`,
				{
					body: JSON.stringify({ viewport, configuration, geometry }, null, 2),
					contentType: "application/json",
				},
			);
		}
	}
});
