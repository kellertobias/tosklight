import type { Locator, Page } from "@playwright/test";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import { programmer } from "./support/catalog";

test.use({ viewport: { width: 1600, height: 1000 } });

type Rgb = [number, number, number];

function linear(value: number) {
	return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
}

/** A semantic Color Intent as the virtual engine records it from an sRGB recipe. */
function colorIntent(rgb: Rgb, spreads?: Array<{ component: "hue"; points: number[] }>) {
	const [r, g, b] = rgb.map(linear);
	return {
		kind: "color_program",
		value: {
			kind: "semantic",
			intent: {
				base_xyz: {
					x: 0.4124564 * r + 0.3575761 * g + 0.1804375 * b,
					y: 0.2126729 * r + 0.7151522 * g + 0.072175 * b,
					z: 0.0193339 * r + 0.119192 * g + 0.9503041 * b,
				},
				recipe: { version: 1, rgb, amber: 0, approximate: false },
				white_blend: 0,
				white_target: { kelvin: 6504, duv: 0 },
				uv: { amount: 0 },
				relative_output: 1,
				allocation: "preserve_recipe",
				...(spreads ? { spreads } : {}),
			},
		},
	};
}

function angles(pan: number, tilt: number) {
	return {
		kind: "position",
		value: {
			kind: "angles",
			pan_degrees: { kind: "value", value: pan },
			tilt_degrees: { kind: "value", value: tilt },
		},
	};
}

function target(x: number, y: number) {
	const value = (number: number) => ({ kind: "value", value: number });
	return {
		kind: "position",
		value: {
			kind: "target",
			reference: { kind: "origin" },
			offset_metres: [value(x), value(y), value(0)],
		},
	};
}

async function storePreset(
	api: ApiDriver,
	showId: string,
	family: "Color" | "Position",
	number: number,
	body: Record<string, unknown>,
) {
	const id = `${family === "Color" ? 2 : 3}.${number}`;
	const existing = await api.showObject(showId, "preset", id);
	await api.seedShowObject(
		showId,
		"preset",
		id,
		{ family, number, name: `${family} ${number}`, values: {}, group_values: {}, ...body },
		existing?.revision ?? 0,
	);
}

function presetPane(page: Page) {
	return page.locator('[data-pane-type="presets"]:visible').first();
}

/** Shows one family in the Preset pool, through its tabs or its pane settings. */
async function showFamily(page: Page, family: "Color" | "Position") {
	const pane = presetPane(page);
	const direct = pane.getByRole("button", { name: family, exact: true });
	if (await direct.count()) {
		await direct.click();
	} else {
		await pane.getByRole("button", { name: "Settings", exact: true }).click();
		const settings = page.getByRole("dialog", { name: "Pane Settings" });
		await settings.getByRole("tab", { name: "Pool", exact: true }).click();
		await settings.getByRole("button", { name: family, exact: true }).click();
		await settings.getByRole("button", { name: "Close settings" }).click();
	}
	await expect(pane.locator(".preset-pool-window")).toHaveClass(
		new RegExp(`pool-family-${family.toLowerCase()}`),
	);
}

function tile(page: Page, number: number) {
	return presetPane(page).locator(".preset-card").nth(number - 1);
}

function swatch(page: Page, number: number) {
	return tile(page, number).locator('[data-preset-preview="color"]');
}

function square(page: Page, number: number) {
	return tile(page, number).locator('[data-preset-preview="position"]');
}

async function expectSquareInsideMedia(card: Locator) {
	const media = await card.locator(".pool-card-media").boundingBox();
	const box = await card.locator('[data-preset-preview="position"]').boundingBox();
	if (!media || !box) throw new Error("the position preview is not drawn");
	expect(Math.abs(box.width - box.height)).toBeLessThanOrEqual(1);
	expect(box.width).toBeLessThanOrEqual(media.width + 0.5);
	expect(box.height).toBeLessThanOrEqual(media.height + 0.5);
}

test.describe("docs/testing/40-preset-intent-previews.md", () => {
	test("PRESET-PREVIEW-001 @ui › Color presets show their distinct colours without recalling", async ({
		api,
		desk,
		page,
		show,
	}) => {
		const [a, b, c, d] = show.fixtureIds;
		await storePreset(api, show.id, "Color", 1, {
			universal_values: { color: colorIntent([1, 0, 0]) },
		});
		await storePreset(api, show.id, "Color", 2, {
			values: {
				[a]: { color: colorIntent([1, 0, 0]) },
				[b]: { color: colorIntent([0, 0, 1]) },
				[c]: { color: colorIntent([1, 0, 0]) },
				[d]: { color: colorIntent([0, 0, 1]) },
			},
		});
		await storePreset(api, show.id, "Color", 3, {
			universal_values: {
				color: colorIntent([1, 0, 0], [{ component: "hue", points: [0, 120, 240] }]),
			},
		});
		await storePreset(api, show.id, "Color", 4, {
			values: { [a]: { "color.wheel.1": { kind: "discrete", value: "deep_red" } } },
		});

		await desk.open(api.baseUrl);
		await showFamily(page, "Color");
		await expect(swatch(page, 1)).toHaveAttribute("data-preview-colors", "#ff0000");
		await expect(swatch(page, 2)).toHaveAttribute("data-preview-colors", "#ff0000 #0000ff");
		await expect(swatch(page, 2)).toHaveAccessibleName("Color preview, 2 colours");
		await expect(swatch(page, 3)).toHaveAttribute(
			"data-preview-colors",
			"#ff0000 #ffff00 #00ff00 #00ffff #0000ff",
		);
		await expect(tile(page, 4)).toContainText("Color 4");
		await expect(tile(page, 4).locator(".preset-preview")).toHaveCount(0);

		const state = await programmer(api);
		expect(state.values).toEqual([]);
		expect(state.selected).toEqual([]);
	});

	test("PRESET-PREVIEW-002 @ui › Position presets show up to ten representative aims", async ({
		api,
		desk,
		page,
		show,
	}) => {
		const ids = show.fixtureIds;
		await storePreset(api, show.id, "Position", 1, {
			values: Object.fromEntries(
				[-60, -30, 0, 30, 60].map((pan, index) => [ids[index], { position: angles(pan, 45) }]),
			),
		});
		// Thirty aims in a six-by-five grid, as a Group spread over thirty members would store them.
		const grid = Array.from({ length: 30 }, (_, index) =>
			angles(-50 + (index % 6) * 20, 10 + Math.floor(index / 6) * 15),
		);
		await storePreset(api, show.id, "Position", 2, {
			values: Object.fromEntries(
				grid.map((aim, index) => [ids[index] ?? `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`, { position: aim }]),
			),
		});
		await storePreset(api, show.id, "Position", 3, {
			universal_values: { position: target(0, 2) },
		});

		await desk.open(api.baseUrl);
		await showFamily(page, "Position");
		await expect(square(page, 1).locator("circle")).toHaveCount(5);
		const fan = await square(page, 1)
			.locator("circle")
			.evaluateAll((dots) => dots.map((dot) => dot.getAttribute("cy")));
		expect(new Set(fan).size).toBe(1);

		await expect(square(page, 2).locator("circle")).toHaveCount(10);
		await expect(square(page, 2)).toHaveAccessibleName("Pan/Tilt position preview, 10 of 30 aims");
		const corners = await square(page, 2)
			.locator("circle")
			.evaluateAll((dots) => dots.map((dot) => `${dot.getAttribute("cx")},${dot.getAttribute("cy")}`));
		// The grid spans 100° of Pan and 60° of Tilt; the extremes sit at the square's padding.
		for (const corner of ["10.00,74.00", "90.00,74.00", "10.00,26.00", "90.00,26.00"])
			expect(corners).toContain(corner);

		await expect(square(page, 3)).toHaveAttribute("data-preview-space", "target");
		await expect(square(page, 3).locator("circle")).toHaveAttribute("cx", "50.00");
		await expect(square(page, 3).locator("circle")).toHaveAttribute("cy", "50.00");
		await expectSquareInsideMedia(tile(page, 2));
	});

	test("PRESET-PREVIEW-003 @ui › previews follow updates, yield to a chosen icon and match the hardware-connected layout", async ({
		api,
		bench,
		desk,
		page,
		show,
	}) => {
		await storePreset(api, show.id, "Color", 1, {
			universal_values: { color: colorIntent([1, 0, 0]) },
		});
		await storePreset(api, show.id, "Position", 1, {
			values: { [show.fixtureIds[0]]: { position: angles(-30, 40) }, [show.fixtureIds[1]]: { position: angles(30, 40) } },
		});
		await desk.open(api.baseUrl);
		await showFamily(page, "Color");
		await expect(swatch(page, 1)).toHaveAttribute("data-preview-colors", "#ff0000");

		// An Update reaches the tile live; nothing in the pool is touched.
		await storePreset(api, show.id, "Color", 1, {
			universal_values: { color: colorIntent([0, 1, 0]) },
		});
		await expect(swatch(page, 1)).toHaveAttribute("data-preview-colors", "#00ff00");

		// A chosen icon replaces the preview; Automatic icon brings it back.
		const configure = async () => {
			await page.locator('[data-keypad-key="SET"]:visible').first().click();
			await expect(tile(page, 1)).toHaveClass(/(?:^|\s)set-target(?:\s|$)/);
			await tile(page, 1).click();
			return page.getByRole("dialog", { name: "Configure preset button" });
		};
		let dialog = await configure();
		await dialog.getByRole("button", { name: /Choose icon/i }).click();
		await page.getByRole("button", { name: "Use ★" }).click();
		await dialog.getByRole("button", { name: "Save button" }).click();
		await expect(dialog).toBeHidden();
		await expect(tile(page, 1).locator(".pool-card-media")).toHaveText("★");
		await expect(swatch(page, 1)).toHaveCount(0);

		dialog = await configure();
		await dialog.getByRole("button", { name: "Automatic icon" }).click();
		await dialog.getByRole("button", { name: "Save button" }).click();
		await expect(dialog).toBeHidden();
		await expect(swatch(page, 1)).toHaveAttribute("data-preview-colors", "#00ff00");

		// The same previews in the hardware-connected layout.
		const hardware = await bench.osc();
		await hardware.subscribe(`preset-preview-${crypto.randomUUID()}`, "desk");
		await expect(page.locator(".control-section.hardware-connected")).toBeVisible();
		await expect(swatch(page, 1)).toHaveAttribute("data-preview-colors", "#00ff00");
		await showFamily(page, "Position");
		await expect(square(page, 1).locator("circle")).toHaveCount(2);
		await expectSquareInsideMedia(tile(page, 1));
		await page.screenshot({
			path: test.info().outputPath("hardware-connected-position-previews.png"),
		});
	});
});
