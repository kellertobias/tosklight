import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { build } from "esbuild";

test("Playback tile modes retain authored sparse addresses, scroll and Solo outlines @ui", async ({
	page,
}, testInfo) => {
	const bundle = await build({
		entryPoints: [
			fileURLToPath(
				new URL(
					"./bench/playbacks/playbackTileModes.fixture.tsx",
					import.meta.url,
				),
			),
		],
		bundle: true,
		write: false,
		outfile: testInfo.outputPath("fixture.js"),
		platform: "browser",
		jsx: "automatic",
		logLevel: "silent",
		loader: {
			".woff2": "dataurl",
			".woff": "dataurl",
			".svg": "dataurl",
			".png": "dataurl",
		},
	});
	await page.setViewportSize({ width: 1100, height: 600 });
	await page.setContent('<style>body{margin:20px}</style><div id="app"></div>');
	for (const file of bundle.outputFiles) {
		if (file.path.endsWith(".css"))
			await page.addStyleTag({ content: file.text });
		if (file.path.endsWith(".js"))
			await page.addScriptTag({ content: file.text });
	}
	const read = () =>
		page.evaluate(() => {
			const viewport = document.querySelector(
				".playback-bank-viewport,.playback-bank-fixed",
			);
			const bank = document.querySelector(".playback-fader-bank");
			if (!viewport || !bank) throw new Error("Playback bank not rendered");
			const cards = [
				...document.querySelectorAll<HTMLElement>("[data-playback-slot]"),
			];
			const last = cards.at(-1);
			if (!last) throw new Error("Playback cells not rendered");
			const box = cards[0].getBoundingClientRect();
			const vb = viewport.getBoundingClientRect();
			return {
				rowGap: Number.parseFloat(getComputedStyle(bank).rowGap),
				square: Math.abs(box.width - box.height) < 1,
				sameHeights: cards.every(
					(card) =>
						Math.abs(card.getBoundingClientRect().height - box.height) < 1,
				),
				inert: document
					.querySelector(".playback-fader-bank")
					?.hasAttribute("inert"),
				columns: new Set(cards.map((card) => card.getBoundingClientRect().left))
					.size,
				rowHeight: box.height,
				viewportHeight: viewport.clientHeight,
				scrolls: viewport.scrollHeight > viewport.clientHeight + 1,
				addresses: cards.map((card) => Number(card.dataset.playbackSlot)),
				lastVisible:
					last.getBoundingClientRect().bottom <= vb.bottom + 1 &&
					last.getBoundingClientRect().top >= vb.top - 1,
				actions: (window as unknown as { actions: number }).actions,
			};
		});
	await expect.poll(async () => (await read()).scrolls).toBe(false);
	expect((await read()).columns).toBe(6);
	await page.screenshot({ path: testInfo.outputPath("legacy-fit.png") });
	for (const mode of ["fixed", "automatic"]) {
		await page.evaluate(
			(mode) =>
				(window as unknown as { setTileMode(mode: string): void }).setTileMode(
					mode,
				),
			mode,
		);
		await expect.poll(async () => (await read()).scrolls).toBe(true);
		if (mode === "fixed") {
			const actual = await read();
			expect(actual.columns).toBe(6);
			expect(actual.rowHeight * 3 + actual.rowGap * 2).toBeCloseTo(
				actual.viewportHeight,
				1,
			);
		} else await expect.poll(async () => (await read()).square).toBe(true);
		await expect
			.poll(() => page.locator(".playback-solo-fence-overlay path").count())
			.toBeGreaterThan(0);
		expect((await read()).addresses).toEqual([
			...Array.from({ length: 28 }, (_, i) => 401 + i),
			800,
			900,
		]);
		await page.screenshot({ path: testInfo.outputPath(`${mode}.png`) });
		await page.locator(".playback-bank-viewport").hover();
		await page.mouse.wheel(0, 10000);
		await expect.poll(async () => (await read()).lastVisible).toBe(true);
		expect((await read()).actions).toBe(0);
		expect((await read()).sameHeights).toBe(true);
		expect((await read()).inert).toBe(false);
		await page.locator('[data-playback-slot="900"] button').click();
		expect((await read()).actions).toBe(1);
		await page.evaluate(() => {
			(window as unknown as { actions: number }).actions = 0;
			document.querySelector(".playback-bank-viewport")?.scrollTo(0, 0);
		});
	}
});
