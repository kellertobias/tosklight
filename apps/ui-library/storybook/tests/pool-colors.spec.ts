import { expect, test } from "@playwright/test";

const STORY =
	"/iframe.html?id=tables-and-grids-pools-production-pool-cards--consistent-object-type-colors&viewMode=story";
const EVERY_STATE_STORY =
	"/iframe.html?id=tables-and-grids-pools-production-pool-cards--scaling-and-every-state&viewMode=story";

test("pool colors preserve defaults, modes, contrast, and non-color states", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1200, height: 850 });
	await page.goto(STORY);
	await page.evaluate(() => document.fonts.ready);

	const cards = page.locator(".pool-card.pool-presentation");
	await expect(cards).toHaveCount(15);
	await expect(cards.nth(0)).toHaveCSS("--pool-card-color", "#d8ad55");
	await expect(cards.nth(1)).toHaveCSS("--pool-card-color", "#8f3541");
	await expect(cards.nth(2)).toHaveCSS("--pool-card-color", "#3bbdce");
	await expect(cards.nth(3)).toHaveCSS("--pool-card-color", "#93cc55");
	await expect(cards.nth(4)).toHaveCSS("--pool-card-color", "#93cc55");
	for (const index of [5, 6, 7, 8, 9]) {
		await expect(cards.nth(index)).toHaveCSS("--pool-card-color", "#89939e");
	}
	await expect(cards.nth(10)).toHaveCSS("--pool-card-color", "#6857d8");
	await expect(cards.nth(11)).toHaveCSS("--pool-card-color", "#89939e");

	await expect(cards.nth(0)).toHaveClass(/selected/u);
	await expect(cards.nth(10)).toHaveClass(/focused/u);
	await expect(cards.nth(12)).toHaveClass(/record-target/u);
	await expect(cards.nth(13)).toHaveClass(/update-target/u);
	await expect(cards.nth(14)).toHaveClass(/empty/u);
	await expect(cards.nth(14)).not.toHaveClass(/disabled/u);

	await cards.nth(11).focus();
	await expect(cards.nth(11)).toHaveCSS("outline-style", "solid");

	const contrastRatios = await cards.evaluateAll((elements) => {
		const parseRgb = (value: string): [number, number, number] => {
			const channels = value
				.match(/\d+(?:\.\d+)?/gu)
				?.slice(0, 3)
				.map(Number);
			if (!channels || channels.length !== 3)
				throw new Error(`Expected an RGB color, received ${value}`);
			return channels as [number, number, number];
		};
		const luminance = (channels: [number, number, number]) =>
			channels
				.map((channel) => {
					const value = channel / 255;
					return value <= 0.04045
						? value / 12.92
						: ((value + 0.055) / 1.055) ** 2.4;
				})
				.reduce(
					(sum, channel, index) =>
						sum + channel * ([0.2126, 0.7152, 0.0722] as const)[index],
					0,
				);
		return elements.map((element) => {
			const style = getComputedStyle(element);
			const foreground = parseRgb(style.color);
			const background = parseRgb(style.backgroundColor);
			const light = Math.max(luminance(foreground), luminance(background));
			const dark = Math.min(luminance(foreground), luminance(background));
			return (light + 0.05) / (dark + 0.05);
		});
	});
	for (const ratio of contrastRatios) expect(ratio).toBeGreaterThanOrEqual(4.5);
});

test("gray pool cards keep active, resting, and empty states visually distinct", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1200, height: 850 });
	await page.goto(EVERY_STATE_STORY);
	await page.evaluate(() => document.fonts.ready);

	const active = page.getByRole("button", {
		name: /Front Wash With A Deliberately Long Operator Name/u,
	});
	const resting = page.getByRole("button", { name: /Frozen Revision 8/u });
	const empty = page.getByRole("button", {
		name: /Empty Press Record to use this slot/u,
	});
	const recordTarget = page.getByRole("button", {
		name: /Record here Record/u,
	});

	const [activeBackground, restingBackground, emptyBackground] =
		await Promise.all(
			[active, resting, empty].map((card) =>
				card.evaluate((element) => getComputedStyle(element).backgroundColor),
			),
		);
	const lightness = (value: string) => {
		const channels = value
			.match(/\d+(?:\.\d+)?/gu)
			?.slice(0, 3)
			.map(Number);
		if (!channels || channels.length !== 3)
			throw new Error(`Expected a computed RGB color, received ${value}`);
		const scale = value.startsWith("color(srgb") ? 1 : 255;
		return channels.reduce((sum, channel) => sum + channel / scale, 0) / 3;
	};

	expect(lightness(activeBackground)).toBeGreaterThan(
		lightness(restingBackground) * 3,
	);
	expect(lightness(emptyBackground)).toBeLessThan(lightness(restingBackground));
	await expect(empty.locator(".pool-card-name")).toHaveCSS("opacity", "0.45");
	await expect(recordTarget.locator(".pool-card-name")).toHaveCSS(
		"opacity",
		"1",
	);
});
