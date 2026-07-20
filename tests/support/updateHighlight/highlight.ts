import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import type {
	Locator,
	Page,
} from "../../../apps/control-ui/node_modules/@playwright/test/index.js";
import { objects, programmer } from "../catalog";

export interface HighlightFixture {
	id: string;
	number: number;
}

export function escapeRegex(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export async function openBuiltIn(page: Page, name: string): Promise<void> {
	const entry = page.locator(".dock-entry").filter({ hasText: name }).first();
	if (!(await entry.isVisible()))
		await page.getByRole("button", { name: "BUILT-INS", exact: true }).click();
	await expect(entry).toBeVisible();
	await entry.click();
}

export async function openGroups(page: Page): Promise<void> {
	await page.locator('[data-keypad-key="SHIFT"]').click();
	await page.locator('[data-keypad-key="1"]').click();
	await expect(page.locator(".group-pool-window")).toBeVisible();
}

export function fixtureSheetRow(page: Page, number: number) {
	return page
		.locator(".fixture-window .ui-data-table-row:not(.header)")
		.filter({
			has: page.getByRole("cell", { name: String(number), exact: true }),
		})
		.first();
}

export function fixtureSheetRowById(page: Page, fixtureId: string) {
	return page
		.locator(
			`.fixture-window .ui-data-table-row[data-fixture-id="${fixtureId}"]`,
		)
		.first();
}

export async function storeCurrentProgrammerPreset(
	api: Parameters<typeof objects>[0],
	showId: string,
	presetId: string,
) {
	const programmers = await api.request<any[]>(
		"GET",
		"/api/v1/programmers",
	);
	const current = programmers.find(
		(entry) => entry.session_id === api.session!.session_id,
	);
	expect(current).toBeDefined();
	const values: Record<string, Record<string, unknown>> = {};
	for (const entry of current.values ?? []) {
		const fixtureValues = values[entry.fixture_id] ?? {};
		fixtureValues[entry.attribute] = entry.value;
		values[entry.fixture_id] = fixtureValues;
	}
	await api.request(
		"POST",
		`/api/v1/shows/${showId}/presets/${presetId}/store`,
		{
			mode: "overwrite",
			preset: {
				name: "Highlight isolation",
				family: "Mixed",
				values,
				group_values: {},
			},
		},
		true,
		0,
	);
}

export async function fixturesByNumber(
	api: Parameters<typeof objects>[0],
	numbers: number[],
): Promise<HighlightFixture[]> {
	const patched = await objects<any>(api, "patched_fixture");
	const byNumber = new Map<number, HighlightFixture>(
		patched.map((entry) => [
			Number(entry.body.fixture_number),
			{
				id: entry.body.fixture_id as string,
				number: Number(entry.body.fixture_number),
			},
		]),
	);
	return numbers.map((number) => {
		const fixture = byNumber.get(number);
		expect(
			fixture,
			`Fixture ${number} must exist in default-stage.show`,
		).toBeDefined();
		return fixture!;
	});
}

export function fixtureIds(fixtures: HighlightFixture[]): string[] {
	return fixtures.map((fixture) => fixture.id);
}

export function selectionsEqual(actual: string[], expected: string[]): boolean {
	return JSON.stringify(actual) === JSON.stringify(expected);
}

export function groupBody(name: string, fixtures: string[]) {
	return {
		derived_from: null,
		fixtures,
		frozen_from: null,
		master: 1,
		name,
		playback_fader: null,
		programming: {},
	};
}

export async function highlightState(
	api: Parameters<typeof objects>[0],
): Promise<any> {
	return api.request<any>("GET", "/api/v1/highlight", undefined, true);
}

export async function highlightAction(
	api: Parameters<typeof objects>[0],
	action: "on" | "off" | "toggle" | "previous" | "next" | "all",
): Promise<void> {
	await api.request("POST", "/api/v1/highlight/action", { action });
	// The shared hardware/software repeat guard intentionally rejects duplicate
	// physical presses inside 150 ms. Acceptance actions model distinct presses.
	await new Promise((resolve) => setTimeout(resolve, 175));
}

export async function expectSelection(
	api: Parameters<typeof objects>[0],
	expected: string[],
): Promise<void> {
	await expect
		.poll(async () => (await programmer(api)).selected)
		.toEqual(expected);
}

export function highlightKey(
	page: Page,
	key: "HIGH" | "PREV" | "NEXT" | "ALL",
) {
	const fallback = {
		HIGH: ".highlight-toggle",
		PREV: ".highlight-previous",
		NEXT: ".highlight-next",
		ALL: ".highlight-all",
	}[key];
	return page.locator(`[data-keypad-key="${key}"], ${fallback}`).first();
}

export async function clickHighlightKey(
	page: Page,
	api: Parameters<typeof objects>[0],
	key: "HIGH" | "PREV" | "NEXT" | "ALL",
	expectedSelection?: string[],
): Promise<void> {
	const button = highlightKey(page, key);
	await expect(button).toBeVisible();
	await expect(button).toBeEnabled();
	await button.click();
	if (expectedSelection) await expectSelection(api, expectedSelection);
	await page.waitForTimeout(175);
}

export async function restoreSecondStep(
	api: Parameters<typeof objects>[0],
): Promise<void> {
	await highlightAction(api, "all");
	await highlightAction(api, "next");
	await highlightAction(api, "next");
}

export async function setPanThroughUi(
	page: Page,
	percent: number,
): Promise<void> {
	await page.getByRole("button", { name: "Position", exact: true }).click();
	const encoder = page
		.locator(".vertical-touch-fader-stack")
		.filter({ hasText: "Enc 1 · Pan" });
	await expect(encoder).toBeVisible();
	await encoder.getByRole("button", { name: "Set value" }).click();
	const dialog = page.getByRole("dialog", { name: "Enc 1 · Pan value" });
	await expect(dialog).toBeVisible();
	await page.keyboard.type(String(percent));
	await page.keyboard.press("Enter");
	await expect(dialog).toBeHidden();
}

export async function assertFixtureSheetStep(
	page: Page,
	fixtures: HighlightFixture[],
	activeNumber: number,
): Promise<void> {
	for (const fixture of fixtures) {
		const row = fixtureSheetRowById(page, fixture.id);
		await expect(row).toBeVisible();
		await expect(row).toHaveAttribute(
			"data-step-selection",
			fixture.number === activeNumber ? "active" : "base",
		);
	}
}

export async function verifyProgrammerKeypadGeometry(
	page: Page,
	api: Parameters<typeof objects>[0],
): Promise<void> {
	const upperNames = ["HIGH", "PREV", "NEXT", "ALL"] as const;
	const lowerNames = ["GRP", "CUE", "TIME", "DIV"] as const;
	const upper = await Promise.all(
		upperNames.map(async (name) => {
			const locator = highlightKey(page, name);
			await expect(locator).toHaveText(name);
			const box = await locator.boundingBox();
			expect(box).toBeTruthy();
			return { locator, box: box! };
		}),
	);
	const lower = await Promise.all(
		lowerNames.map(async (name) => {
			const locator = page.locator(`[data-keypad-key="${name}"]`);
			const box = await locator.boundingBox();
			expect(box).toBeTruthy();
			return { locator, box: box! };
		}),
	);

	const tolerance = 1.5;
	for (let index = 0; index < upper.length; index += 1) {
		expect(
			Math.abs(centerX(upper[index].box) - centerX(lower[index].box)),
		).toBeLessThanOrEqual(tolerance);
		expect(
			Math.abs(upper[index].box.width - lower[index].box.width),
		).toBeLessThanOrEqual(tolerance);
		expect(
			Math.abs(upper[index].box.height - lower[index].box.height),
		).toBeLessThanOrEqual(tolerance);
		expect(upper[index].box.y + upper[index].box.height).toBeLessThanOrEqual(
			lower[index].box.y,
		);
	}
	const upperY = centerY(upper[0].box);
	const lowerY = centerY(lower[0].box);
	for (const item of upper)
		expect(Math.abs(centerY(item.box) - upperY)).toBeLessThanOrEqual(tolerance);
	for (const item of lower)
		expect(Math.abs(centerY(item.box) - lowerY)).toBeLessThanOrEqual(tolerance);

	const keyStyles = await Promise.all(
		upper.map(({ locator }) =>
			locator.evaluate((element) => {
				const style = getComputedStyle(element);
				return {
					alignItems: style.alignItems,
					borderRadius: style.borderRadius,
					fontFamily: style.fontFamily,
					fontSize: style.fontSize,
					justifyContent: style.justifyContent,
					textAlign: style.textAlign,
				};
			}),
		),
	);
	expect(
		keyStyles.every(
			(style) => JSON.stringify(style) === JSON.stringify(keyStyles[0]),
		),
	).toBe(true);
	await expect(upper[0].locator).toHaveClass(/highlight-armed/);
	await clickHighlightKey(page, api, "HIGH");
	await expect.poll(async () => (await highlightState(api)).active).toBe(false);
	await expect(upper[0].locator).toHaveClass(/highlight-off/);

	const fade = page.locator(".numeric-pad-fade");
	await expect(fade).toHaveAttribute("data-grid-column-span", "2");
	await expect(fade).toHaveAttribute("data-grid-row-span", "2");
	const fadeBox = await fade.boundingBox();
	const delBox = await page.locator('[data-keypad-key="DEL"]').boundingBox();
	const clrBox = await page.locator('[data-keypad-key="CLR"]').boundingBox();
	const movBox = await page.locator('[data-keypad-key="MOV"]').boundingBox();
	expect(fadeBox && delBox && clrBox && movBox).toBeTruthy();
	expect(
		Math.abs(fadeBox!.width - (clrBox!.x + clrBox!.width - delBox!.x)),
	).toBeLessThanOrEqual(tolerance);
	expect(
		Math.abs(fadeBox!.height - (movBox!.y + movBox!.height - delBox!.y)),
	).toBeLessThanOrEqual(tolerance);
	const followingGap = delBox!.y - (fadeBox!.y + fadeBox!.height);
	const normalGap = clrBox!.x - (delBox!.x + delBox!.width);
	expect(Math.abs(followingGap - normalGap)).toBeLessThanOrEqual(tolerance);
}

export async function softwareHighlightGeometry(page: Page) {
	return Promise.all(
		[
			page.locator(".programmer-number-block"),
			highlightKey(page, "HIGH"),
			page.locator('[data-keypad-key="GRP"]'),
			page.locator(".global-store-button"),
			page.locator(".preload-button"),
		].map(async (locator) => {
			const box = await locator.boundingBox();
			expect(box).toBeTruthy();
			return roundedBox(box!);
		}),
	);
}

export async function hardwareHighlightGeometry(page: Page) {
	return Promise.all(
		[
			page.locator(".hardware-right-pane"),
			page.locator(".hardware-control-summary"),
			page.locator(".global-store-button"),
			page.locator(".preload-button"),
		].map(async (locator) => {
			const box = await locator.boundingBox();
			expect(box).toBeTruthy();
			return roundedBox(box!);
		}),
	);
}

export async function assertReachableAlert(
	page: Page,
	alert: Locator,
	modal: Locator,
	viewport: { width: number; height: number },
) {
	const box = await alert.boundingBox();
	expect(box).toBeTruthy();
	expect(box!.x).toBeGreaterThanOrEqual(0);
	expect(box!.y).toBeGreaterThanOrEqual(0);
	expect(box!.x + box!.width).toBeLessThanOrEqual(viewport.width);
	expect(box!.y + box!.height).toBeLessThanOrEqual(viewport.height);
	const topElementIsAlert = await page.evaluate(
		({ x, y }) => {
			const top = document.elementFromPoint(x, y);
			return Boolean(top?.closest("[data-highlight-error-alert]"));
		},
		{ x: centerX(box!), y: centerY(box!) },
	);
	expect(topElementIsAlert).toBe(true);
	const [alertZ, modalZ] = await Promise.all([
		alert.evaluate((element) => Number(getComputedStyle(element).zIndex) || 0),
		modal.evaluate((element) => {
			const layer =
				element.closest<HTMLElement>(".stacked-modal-layer") ?? element;
			return Number(getComputedStyle(layer).zIndex) || 0;
		}),
	]);
	expect(alertZ).toBeGreaterThan(modalZ);
}

export function roundedBox(box: {
	x: number;
	y: number;
	width: number;
	height: number;
}) {
	return Object.fromEntries(
		Object.entries(box).map(([key, value]) => [
			key,
			Math.round(value * 10) / 10,
		]),
	);
}

export function centerX(box: { x: number; width: number }): number {
	return box.x + box.width / 2;
}

export function centerY(box: { y: number; height: number }): number {
	return box.y + box.height / 2;
}

export async function operateProgrammerFade(
	page: Page,
	api: Parameters<typeof objects>[0],
): Promise<void> {
	const fade = page.locator(".numeric-pad-fade");
	const button = fade.getByRole("button", { name: /Prog\. Fade/ });
	await expect(button).toContainText("Prog. Fade");
	await expect(button).toContainText("s");
	await button.click();
	const dialog = page.getByRole("dialog", { name: "Prog. Fade value" });
	await expect(dialog).toBeVisible();
	await page.keyboard.type("4.2");
	await page.keyboard.press("Enter");
	await expect(dialog).toBeHidden();
	await expect
		.poll(async () => {
			const response = await api.request<any>(
				"GET",
				"/api/v1/configuration",
				undefined,
				false,
			);
			return response.configuration.programmer_fade_millis;
		})
		.toBe(4_200);
}
