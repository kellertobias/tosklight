import { expect, test } from "@playwright/test";

const STORY =
	"/iframe.html?id=tosklight-windows-pool-media-alignment--square-and-configured-tiles&viewMode=story";

test("pool artwork stays vertically and horizontally centered without clipping", async ({
	page,
}) => {
	await page.setViewportSize({ width: 1200, height: 2200 });
	await page.goto(STORY);
	await expect(page.locator(".pool-card-media")).toHaveCount(108);
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
					".pool-card-icon-image, .pool-card-image, .preset-preview-position, .pool-card-icon, .pool-card-color",
				);
				if (!artwork) throw new Error("Missing tile artwork");
				const outer = media.getBoundingClientRect();
				const inner = artwork.getBoundingClientRect();
				const tile = media.closest("button");
				const parent = media.parentElement;
				if (!tile || !parent) throw new Error("Missing tile media slot");
				const card = tile.getBoundingClientRect();
				const slot = parent.getBoundingClientRect();
				const title = tile.querySelector(".pool-card-name");
				const count = tile.querySelector(".pool-card-information");
				const id = tile.querySelector(".number");
				const titleRange = document.createRange();
				if (!title || !count || !id) throw new Error("Missing tile labels");
				titleRange.selectNodeContents(title);
				const titleBottom = Math.max(
					...Array.from(titleRange.getClientRects()).map((rect) => rect.bottom),
				);
				const countBox = count.getBoundingClientRect();
				const idBox = id.getBoundingClientRect();
				return {
					titleGap: outer.top - titleBottom,
					titleLeftGap: outer.left - title.getBoundingClientRect().right,
					countGap: outer.left - countBox.right,
					idGap: outer.top - idBox.bottom,
					idLeftGap: idBox.left - outer.right,
					mediaWidth: outer.width,
					mediaHeight: outer.height,
					expectedRatio: media.classList.contains("pool-card-media-image")
						? 16 / 9
						: 1,
					tileWidth: card.width,
					tileHeight: card.height,
					slotBottom: slot.bottom - outer.bottom,
					slotRight: slot.right - outer.right,
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
		if (measurement.expectedRatio === 1) {
			const previousSize =
				Math.min(measurement.tileWidth - 14.8, measurement.tileHeight - 14.8) *
				0.28;
			expect(measurement.mediaWidth, context).toBeGreaterThanOrEqual(
				previousSize * 2 - 0.1,
			);
			expect(measurement.mediaWidth, context).toBeGreaterThanOrEqual(32 - 0.1);
		}
		expect(
			Math.max(measurement.titleGap, measurement.titleLeftGap),
			context,
		).toBeGreaterThanOrEqual(-0.6);
		expect(measurement.countGap, context).toBeGreaterThanOrEqual(0);
		expect(
			Math.max(measurement.idGap, measurement.idLeftGap),
			context,
		).toBeGreaterThanOrEqual(0);
		expect(
			Math.abs(
				measurement.mediaWidth -
					measurement.mediaHeight * measurement.expectedRatio,
			),
			context,
		).toBeLessThanOrEqual(0.6);
		expect(Math.abs(measurement.slotBottom), context).toBeLessThanOrEqual(0.6);
		expect(Math.abs(measurement.slotRight), context).toBeLessThanOrEqual(0.6);
		if (["square", "large", "tiny"].includes(measurement.shape ?? ""))
			expect(
				Math.abs(measurement.tileWidth - measurement.tileHeight),
				context,
			).toBeLessThanOrEqual(0.6);
		expect(Math.abs(measurement.dx), context).toBeLessThanOrEqual(0.6);
		expect(Math.abs(measurement.dy), context).toBeLessThanOrEqual(0.6);
		for (const edge of ["left", "top", "right", "bottom"] as const)
			expect(measurement[edge], context).toBeGreaterThanOrEqual(0.5);
	}
	// Automatic square cards retain their width; both artwork dimensions grow with them.
	for (const selected of ["false", "true"]) {
		const widthFor = (shape: string) => {
			const measurement = measurements.find(
				(item) =>
					item.shape === shape &&
					item.selected === selected &&
					item.name?.includes("Fixture"),
			);
			if (!measurement) throw new Error(`Missing ${shape} fixture tile`);
			return measurement.mediaWidth;
		};
		expect(widthFor("large")).toBeGreaterThan(widthFor("square") * 1.5);
		expect(widthFor("square")).toBeGreaterThan(widthFor("tiny") * 1.3);
	}
	// Selection changes the border, never the size-query mode or artwork position.
	for (const measurement of measurements.filter(
		(item) => item.selected === "false",
	)) {
		const selected = measurements.find(
			(item) =>
				item.shape === measurement.shape &&
				item.name === measurement.name &&
				item.selected === "true",
		);
		if (!selected) throw new Error("Missing selected tile pair");
		expect(
			Math.abs(selected.mediaWidth - measurement.mediaWidth),
		).toBeLessThanOrEqual(0.1);
		expect(
			Math.abs(selected.mediaHeight - measurement.mediaHeight),
		).toBeLessThanOrEqual(0.1);
	}
	const expectedWidths: Record<string, number> = {
		square: 96,
		large: 180,
		tiny: 64,
		rectangular: 160,
		group: 124,
		preset: 210,
		"shallow-color": 98,
		"shallow-group": 100,
		boundary: 100,
	};
	const expectedHeights: Record<string, number> = {
		"shallow-color": 36,
		"shallow-group": 53,
		boundary: 58,
	};
	for (const measurement of measurements) {
		const height = expectedHeights[measurement.shape ?? ""];
		if (height !== undefined) expect(measurement.tileHeight).toBe(height);
		const width = expectedWidths[measurement.shape ?? ""];
		if (width === undefined) throw new Error("Unexpected tile shape");
		expect(Math.abs(measurement.tileWidth - width)).toBeLessThanOrEqual(0.6);
	}
});

test("media library keeps its full-card image beneath the title strip", async ({
	page,
}) => {
	await page.goto(
		STORY.replace("square-and-configured-tiles", "full-card-media-preview"),
	);
	const card = page.locator(".pool-card");
	await expect(card).toBeVisible();
	const bounds = await card.evaluate((element) => {
		const card = element.getBoundingClientRect();
		const artwork = element.querySelector(".pool-card-media");
		if (!artwork) throw new Error("Missing library preview");
		const media = artwork.getBoundingClientRect();
		return {
			cardWidth: card.width,
			mediaWidth: media.width,
			top: media.top - card.top,
			bottom: card.bottom - media.bottom,
		};
	});
	expect(bounds.mediaWidth).toBeGreaterThan(bounds.cardWidth * 0.98);
	expect(bounds.top).toBeCloseTo(31, 0);
	expect(bounds.bottom).toBeCloseTo(1, 0);
});
