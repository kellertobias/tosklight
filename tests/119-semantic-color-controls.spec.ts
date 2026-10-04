import type { Locator, Page } from "@playwright/test";
import {
	acceptedColorReport,
	approximation,
	approximationRow,
	channelBytes,
	closeColorModal,
	colorEdits,
	colorModalLayer,
	createColorIntentShow,
	openColorSpecialDialog,
	openFullColorModal,
	programColor,
	programmerValues,
	SEMANTIC_COLOR_GATE,
	saveMeasuredRgbProfile,
	saveMeasuredWheelSpot,
	scalarColorEdit,
	selectedFixtureIds,
	selectFixtures,
	semanticPagesPublished,
	setIntensity,
	uvWording,
} from "./bench/color/semanticColorScenario";
import type { ApiDriver } from "./bench/core/api";
import type { DeskDriver } from "./bench/core/desk";
import { expect, test } from "./bench/core/fixtures";
import type { LightBench } from "./bench/core/lightBench";
import { requireSemanticContract } from "./bench/core/semanticContract";
import { fixture } from "./bench/output/fixtureDmxContract";

/**
 * docs/testing/36-semantic-color-controls.md (TL-550): the full Color modal, Media layers and the
 * quiet Fixture Sheet Color status. SEMANTIC-COLOR-001 and 004 live in tests/112-color-intent.spec.ts,
 * SEMANTIC-COLOR-003 in tests/65-semantic-special-dialogs-and-hardware-selection.spec.ts.
 *
 * Production reports programming contract 1 (TL-552), so these run under `npm run test:e2e`; the
 * gate only skips on an older contract-0 runtime and fails in the `e2e-semantic` project.
 *
 * Every case starts from a fresh Color Intent show patched from the shipped library. The bench
 * clock is manual: a tick accepts one output frame, so the colour report describes output that
 * was actually sent.
 */

const WIDE = { width: 1496, height: 761 };
const MAGENTA = { hue: 300, saturation: 1 };

test.describe("docs/testing/36-semantic-color-controls.md", () => {
	test("SEMANTIC-COLOR-002 @ui › full modal: hue ring with every fader beside it and the per-fixture approximation of a mixed rig", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.setTimeout(90_000);
		const { show, report } = await arrangeMixedRig({ api, bench, desk, page });

		await desk.open(api.baseUrl);
		const layer = await openModalWithReport(page, bench, 3);
		const frame = layer.getByRole("dialog", { name: "Color Special Dialog" });
		// The standard modal frame titled Color with its close button.
		await expect(frame).toBeVisible();
		await expect(layer.getByRole("button", { name: "Close Special Dialog", exact: true })).toBeVisible();

		// A large hue ring (about 340 px) with Saturation, White Blend, Temperature and Duv beside it.
		const ring = await box(layer.getByRole("slider", { name: "Hue" }));
		expect(ring.width).toBeGreaterThanOrEqual(300);
		expect(ring.width).toBeLessThanOrEqual(400);
		expect(Math.abs(ring.width - ring.height)).toBeLessThanOrEqual(1);
		for (const name of ["Saturation", "White Blend", "Temperature", "Duv"]) {
			const fader = await box(layer.getByRole("slider", { name, exact: true }));
			expect(fader.x, `${name} sits beside the ring`).toBeGreaterThan(ring.x + ring.width);
			expect(fader.y).toBeGreaterThanOrEqual(ring.y - 1);
			expect(fader.y + fader.height).toBeLessThanOrEqual(ring.y + ring.height + 1);
		}

		// The per-fixture approximation sits below and is readable without scrolling at 1496×761.
		const results = approximation(layer);
		await expect(results.locator("tbody tr")).toHaveCount(3);
		const table = await box(results);
		expect(table.y).toBeGreaterThan(ring.y + ring.height);
		const outer = await box(frame);
		expect(outer.y).toBeGreaterThanOrEqual(0);
		expect(outer.y + outer.height).toBeLessThanOrEqual(WIDE.height);
		const body = layer.getByTestId("editor-page");
		expect(await body.evaluate((element) => element.scrollTop)).toBe(0);
		const visibleBody = await box(body);
		expect(table.y + table.height).toBeLessThanOrEqual(visibleBody.y + visibleBody.height);

		// The requested swatch once.
		const swatch = results.getByTestId("color-requested-swatch");
		await expect(swatch).toHaveCount(1);
		await expect(swatch).toHaveCSS("background-color", "rgb(255, 0, 255)");

		// One row per head from the accepted output, the visible match with Δu′v′, UV in its column.
		let withDelta = 0;
		for (const head of report.heads) {
			const row = approximationRow(results, head.fixture_id);
			await expect(row).toHaveCount(1);
			const visible = row.locator("td").nth(0);
			if (head.delta_uv != null && head.quality !== "exact") {
				withDelta += 1;
				await expect(visible).toContainText(`Δu′v′ ${head.delta_uv.toFixed(4)}`);
			} else await expect(visible).not.toContainText("Δu′v′");
			await expect(row.locator("td").nth(1)).toHaveText(uvWording(head) ?? "");
		}
		expect(withDelta, "the measured wheel reports its visible distance").toBeGreaterThan(0);
		expect(report.heads.find((head) => head.fixture_number === 3)?.quality).toBe("wheel_limited");
		const uv = Object.fromEntries(report.heads.map((head) => [head.fixture_number, uvWording(head)]));
		expect(uv[1]).toBe("UV unavailable on this fixture");
		expect(uv[3]).toBe("UV unavailable on this fixture");
		expect(["UV applied", "UV limited by the emitter"]).toContain(uv[2]);
		await expect(results.locator("thead th").nth(2)).toHaveText("UV");

		// A fixture without a programmed colour is not listed.
		await expect(approximationRow(results, show.ids[4])).toHaveCount(0);
		await closeColorModal(layer);
	});

	test("SEMANTIC-COLOR-005 @ui › Media layers: Media color with Preview, White Blend greys the picture and leaves Intensity alone", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.setTimeout(90_000);
		await page.setViewportSize(WIDE);
		const show = await createColorIntentShow(api, page, desk, "Media", [
			{ number: MEDIA, name: "Media 9", manufacturer: "ToskLight", profile: "Media Server", mode: "2 layers", address: "2.1" },
			{ number: 1, name: "RGB 1", manufacturer: "Generic", profile: "RGB LED", mode: "RGB virtual dimmer", address: "1.1" },
		]);
		const media = (await api.patch()).fixtures.find((entry) => entry.fixture_number === MEDIA);
		if (!media) throw new Error("Media Server 9 was not patched");
		const layers = media.logical_heads.map((head) => head.fixture_id);
		expect(layers).toHaveLength(2);
		requireSemanticContract(await semanticPagesPublished(api, layers), SEMANTIC_COLOR_GATE);
		await selectFixtures(api, show.id, layers);
		// Layer Intensity 60%, Master Intensity 100%: no Color edit may touch either.
		await setIntensity(api, layers, 0.6);
		await setIntensity(api, [media.fixture_id], 1);
		const intensities = await intensityValues(api);
		const levels = { layer: 153, master: 255 };
		await expectMediaIntensity(api, bench, levels);

		// 1. Compact page two is Preview instead of Temperature and Duv; the modal is Media color.
		await desk.open(api.baseUrl);
		const opened = await openColorSpecialDialog(page);
		if (await opened.getByRole("button", { name: "Expand", exact: true }).count()) {
			await expect(opened.getByRole("button")).toHaveText(["Preview", "Expand"]);
			await opened.getByRole("button", { name: "Switch to Preview", exact: true }).click();
			await expect(opened.getByTestId("media-color-preview")).toBeVisible();
			await expect(opened.getByRole("slider", { name: "Temperature" })).toHaveCount(0);
			await expect(opened.getByRole("slider", { name: "Duv" })).toHaveCount(0);
			await opened.getByRole("button", { name: "Expand", exact: true }).click();
		}
		const layer = colorModalLayer(page);
		await expect(layer.getByRole("heading", { level: 2, name: "Media color", exact: true })).toBeVisible();
		await expect(layer.getByRole("region", { name: "Media preview" })).toBeVisible();
		await expect(layer.getByRole("slider", { name: "Temperature" })).toHaveCount(0);
		await expect(layer.getByRole("slider", { name: "Duv" })).toHaveCount(0);
		await expect(layer.getByTestId("color-approximation")).toHaveCount(0);

		// 2. A neutral tint (Saturation 0%): White Blend 0 → 50 → 100% greys the picture progressively.
		await keyTo(page, layer.getByRole("slider", { name: "Saturation", exact: true }), "Home");
		const whiteBlend = layer.getByRole("slider", { name: "White Blend", exact: true });
		const chroma: number[] = [];
		for (const [percent, grayscale] of [
			[0, [0, 0]],
			[50, [127, 128]],
			[100, [255, 255]],
		] as const) {
			await whiteBlend.fill(String(percent));
			await expect(layer.getByLabel("White Blend value")).toHaveText(`${percent}%`);
			await expectLayers(api, bench, { grayscale, tint: [0, 0, 0] });
			await expectMediaIntensity(api, bench, levels);
			chroma.push(await previewChroma(layer));
		}
		expect(chroma[0]).toBeGreaterThan(chroma[1]);
		expect(chroma[1]).toBeGreaterThan(chroma[2]);
		expect(chroma[2]).toBe(0);

		// A 100% red tint keeps the tint: the greyed picture is red, not neutral.
		await keyTo(page, layer.getByRole("slider", { name: "Hue" }), "Home");
		await keyTo(page, layer.getByRole("slider", { name: "Saturation", exact: true }), "End");
		// The Media personality's tint channels are inverted: raw 0 keeps that primary in full.
		await expectLayers(api, bench, { grayscale: [255, 255], tint: [0, 255, 255] });
		const red = await previewPixels(layer);
		expect(red.every(([, green, blue]) => green === 0 && blue === 0)).toBe(true);
		expect(red.some(([level]) => level > 0)).toBe(true);
		for (const percent of [50, 0]) {
			await whiteBlend.fill(String(percent));
			await expectLayers(api, bench, {
				grayscale: percent === 0 ? [0, 0] : [127, 128],
				tint: [0, 255, 255],
			});
		}

		// 3. Layer and Master Intensity are unchanged by every Color edit.
		await expectMediaIntensity(api, bench, levels);
		expect(await intensityValues(api)).toEqual(intensities);

		// 4. Adding a lamp to the selection opens the lamp dialog instead.
		await closeColorModal(layer);
		await selectFixtures(api, show.id, [show.ids[1], ...layers]);
		const lamp = await openColorSpecialDialog(page);
		if (await lamp.getByRole("button", { name: "Expand", exact: true }).count()) {
			await expect(lamp.getByRole("button")).toHaveText(["White balance", "Expand"]);
			await lamp.getByRole("button", { name: "Expand", exact: true }).click();
		}
		await expect(layer.getByRole("heading", { level: 2, name: "Color", exact: true })).toBeVisible();
		await expect(layer.getByRole("slider", { name: "Temperature" })).toBeVisible();
		await expect(layer.getByTestId("media-color-preview")).toHaveCount(0);
		await closeColorModal(layer);
	});

	test("SEMANTIC-COLOR-006 @ui › one quiet Fixture Sheet triangle for UV a fixture cannot give opens its Color details", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.setTimeout(90_000);
		const { show, all } = await arrangeUvSheet({ api, bench, desk, page });
		const selected = await selectedFixtureIds(api);

		const reports: Array<{ at: number; fixtures: string[] }> = [];
		page.on("request", (request) => {
			const url = new URL(request.url());
			if (url.pathname === "/api/v2/color-intent/report")
				reports.push({ at: Date.now(), fixtures: (url.searchParams.get("fixtures") ?? "").split(",") });
		});
		const sheet = await openFixtureSheet(desk, page, api.baseUrl);

		// 1. One small, steady, subdued triangle beside RGB 1's Color value.
		const notices = sheet.getByTestId("fixture-sheet-color-notice");
		await untilNoticesShown(api, bench, notices, show.ids[2]);
		const notice = notices.first();
		await expect(sheetRow(sheet, "RGB 1").getByTestId("fixture-sheet-color-notice")).toHaveCount(1);
		await expect(sheetRow(sheet, "RGB 2").getByTestId("fixture-sheet-color-notice")).toHaveCount(0);
		await expect(notice).toHaveAccessibleName(/UV unavailable on this fixture/);
		const glyph = notice.locator(".fixture-sheet-color-notice-glyph");
		const glyphBox = await box(glyph);
		expect(glyphBox.width).toBeLessThanOrEqual(14);
		expect(glyphBox.height).toBeLessThanOrEqual(14);
		const style = await glyph.evaluate((element) => {
			const computed = getComputedStyle(element);
			return { opacity: Number(computed.opacity), animation: computed.animationName };
		});
		expect(style.opacity).toBeLessThan(1);
		expect(style.animation).toBe("none");
		const header = sheet.locator(".ui-data-table-row.header");
		const colorColumn = await box(header.getByText("Color", { exact: true }));
		const positionColumn = await box(header.getByText("Position", { exact: true }));
		const triangle = await box(notice);
		expect(triangle.x).toBeGreaterThanOrEqual(colorColumn.x);
		expect(triangle.x + triangle.width).toBeLessThanOrEqual(positionColumn.x);

		// 2. While the output keeps changing: no toast, banner, alert or live region; focus stays.
		const focused = await page.evaluate(() => {
			document.activeElement?.setAttribute("data-semantic-color-006-focus", "");
			return document.activeElement?.tagName ?? null;
		});
		const started = Date.now();
		for (const level of [0.5, 0.8, 0.3, 1]) {
			await setIntensity(api, all, level);
			await bench.tick(25);
			await page.waitForTimeout(500);
			await expect(notices).toHaveCount(1);
		}
		const elapsed = Date.now() - started;
		await expect(page.locator(".desk-notice-toast, .server-error-toast, .update-armed-banner")).toHaveCount(0);
		await expect(page.locator("[role='alert'], [role='alertdialog']")).toHaveCount(0);
		for (const live of await page.locator("[role='status'], [aria-live]:not([aria-live='off'])").all())
			await expect(live).not.toContainText(/UV|colou?r/i);
		expect(
			await page.evaluate(() => document.activeElement?.hasAttribute("data-semantic-color-006-focus") ?? false),
			`focus stays on ${focused}`,
		).toBe(true);
		await expect(colorModalLayer(page)).toHaveCount(0);

		// One batched report request covers every row on screen (never one per row), and the
		// reads stay throttled while the output changes.
		expect(reports.length).toBeGreaterThan(0);
		for (const request of reports) expect([...request.fixtures].sort()).toEqual([...all].sort());
		const during = reports.filter((request) => request.at >= started);
		expect(during.length).toBeLessThanOrEqual(Math.ceil(elapsed / 1_000) + 1);

		// 3. Tapping the triangle opens the full Color modal on RGB 1's details; selection unchanged.
		await notice.click();
		const layer = colorModalLayer(page);
		await expectDetailsOf(layer, show.ids[1], show.ids[2]);
		expect(await selectedFixtureIds(api)).toEqual(selected);
		await closeColorModal(layer);

		// Enter on the focused triangle does the same.
		await notice.focus();
		await page.keyboard.press("Enter");
		await expectDetailsOf(layer, show.ids[1], show.ids[2]);
		expect(await selectedFixtureIds(api)).toEqual(selected);
		await closeColorModal(layer);
	});

	test("SEMANTIC-COLOR-002 @ui › the full modal has no vertical overflow at 1496×761", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.fail(
			true,
			"BUG: at 1496×761 the full Color modal body overflows vertically by about 170 px and scrolls: the TL-554 Direct color section sits below the approximation",
		);
		test.setTimeout(60_000);
		await arrangeMixedRig({ api, bench, desk, page });
		await desk.open(api.baseUrl);
		const layer = await openModalWithReport(page, bench, 3);
		const body = layer.getByTestId("editor-page");
		const overflow = await body.evaluate((element) => element.scrollHeight - element.clientHeight);
		expect(overflow).toBeLessThanOrEqual(0);
	});

	test("SEMANTIC-COLOR-006 @ui › the triangle sits beside the fixture's shown Color value", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.fail(
			true,
			"BUG: the Fixture Sheet Color cell reads '—' with no colour dot for a fixture whose Programmer holds a semantic colour (color_program), although DMX outputs it",
		);
		test.setTimeout(60_000);
		await arrangeUvSheet({ api, bench, desk, page });
		const sheet = await openFixtureSheet(desk, page, api.baseUrl);
		const row = sheetRow(sheet, "RGB 1");
		await bench.tick(25);
		const value = row.locator(".fixture-sheet-group-presentation");
		await expect(value.locator(".color-dot")).toHaveCount(1, { timeout: 3_000 });
		await expect(value).not.toHaveText("—");
	});

	test("SEMANTIC-COLOR-006 @ui › the triangle appears once the output is accepted, without a Programmer change", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.fail(
			true,
			"BUG: useAcceptedColorReport drops a not_yet_available colour report and never re-reads until the Programmer projection changes, so the triangle stays missing after the frame is accepted",
		);
		test.setTimeout(60_000);
		const { show, all } = await arrangeUvSheet({ api, bench, desk, page });
		// The sheet's reads while it opens land before the frame is accepted, as when the output
		// generation moves while the sheet opens; afterwards every read reaches the server.
		let held = true;
		let intercepted = 0;
		await page.route("**/api/v2/color-intent/report?**", async (route) => {
			if (!held) return route.continue();
			intercepted += 1;
			await route.fulfill({
				status: 200,
				contentType: "application/json",
				body: JSON.stringify({
					color_model: "intent",
					heads: [],
					accepted_frame: { state: "not_yet_available", frame: null },
				}),
			});
		});
		const sheet = await openFixtureSheet(desk, page, api.baseUrl);
		await expect.poll(() => intercepted).toBeGreaterThan(0);
		await page.waitForTimeout(2_000);
		held = false;
		expect((await acceptedColorReport(api, bench, show.id, all)).heads).toHaveLength(2);
		const notices = sheet.getByTestId("fixture-sheet-color-notice");
		await expect
			.poll(
				async () => {
					await bench.tick(25);
					return notices.count();
				},
				{ timeout: 6_000 },
			)
			.toBe(1);
	});
});

// ---------------------------------------------------------------------------------------------

const MEDIA = 9;
const MASTER_HEAD = 1;
const LAYER_HEADS = [2, 3];

interface Bench {
	api: ApiDriver;
	bench: LightBench;
	desk: DeskDriver;
	page: Page;
}

/**
 * SEMANTIC-COLOR-002 rig: JBLED A7, ROOT PAR 6 and a wheel-only fixture programmed magenta with a
 * UV request and output once; a fourth selected RGB LED holds no colour.
 */
async function arrangeMixedRig({ api, bench, desk, page }: Bench) {
	await page.setViewportSize(WIDE);
	// The wheel-only fixture carries the shipped AURO SPOT Z300 wheel with measured slots, so its
	// wheel-limited match carries a Δu′v′ (an uncalibrated estimate never shows one).
	const wheel = await saveMeasuredWheelSpot(api);
	const show = await createColorIntentShow(api, page, desk, "Full modal", [
		{ number: 1, name: "A7 1", manufacturer: "JB-Lighting", profile: "JBLED A7", mode: "Standard RGB 8 Bit (S8)", address: "1.1" },
		{ number: 2, name: "Par 2", manufacturer: "Cameo", profile: "ROOT PAR 6", mode: "D7CH — Delay Off, virtual dimmer", address: "1.21" },
		{ number: 3, name: "Spot 3", ...wheel, address: "1.31" },
		{ number: 4, name: "RGB 4", manufacturer: "Generic", profile: "RGB LED", mode: "RGB virtual dimmer", address: "1.61" },
	]);
	const all = [1, 2, 3, 4].map((number) => show.ids[number]);
	requireSemanticContract(await semanticPagesPublished(api, all), SEMANTIC_COLOR_GATE);
	await selectFixtures(api, show.id, all);
	await setIntensity(api, all, 1);
	await programColor(api, all.slice(0, 3), MAGENTA, [scalarColorEdit("uv", 1)]);
	const report = await acceptedColorReport(api, bench, show.id, all);
	expect(report.heads.map((head) => head.fixture_number).sort()).toEqual([1, 2, 3]);
	return { show, report };
}

/**
 * SEMANTIC-COLOR-006 rig: two measured RGB fixtures show red exactly, so the only limitation is
 * the UV request on RGB 1; output once.
 */
async function arrangeUvSheet({ api, bench, desk, page }: Bench) {
	await page.setViewportSize(WIDE);
	const measured = await saveMeasuredRgbProfile(api);
	const show = await createColorIntentShow(api, page, desk, "Sheet", [
		{ number: 1, name: "RGB 1", ...measured, mode: "DRGB 8-bit dimmer first", address: "1.1" },
		{ number: 2, name: "RGB 2", ...measured, mode: "DRGB 8-bit dimmer first", address: "1.11" },
	]);
	const all = [show.ids[1], show.ids[2]];
	requireSemanticContract(await semanticPagesPublished(api, all), SEMANTIC_COLOR_GATE);
	await selectFixtures(api, show.id, all);
	await setIntensity(api, all, 1);
	await programColor(api, all, { hue: 0, saturation: 1 });
	await colorEdits(api, [show.ids[1]], [scalarColorEdit("uv", 1)]);
	const report = await acceptedColorReport(api, bench, show.id, all);
	expect(
		report.heads
			.map((head) => [head.fixture_number, head.quality, uvWording(head)])
			.sort((left, right) => Number(left[0]) - Number(right[0])),
	).toEqual([
		[1, "exact", "UV unavailable on this fixture"],
		[2, "exact", null],
	]);
	return { show, all };
}

async function openFixtureSheet(desk: DeskDriver, page: Page, baseUrl: string) {
	await desk.open(baseUrl);
	await desk.openFixtures();
	const sheet = page.locator(".fixture-window");
	await expect(sheet).toBeVisible();
	return sheet;
}

/**
 * Opens the full Color modal and outputs until its approximation lists `rows` heads. The modal
 * reads the report when it opens; a read that lands before the frame is accepted is reopened
 * (see the SEMANTIC-COLOR-006 retry BUG below).
 */
async function openModalWithReport(page: Page, bench: LightBench, rows: number) {
	let layer = await openFullColorModal(page);
	for (let attempt = 0; attempt < 5; attempt += 1) {
		await bench.tick(25);
		try {
			await expect(approximation(layer).locator("tbody tr")).toHaveCount(rows, { timeout: 2_500 });
			return layer;
		} catch {
			await closeColorModal(layer);
			layer = await openFullColorModal(page);
		}
	}
	await expect(approximation(layer).locator("tbody tr")).toHaveCount(rows);
	return layer;
}

/**
 * Outputs until the sheet shows exactly one triangle. A sheet read that lands before the frame is
 * accepted is only repeated on a Programmer change (SEMANTIC-COLOR-006 retry BUG), so the bench
 * nudges the Intensity of a fixture without a limitation and restores it.
 */
async function untilNoticesShown(api: ApiDriver, bench: LightBench, notices: Locator, nudgeId: string) {
	let nudged = false;
	await expect
		.poll(
			async () => {
				await bench.tick(25);
				if ((await notices.count()) === 1) return 1;
				nudged = !nudged;
				await setIntensity(api, [nudgeId], nudged ? 0.99 : 1);
				await bench.tick(25);
				return notices.count();
			},
			{ timeout: 15_000, intervals: [1_200] },
		)
		.toBe(1);
	if (nudged) await setIntensity(api, [nudgeId], 1);
}

async function box(locator: Locator) {
	await expect(locator).toBeVisible();
	const bounds = await locator.boundingBox();
	if (!bounds) throw new Error("element has no box");
	return bounds;
}

async function keyTo(page: Page, slider: Locator, key: "Home" | "End") {
	await slider.focus();
	await page.keyboard.press(key);
}

function sheetRow(sheet: Locator, name: string) {
	return sheet.locator(".ui-data-table-row:not(.header)").filter({ hasText: name });
}

async function expectDetailsOf(layer: Locator, focusedId: string, otherId: string) {
	await expect(layer.getByRole("heading", { level: 2, name: "Color", exact: true })).toBeVisible();
	const row = approximationRow(approximation(layer), focusedId);
	await expect(row).toHaveAttribute("data-focused", "true");
	await expect(row).toContainText("UV unavailable on this fixture");
	await expect(approximationRow(approximation(layer), otherId)).not.toHaveAttribute("data-focused", "true");
}

async function intensityValues(api: ApiDriver) {
	return (await programmerValues(api))
		.filter((value) => value.attribute === "intensity")
		.sort((left, right) => left.fixture_id.localeCompare(right.fixture_id));
}

async function expectMediaIntensity(
	api: ApiDriver,
	bench: LightBench,
	levels: { layer: number; master: number },
) {
	await expect
		.poll(async () => {
			const heads = [MASTER_HEAD, ...LAYER_HEADS];
			const bytes = [];
			for (const head of heads)
				bytes.push((await channelBytes(api, bench, fixture(MEDIA, head), ["Intensity"])).Intensity);
			return bytes;
		})
		.toEqual([levels.master, levels.layer, levels.layer]);
}

/** Both layers' greyscale (within the inclusive range) and raw tint bytes on DMX. */
async function expectLayers(
	api: ApiDriver,
	bench: LightBench,
	expected: { grayscale: readonly [number, number]; tint: readonly [number, number, number] },
) {
	await expect
		.poll(async () => {
			const layers = [];
			for (const head of LAYER_HEADS)
				layers.push(
					await channelBytes(api, bench, fixture(MEDIA, head), [
						"Media grayscale",
						"Color red",
						"Color green",
						"Color blue",
					]),
				);
			return layers.every(
				(bytes) =>
					bytes["Media grayscale"] >= expected.grayscale[0] &&
					bytes["Media grayscale"] <= expected.grayscale[1] &&
					bytes["Color red"] === expected.tint[0] &&
					bytes["Color green"] === expected.tint[1] &&
					bytes["Color blue"] === expected.tint[2],
			)
				? "ok"
				: JSON.stringify(layers);
		})
		.toBe("ok");
}

async function previewPixels(layer: Locator): Promise<Array<[number, number, number]>> {
	return layer.locator(".media-color-preview-card > i").evaluateAll((cells) =>
		cells.map((cell) => {
			const match = getComputedStyle(cell).backgroundColor.match(/\d+(\.\d+)?/g) ?? [];
			return [Number(match[0]), Number(match[1]), Number(match[2])] as [number, number, number];
		}),
	);
}

/** The largest per-pixel chroma (max − min channel) of the Media preview card. */
async function previewChroma(layer: Locator) {
	return Math.max(...(await previewPixels(layer)).map((pixel) => Math.max(...pixel) - Math.min(...pixel)));
}
