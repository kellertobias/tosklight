import type { Locator, Page } from "@playwright/test";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";

const LONG_INFORMATION =
	"Keyboard shortcuts are always disabled while hardware controls are connected. " +
	"This deliberately long information text wraps over several lines so the row proves " +
	"that informational copy can never push, overlap, or misalign the neighbouring controls.";

type Box = { x: number; y: number; width: number; height: number };

async function box(locator: Locator): Promise<Box> {
	const result = await locator.boundingBox();
	if (!result) throw new Error("Element has no layout box");
	return result;
}

function overlaps(a: Box, b: Box) {
	return (
		a.x < b.x + b.width - 0.5 &&
		b.x < a.x + a.width - 0.5 &&
		a.y < b.y + b.height - 0.5 &&
		b.y < a.y + a.height - 0.5
	);
}

async function openScreensSetup(page: Page) {
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
	await page
		.locator(".setup-window nav")
		.getByRole("button", { name: "Screens & playback", exact: true })
		.click();
}

async function lengthenInformation(page: Page) {
	await page
		.locator(".default-screen-compact-row > .ui-form-field small")
		.first()
		.evaluate((element, text) => {
			element.textContent = text;
		}, LONG_INFORMATION);
}

/** Every cell of a row starts on one line and no two cells share any area. */
async function expectTopAlignedWithoutOverlap(cells: Locator) {
	const boxes: Box[] = [];
	for (const cell of await cells.all()) boxes.push(await box(cell));
	expect(boxes.length).toBeGreaterThan(1);
	const rows = new Map<number, Box[]>();
	for (const cell of boxes) {
		const key = [...rows.keys()].find((top) => Math.abs(top - cell.y) <= 1);
		rows.set(key ?? cell.y, [...(rows.get(key ?? cell.y) ?? []), cell]);
	}
	// Wrapped grid rows are fine; within each row every cell starts at the same top edge.
	for (const [top, row] of rows)
		for (const cell of row)
			expect(Math.abs(cell.y - top)).toBeLessThanOrEqual(1);
	for (const [index, cell] of boxes.entries())
		for (const other of boxes.slice(index + 1))
			expect(overlaps(cell, other)).toBe(false);
}

async function deleteScreens(api: ApiDriver, before: string[]) {
	const snapshot = await api.request<{ screens: Array<{ id: string }> }>(
		"GET",
		"/api/v2/screens",
	);
	for (const screen of snapshot.screens)
		if (!before.includes(screen.id))
			await api.request("POST", "/api/v2/screens/actions", {
				request_id: crypto.randomUUID(),
				action: { type: "delete", screen_id: screen.id },
			});
}

test("TL-471 @ui › the default screen row stays top-aligned with long information text at every width", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await desk.enableControllableDesktop();
	await desk.open(bench.baseUrl);
	api.session = await desk.session();
	await openScreensSetup(page);
	const row = page.locator(".default-screen-compact-row");
	await expect(row).toBeVisible();
	await lengthenInformation(page);

	for (const width of [1600, 1180, 900, 760]) {
		await page.setViewportSize({ width, height: 900 });
		const cells = row.locator(":scope > *");
		await expectTopAlignedWithoutOverlap(cells);
		// The information stays inside its own field and the row never scrolls sideways.
		const information = row.locator(".ui-form-field small").first();
		const field = row.locator(":scope > .ui-form-field").nth(1);
		const informationBox = await box(information);
		const fieldBox = await box(field);
		expect(informationBox.x + informationBox.width).toBeLessThanOrEqual(
			fieldBox.x + fieldBox.width + 1,
		);
		expect(
			await row.evaluate(
				(element) => element.scrollWidth - element.clientWidth,
			),
		).toBeLessThanOrEqual(0);
		// The actions start level with the field controls beside them, not with their labels.
		const deskName = await box(row.getByRole("textbox", { name: "Desk name" }));
		const configure = await box(
			row.getByRole("button", { name: "Configure Playbacks", exact: true }),
		);
		if (
			Math.abs(configure.x - deskName.x) > 1 &&
			configure.y < deskName.y + deskName.height
		)
			expect(Math.abs(configure.y - deskName.y)).toBeLessThanOrEqual(8);
		for (const name of ["Configure Playbacks", "Known windows"]) {
			const action = row.getByRole("button", { name, exact: true });
			await expect(action).toBeVisible();
			await expect(action).toBeInViewport();
		}
	}
});

test("TL-471 @ui › Add Screen is a separate Desk Setup title action that adds a top-aligned screen row", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await desk.enableControllableDesktop();
	await desk.open(bench.baseUrl);
	api.session = await desk.session();
	const before = (
		await api.request<{ screens: Array<{ id: string }> }>(
			"GET",
			"/api/v2/screens",
		)
	).screens.map((screen) => screen.id);
	try {
		await openScreensSetup(page);
		const header = page.locator(".setup-window .ui-window-header");
		const addScreen = header.getByRole("button", {
			name: "Add Screen",
			exact: true,
		});
		await expect(addScreen).toBeVisible();
		// The page body no longer carries its own add button.
		await expect(
			page
				.locator(".screens-playback-setup")
				.getByRole("button", { name: /add screen/i }),
		).toHaveCount(0);

		// Its own title group, visually apart from Undo and the configuration actions.
		const groups = header.locator(".ui-title-chrome-group");
		const addGroup = groups.filter({
			has: page.getByRole("button", { name: "Add Screen", exact: true }),
		});
		await expect(addGroup).toHaveCount(1);
		await expect(addGroup.getByRole("button")).toHaveCount(1);
		const configurationGroup = groups.filter({
			has: page.getByRole("button", { name: "Undo", exact: true }),
		});
		await expect(configurationGroup).toHaveCount(1);
		expect(overlaps(await box(addGroup), await box(configurationGroup))).toBe(
			false,
		);

		// Touch-sized in both a wide and a narrow Desk Setup window.
		for (const width of [1600, 900]) {
			await page.setViewportSize({ width, height: 900 });
			await expect(addScreen).toBeInViewport();
			const size = await box(addScreen);
			expect(size.height).toBeGreaterThanOrEqual(32);
			expect(size.width).toBeGreaterThanOrEqual(32);
		}

		const cards = page.locator(".screen-settings-card[data-screen-id]");
		const count = await cards.count();
		await addScreen.click();
		await expect(cards).toHaveCount(count + 1);
		const header_ = cards.nth(count).locator(".screen-settings-header");
		for (const width of [1600, 900]) {
			await page.setViewportSize({ width, height: 900 });
			await expectTopAlignedWithoutOverlap(header_.locator(":scope > *"));
			const actions = header_.locator(".screen-settings-actions > button");
			await expect(actions).toHaveCount(4);
			for (const action of await actions.all()) {
				await action.scrollIntoViewIfNeeded();
				await expect(action).toBeInViewport();
			}
		}
	} finally {
		await deleteScreens(api, before);
	}
});

test("TL-471 @ui › a browser desk without the desktop app shows no Add Screen title action", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await desk.open(bench.baseUrl);
	api.session = await desk.session();
	await openScreensSetup(page);
	const header = page.locator(".setup-window .ui-window-header");
	await expect(
		header.getByRole("button", { name: "Undo", exact: true }),
	).toBeVisible();
	await expect(
		header.getByRole("button", { name: "Add Screen", exact: true }),
	).toHaveCount(0);
	await expect(
		page.getByText(
			"Additional console screens are available in the ToskLight desktop app.",
		),
	).toBeVisible();
	await lengthenInformation(page);
	await expectTopAlignedWithoutOverlap(
		page.locator(".default-screen-compact-row > *"),
	);
});
