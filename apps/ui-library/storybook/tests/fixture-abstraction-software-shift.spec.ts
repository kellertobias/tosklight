import { expect, test, type Page } from "@playwright/test";

// TL-577: the lit software SHIFT key is the same modifier the fixture-independent Color controls
// use for ordered range endpoints. Every contact here is an on-screen touch; no keyboard Shift.
const story = "/iframe.html?id=tosklight-design-fixture-independent-programming--easy-rgbw&viewMode=story";

test.use({ hasTouch: true, viewport: { width: 1496, height: 761 } });

async function openEasyRgbw(page: Page) {
	await page.goto(story);
	await expect(page.getByTestId("fixture-abstraction-mockup")).toBeVisible();
	await page.evaluate(() => document.fonts.ready);
	const workspace = await page.getByTestId("existing-workspace").innerHTML();
	return async () => expect(page.getByTestId("existing-workspace")).toHaveJSProperty("innerHTML", workspace);
}
function shiftKey(page: Page) { return page.locator('[data-keypad-key="SHIFT"]').first(); }
async function tapShift(page: Page, armed: boolean) {
	await shiftKey(page).tap();
	// Lamp and behavior come from one desk state: the lit key is the armed modifier.
	if (armed) {
		await expect(shiftKey(page)).toHaveClass(/active/u);
		await expect(shiftKey(page)).toHaveAttribute("aria-pressed", "true");
	} else {
		await expect(shiftKey(page)).not.toHaveClass(/active/u);
		await expect(shiftKey(page)).not.toHaveAttribute("aria-pressed", "true");
	}
}
function slider(page: Page, name: string) { return page.getByRole("slider", { name, exact: true }); }
async function touchFader(page: Page, name: string, fraction: number) {
	const bounds = await slider(page, name).boundingBox();
	expect(bounds).not.toBeNull();
	await page.touchscreen.tap(bounds!.x + bounds!.width * fraction, bounds!.y + bounds!.height / 2);
}
async function touchPicker(page: Page, hue: number, saturation: number) {
	const bounds = await page.getByTestId("color-picker").boundingBox();
	expect(bounds).not.toBeNull();
	await page.touchscreen.tap(bounds!.x + bounds!.width * hue / 359, bounds!.y + bounds!.height * (1 - saturation / 100));
}
async function openCompactColor(page: Page) {
	await page.getByRole("button", { name: "Special Dialog", exact: true }).tap();
	await expect(page.locator(".fam-inline-dialog")).toBeVisible();
}
/** The full Color modal shows the per-fixture results on its Details tab. */
async function showDetails(page: Page) {
	const tab = page.getByRole("dialog", { name: "Color Special Dialog", exact: true }).getByRole("tab", { name: "Details", exact: true });
	await tab.tap();
	await expect(tab).toHaveAttribute("aria-selected", "true");
}
async function selectionValues(page: Page, attribute: string) {
	return page.locator("[data-testid^='selection-color-']").evaluateAll(
		(rows, key) => rows.map(row => Number(row.getAttribute(`data-${key}`))), attribute);
}
async function expectSpread(page: Page, attribute: string, expected: number[], digits = 2) {
	await expect.poll(async () => (await selectionValues(page, attribute)).length).toBe(expected.length);
	const actual = await selectionValues(page, attribute);
	for (const [index, value] of expected.entries()) expect(actual[index], `Fixture ${index + 1} ordered ${attribute}`).toBeCloseTo(value, digits);
}

test("software SHIFT arms ordered endpoints for White Blend, Temperature/Duv and the 2D picker and disarming clears them", async ({ page }) => {
	const workspaceUnchanged = await openEasyRgbw(page);
	await openCompactColor(page);
	const keypadBefore = await shiftKey(page).boundingBox();

	// White Blend: first contact is the pending endpoint, second contact completes the ordered range.
	await tapShift(page, true);
	await touchFader(page, "White Blend", .2);
	await expect(slider(page, "White Blend")).toHaveAttribute("aria-valuenow", "20");
	await expect(page.getByText("Shift-click the last value", { exact: true })).toBeVisible();
	await expect(shiftKey(page)).toHaveClass(/active/u);
	await touchFader(page, "White Blend", .8);
	await expect(slider(page, "White Blend")).toHaveAttribute("aria-valuetext", "20% through 80%");
	await expect(page.getByLabel("White Blend value", { exact: true })).toHaveText("20% → 80%");
	await expect(page.getByText("Shift-click the last value", { exact: true })).toHaveCount(0);

	// Temperature and Duv on the White balance page, still under the same lit SHIFT.
	await page.getByRole("button", { name: "Switch to White balance", exact: true }).tap();
	await touchFader(page, "Temperature", 2000 / 19000);
	await expect(slider(page, "Temperature")).toHaveAttribute("aria-valuenow", "3000");
	await expect(page.getByText("Shift-click the last value", { exact: true })).toBeVisible();
	await touchFader(page, "Temperature", 8000 / 19000);
	await expect(slider(page, "Temperature")).toHaveAttribute("aria-valuetext", "3000 K through 9000 K");
	await touchFader(page, "Duv", .25);
	await touchFader(page, "Duv", .75);
	await expect(slider(page, "Duv")).toHaveAttribute("aria-valuetext", "-0.0150 through +0.0150");

	// The compact 2D picker: first touch shows marker "1" and the pending caption, second completes.
	await page.getByRole("button", { name: "Switch to Color", exact: true }).tap();
	await touchPicker(page, 300, 80);
	await expect(page.getByText("Shift-click the last color", { exact: true })).toBeVisible();
	await expect(page.locator(".fam-plane-marker")).toHaveText(["1"]);
	await touchPicker(page, 60, 40);
	await expect(page.locator(".fam-plane-marker")).toHaveText(["1", "2"]);
	await expect(page.getByText("Shift-click the last color", { exact: true })).toHaveCount(0);

	// Expand shows each ordered fixture's value, including the intermediate fixtures.
	await page.locator(".fam-inline-dialog").getByRole("button", { name: "Expand", exact: true }).tap();
	await expect(page.getByRole("dialog", { name: "Color Special Dialog", exact: true })).toHaveAttribute("aria-modal", "true");
	await showDetails(page);
	await expectSpread(page, "white", [20, 40, 60, 80]);
	await expectSpread(page, "temperature", [3000, 5000, 7000, 9000]);
	await expectSpread(page, "duv", [-.015, -.005, .005, .015], 5);
	await expectSpread(page, "hue", [300, 340, 20, 60], 0);
	await expectSpread(page, "saturation", [80, 66.67, 53.33, 40], 0);
	await page.getByRole("tab", { name: "Color", exact: true }).tap();
	await expect(slider(page, "Hue")).toHaveAttribute("aria-valuetext", "300 through 60 degrees");
	await expect(slider(page, "Saturation")).toHaveAttribute("aria-valuetext", "80% through 40%");

	// The expanded modal covers the keypad; closing it keeps the modifier lit and armed.
	await page.keyboard.press("Escape");
	await expect(shiftKey(page)).toHaveClass(/active/u);

	// Disarming: SHIFT unlit, and an ordinary contact replaces each range with one scalar value.
	await tapShift(page, false);
	await openCompactColor(page);
	await touchFader(page, "White Blend", .5);
	await expect(slider(page, "White Blend")).toHaveAttribute("aria-valuetext", "50%");
	await expect(page.getByText("Shift-click the last value", { exact: true })).toHaveCount(0);
	await touchPicker(page, 120, 60);
	await expect(page.locator(".fam-plane-marker")).toHaveText([""]);
	await expect(page.getByText("Shift-click the last color", { exact: true })).toHaveCount(0);
	await page.getByRole("button", { name: "Switch to White balance", exact: true }).tap();
	await touchFader(page, "Temperature", 5000 / 19000);
	await expect(slider(page, "Temperature")).toHaveAttribute("aria-valuetext", "6000 K");
	await touchFader(page, "Duv", .5);
	await expect(slider(page, "Duv")).toHaveAttribute("aria-valuetext", "0.0000");
	await page.locator(".fam-inline-dialog").getByRole("button", { name: "Expand", exact: true }).tap();
	await showDetails(page);
	await expectSpread(page, "white", [50, 50, 50, 50]);
	await expectSpread(page, "temperature", [6000, 6000, 6000, 6000]);
	await expectSpread(page, "duv", [0, 0, 0, 0], 5);
	await expectSpread(page, "hue", [120, 120, 120, 120], 0);
	await expectSpread(page, "saturation", [60, 60, 60, 60], 0);

	// Arming changes lamp state only; the software keypad keeps its geometry.
	await page.keyboard.press("Escape");
	await tapShift(page, true);
	expect(await shiftKey(page).boundingBox()).toEqual(keypadBefore);
	await tapShift(page, false);
	await workspaceUnchanged();
});

test("keyboard Shift still sets ordered endpoints while the software SHIFT stays unlit", async ({ page }) => {
	const workspaceUnchanged = await openEasyRgbw(page);
	await openCompactColor(page);
	await expect(shiftKey(page)).not.toHaveClass(/active/u);
	const bounds = await slider(page, "White Blend").boundingBox();
	expect(bounds).not.toBeNull();
	await page.keyboard.down("Shift");
	await page.mouse.click(bounds!.x + bounds!.width * .3, bounds!.y + bounds!.height / 2);
	await page.mouse.click(bounds!.x + bounds!.width * .7, bounds!.y + bounds!.height / 2);
	await page.keyboard.up("Shift");
	await expect(slider(page, "White Blend")).toHaveAttribute("aria-valuetext", "30% through 70%");
	await expect(shiftKey(page)).not.toHaveClass(/active/u);
	// Without either modifier an ordinary touch is a scalar edit.
	await touchFader(page, "White Blend", .6);
	await expect(slider(page, "White Blend")).toHaveAttribute("aria-valuetext", "60%");
	await workspaceUnchanged();
});
