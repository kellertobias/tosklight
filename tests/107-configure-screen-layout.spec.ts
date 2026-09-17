import type { Locator, Page } from "@playwright/test";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";

type Box = { x: number; y: number; width: number; height: number };

type StoredScreen = {
	id: string;
	page_mode: string;
	bounds?: { x: number } | null;
	playback_layout?: {
		playbacks_per_row: number;
		rows: Array<{ first_playback_slot: number }>;
	} | null;
};

async function box(locator: Locator): Promise<Box> {
	const result = await locator.boundingBox();
	if (!result) throw new Error("Element has no layout box");
	return result;
}

async function screens(api: ApiDriver) {
	return (
		await api.request<{ screens: StoredScreen[] }>("GET", "/api/v2/screens")
	).screens;
}

async function deleteScreens(api: ApiDriver, before: string[]) {
	for (const screen of await screens(api))
		if (!before.includes(screen.id))
			await api.request("POST", "/api/v2/screens/actions", {
				request_id: crypto.randomUUID(),
				action: { type: "delete", screen_id: screen.id },
			});
}

/** Opens Desk Setup, adds one optional screen, and opens its Configure Screen modal. */
async function configureNewScreen(page: Page) {
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
	await page
		.locator(".setup-window nav")
		.getByRole("button", { name: "Screens & playback", exact: true })
		.click();
	const cards = page.locator(".screen-settings-card[data-screen-id]");
	const count = await cards.count();
	await page
		.locator(".setup-window .ui-window-header")
		.getByRole("button", { name: "Add Screen", exact: true })
		.click();
	const card = cards.nth(count);
	await expect(card).toBeVisible();
	const screenId = await card.getAttribute("data-screen-id");
	if (!screenId) throw new Error("Created screen has no runtime identity");
	const name = await card.getByLabel("Screen name").inputValue();
	await card
		.getByRole("button", { name: "Configure screen", exact: true })
		.click();
	const dialog = page.getByRole("dialog", { name: `Configure ${name}` });
	await expect(dialog).toBeVisible();
	return {
		screenId,
		dialog,
		form: dialog.locator(".screen-configuration-modal-content"),
	};
}

/** The nearest form field around a named switch. */
function switchField(dialog: Locator, name: string) {
	return dialog
		.getByRole("switch", { name, exact: true })
		.locator(
			"xpath=ancestor::*[contains(concat(' ', normalize-space(@class), ' '), ' ui-form-field ')][1]",
		);
}

function layoutSwitchFields(dialog: Locator) {
	return ["Dock", "Playbacks", "Command line", "Programming"].map((name) =>
		switchField(dialog, name),
	);
}

/** No framed section or heading sits between the modal and its form fields. */
async function expectNoInnerFrames(dialog: Locator, form: Locator) {
	await expect(dialog.locator("section section, h3")).toHaveCount(0);
	await expect(form.locator(":scope > section")).toHaveCount(0);
	await expect(dialog.locator(".fixed-screen-pane-settings")).toHaveCount(0);
}

test("TL-472 @ui › Configure Screen shows its switches as a 2x2 grid and edits playbacks without a nested modal", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await desk.enableControllableDesktop();
	await desk.open(bench.baseUrl);
	api.session = await desk.session();
	await page.setViewportSize({ width: 1600, height: 900 });
	const before = (await screens(api)).map((screen) => screen.id);
	try {
		const { screenId, dialog, form } = await configureNewScreen(page);
		await expectNoInnerFrames(dialog, form);

		const [dock, playbacks, commandLine, programming] = await Promise.all(
			layoutSwitchFields(dialog).map(box),
		);
		// Row 1: Dock | Playbacks. Row 2: Command line | Programming.
		expect(Math.abs(dock.y - playbacks.y)).toBeLessThanOrEqual(1);
		expect(Math.abs(commandLine.y - programming.y)).toBeLessThanOrEqual(1);
		expect(commandLine.y).toBeGreaterThanOrEqual(dock.y + dock.height);
		expect(Math.abs(dock.x - commandLine.x)).toBeLessThanOrEqual(1);
		expect(Math.abs(playbacks.x - programming.x)).toBeLessThanOrEqual(1);
		expect(playbacks.x).toBeGreaterThanOrEqual(dock.x + dock.width);
		// Page controls follows the grid across the full form width.
		const pageControls = await box(switchField(dialog, "Page controls"));
		expect(pageControls.y).toBeGreaterThanOrEqual(
			programming.y + programming.height,
		);
		expect(pageControls.width).toBeGreaterThan(dock.width * 1.5);

		await dialog.getByRole("tab", { name: "Settings", exact: true }).click();
		await expect(form).toHaveAttribute("data-tab", "settings");
		await expect(form.locator(":scope > .screen-settings-note")).toHaveText(
			"This screen follows the selected Desktop layout.",
		);
		await expectNoInnerFrames(dialog, form);

		await dialog.getByRole("tab", { name: "Playbacks", exact: true }).click();
		await expect(page.getByRole("dialog")).toHaveCount(1);
		await expect(
			dialog.getByRole("button", { name: "Configure Playbacks", exact: true }),
		).toHaveCount(0);
		const addRow = dialog
			.locator(".ui-title-chrome-group")
			.getByRole("button", { name: "Add Row", exact: true });
		await expect(addRow).toBeVisible();
		const rows = dialog.locator("[data-playback-row-index]");
		const rowCount = await rows.count();
		await addRow.click();
		await expect(rows).toHaveCount(rowCount + 1);
		await expect
			.poll(
				async () =>
					(await screens(api)).find((screen) => screen.id === screenId)
						?.playback_layout?.rows.length,
			)
			.toBe(rowCount + 1);
		await dialog.getByRole("button", { name: "Follow Main" }).click();
		await page.getByRole("option", { name: "Dedicated Page" }).click();
		await expect
			.poll(
				async () =>
					(await screens(api)).find((screen) => screen.id === screenId)
						?.page_mode,
			)
			.toBe("independent");
		// Touch-sized row controls inside the embedded form.
		const remove = await box(
			rows.last().getByRole("button", { name: /Remove row/ }),
		);
		expect(remove.height).toBeGreaterThanOrEqual(44);
		expect(remove.width).toBeGreaterThanOrEqual(44);
	} finally {
		await deleteScreens(api, before);
	}
});

test("TL-472 @ui › a constrained Configure Screen stacks, scrolls, and keeps typed values", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await desk.enableControllableDesktop();
	await desk.open(bench.baseUrl);
	api.session = await desk.session();
	await page.setViewportSize({ width: 1600, height: 900 });
	const before = (await screens(api)).map((screen) => screen.id);
	try {
		const { screenId, dialog, form } = await configureNewScreen(page);
		await page.setViewportSize({ width: 520, height: 520 });

		const fields = layoutSwitchFields(dialog);
		const boxes = await Promise.all(fields.map(box));
		// One column: shared left edge, each switch below the previous one.
		for (const [index, field] of boxes.entries()) {
			expect(Math.abs(field.x - boxes[0].x)).toBeLessThanOrEqual(1);
			if (index > 0)
				expect(field.y).toBeGreaterThanOrEqual(
					boxes[index - 1].y + boxes[index - 1].height - 1,
				);
		}
		// The form scrolls vertically and never sideways; the last switch is reachable.
		expect(
			await form.evaluate(
				(element) => element.scrollWidth - element.clientWidth,
			),
		).toBeLessThanOrEqual(0);
		expect(
			await form.evaluate(
				(element) => element.scrollHeight > element.clientHeight,
			),
		).toBe(true);
		const pageControls = switchField(dialog, "Page controls");
		await form.evaluate((element) => {
			element.scrollTop = element.scrollHeight;
		});
		expect(await form.evaluate((element) => element.scrollTop)).toBeGreaterThan(
			0,
		);
		if (process.env.TL472_CAPTURE)
			await page.screenshot({ path: process.env.TL472_CAPTURE });
		await expect(pageControls).toBeInViewport();

		await dialog.getByRole("tab", { name: "Placement", exact: true }).click();
		const windowX = dialog.getByLabel("Window X");
		await windowX.fill("140");
		await windowX.blur();

		await dialog.getByRole("tab", { name: "Playbacks", exact: true }).click();
		const perRow = dialog.getByLabel("Playbacks per row");
		const rows = dialog.locator("[data-playback-row-index]");
		const alert = dialog.getByRole("alert");
		await perRow.fill("32");
		await perRow.blur();
		// A new screen's rows of 32 run past playback 127: kept on screen, not saved.
		await expect(alert).toContainText("Not saved yet");
		const rowCount = await rows.count();
		expect(
			await form.evaluate(
				(element) => element.scrollWidth - element.clientWidth,
			),
		).toBeLessThanOrEqual(0);

		await dialog.getByRole("tab", { name: "Placement", exact: true }).click();
		await expect(windowX).toHaveValue("140");
		await dialog.getByRole("tab", { name: "Playbacks", exact: true }).click();
		await expect(perRow).toHaveValue("32");
		await expect(rows).toHaveCount(rowCount);
		await expect(alert).toBeVisible();
		while (await alert.isVisible())
			await rows
				.last()
				.getByRole("button", { name: /Remove row/ })
				.click();
		const kept = await rows.count();
		expect(kept).toBeGreaterThan(0);
		await expect
			.poll(async () => {
				const stored = (await screens(api)).find(
					(screen) => screen.id === screenId,
				);
				return {
					perRow: stored?.playback_layout?.playbacks_per_row,
					rows: stored?.playback_layout?.rows.length,
					x: stored?.bounds?.x,
				};
			})
			.toEqual({ perRow: 32, rows: kept, x: 140 });

		// Back at desktop width the grid returns without losing anything.
		await page.setViewportSize({ width: 1600, height: 900 });
		await expect(perRow).toHaveValue("32");
		await dialog.getByRole("tab", { name: "Layout", exact: true }).click();
		const [dock, playbacks] = await Promise.all(fields.slice(0, 2).map(box));
		expect(Math.abs(dock.y - playbacks.y)).toBeLessThanOrEqual(1);
	} finally {
		await deleteScreens(api, before);
	}
});
