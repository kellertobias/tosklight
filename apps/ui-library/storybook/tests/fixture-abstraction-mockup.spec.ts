import { mkdirSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import resolver from "../../../../tools/artifact-paths.cjs";

const prefix = "tosklight-design-fixture-independent-programming--";
const stories = [
	"easy-rgbw", "easy-rgbwauv", "advanced-color", "mixed-selection-magenta",
	"mixed-selection-warm-white", "position-angles", "position-fixed-target",
	"position-tracked-target", "fixture-configuration", "beam-and-shutter", "media-color",
];
const viewports = [{ width: 1496, height: 761 }, { width: 1024, height: 768 }, { width: 760, height: 900 }];
const shots = `${resolver.artifactPaths.visual}/fixture-abstraction-mockup/operator-controls`;
mkdirSync(shots, { recursive: true });
const workspaceSnapshots = new WeakMap<Page, string>();

async function openStory(page: Page, name: string, hardware = false, args = "") {
	const previous = workspaceSnapshots.get(page);
	if (previous !== undefined) await expect(page.getByTestId("existing-workspace")).toHaveJSProperty("innerHTML", previous);
	await page.goto(`/iframe.html?id=${prefix}${name}&viewMode=story${hardware ? "&globals=mode:hardware" : ""}${args ? `&args=${args}` : ""}`);
	await expect(page.getByTestId("fixture-abstraction-mockup")).toBeVisible();
	await page.evaluate(() => document.fonts.ready);
	await expect(page.getByTestId("existing-workspace")).toBeVisible();
	workspaceSnapshots.set(page, await page.getByTestId("existing-workspace").innerHTML());
}

// Programming never changes the production workspace above the encoder area.
test.afterEach(async ({ page }) => {
	const original = workspaceSnapshots.get(page);
	if (original !== undefined && !page.isClosed()) await expect(page.getByTestId("existing-workspace")).toHaveJSProperty("innerHTML", original);
});

function family(page: Page, name: string) {
	return page.getByRole("button", { name: new RegExp(`^${name}(?: \\d of \\d)?$`) });
}
function touchValue(page: Page, slot: number, attribute: string) {
	return page.getByRole("button", { name: `Set Enc ${slot} · ${attribute} value`, exact: true });
}
async function setEncoder(page: Page, slot: number, attribute: string, value: number, hardware = false) {
	const control = page.getByRole("button", { name: hardware ? new RegExp(`^Encoder ${slot}: `) : new RegExp(`^Set Enc ${slot} · .+ value$`) });
	await expect(control, `${attribute} must be editable in encoder ${slot}`).toBeVisible();
	await control.click();
	await expect(page.locator('.ui-modal-stack-layer[data-modal-top="true"]')).toBeVisible();
	await page.keyboard.type(String(value));
	await page.keyboard.press("Enter");
	await expect(page.locator('.ui-modal-stack-layer[data-modal-top="true"]')).toHaveCount(0);
}
async function openSpecial(page: Page) {
	const trigger = page.getByRole("button", { name: "Special Dialog", exact: true });
	await expect(trigger).toBeVisible();
	await trigger.click();
	const dialog = page.getByRole("dialog", { name: /Special Dialog$/ });
	await expect(dialog).toBeVisible();
	if (await dialog.evaluate(element => Boolean(element.closest(".ui-modal-stack-layer")))) {
		// ModalStack first renders inert, then registers and hands off focus on the next frame,
		// to the first title-bar control (the Color modal's first tab, otherwise the close button).
		await expect(dialog).toHaveAttribute("aria-modal", "true");
		await expect.poll(() => dialog.evaluate(element => element.contains(document.activeElement)), "Focus moves into the modal").toBe(true);
	}
}
async function closeSpecial(page: Page, activeFamily = "Color") {
	const dialog = page.getByRole("dialog", { name: /Special Dialog$/ });
	if (await dialog.getAttribute("aria-modal") === "true") await page.keyboard.press("Escape");
	else await family(page, activeFamily).click();
	await expect(dialog).toHaveCount(0);
}
async function expandColor(page: Page) {
	const dialog = page.getByRole("dialog", { name: "Color Special Dialog", exact: true });
	const expand = dialog.getByRole("button", { name: "Expand", exact: true });
	if (await expand.count()) await expand.click();
	await expect(dialog).toHaveAttribute("aria-modal", "true");
}
/** The full Color modal shows its controls on the Color tab and the per-fixture results on Details. */
async function showColorTab(page: Page, name: "Color" | "Details" | "Preview") {
	const tab = page.getByRole("dialog", { name: "Color Special Dialog", exact: true }).getByRole("tab", { name, exact: true });
	if (!await tab.count()) return;
	if (await tab.getAttribute("aria-selected") !== "true") await tab.click();
	await expect(tab).toHaveAttribute("aria-selected", "true");
}
function slider(page: Page, name: string) { return page.getByRole("slider", { name, exact: true }); }
async function clickFader(page: Page, name: string, fraction: number, shift = false) {
	await showColorTab(page, "Color");
	const control = slider(page, name), bounds = await control.boundingBox();
	expect(bounds).not.toBeNull();
	if (shift) await page.keyboard.down("Shift");
	await page.mouse.click(bounds!.x + Math.max(1, Math.min(bounds!.width - 1, bounds!.width * fraction)), bounds!.y + bounds!.height / 2);
	if (shift) await page.keyboard.up("Shift");
}
async function dragFader(page: Page, name: string, to: "minimum" | "maximum") {
	await showColorTab(page, "Color");
	const control = slider(page, name), bounds = await control.boundingBox();
	expect(bounds).not.toBeNull();
	await page.mouse.move(bounds!.x + bounds!.width / 2, bounds!.y + bounds!.height / 2);
	await page.mouse.down();
	await page.mouse.move(to === "minimum" ? bounds!.x - 2 : bounds!.x + bounds!.width + 2, bounds!.y + bounds!.height / 2, { steps: 8 });
	await page.mouse.up();
	await expect(control).toHaveAttribute("aria-valuenow", (await control.getAttribute(to === "minimum" ? "aria-valuemin" : "aria-valuemax"))!);
}
async function clickHue(page: Page, degrees: number, shift = false) {
	await showColorTab(page, "Color");
	const ring = slider(page, "Hue"), bounds = await ring.boundingBox();
	expect(bounds).not.toBeNull();
	const radians = degrees * Math.PI / 180;
	await ring.click({ position: { x: bounds!.width * (.5 + Math.sin(radians) * .42), y: bounds!.height * (.5 - Math.cos(radians) * .42) }, modifiers: shift ? ["Shift"] : [] });
}
async function choosePoint(page: Page, name: string) {
	await touchValue(page, 1, "Point").click();
	await page.locator('.ui-modal-stack-layer[data-modal-top="true"]').getByRole("button", { name, exact: true }).click();
	await expect(page.locator('.ui-modal-stack-layer[data-modal-top="true"]')).toHaveCount(0);
}
function processedMedia(page: Page) { return page.getByRole("img", { name: "Processed media test card", exact: true }); }
async function mediaTileColors(page: Page) {
	await openSpecial(page);
	const preview = page.getByRole("button", { name: "Switch to Preview", exact: true });
	if (await preview.count()) await preview.click();
	const colors = await processedMedia(page).locator("rect[fill^='rgb(']").evaluateAll(tiles => tiles.map(tile => (tile.getAttribute("fill") ?? "").match(/\d+/g)?.map(Number) ?? []));
	await expect(processedMedia(page).getByTestId("media-black-tile")).toHaveAttribute("fill", "rgb(0,0,0)");
	await closeSpecial(page);
	return colors;
}
async function expectNoExtraOperatorControls(page: Page) {
	await expect(page.getByTestId("programming-tools")).toHaveCount(0);
	await expect(page.getByTestId("editor-tools")).toHaveCount(0);
	await expect(page.getByRole("button", { name: /^(?:Return to encoders|Fixture details|Position tools|Layer tools|Store .* preset|Recall .* preset)/ })).toHaveCount(0);
	await expect(page.getByRole("dialog", { name: "Programming details", exact: true })).toHaveCount(0);
	await expect(page.locator(".fam-context, .fam-color-editor-footer")).toHaveCount(0);
}
async function expectEditorFits(page: Page, title: string) {
	const dialog = page.getByRole("dialog", { name: `${title} Special Dialog`, exact: true });
	await expect(dialog).toBeInViewport({ ratio: 1 });
	if (title === "Position") {
		await expect(dialog.getByRole("button", { name: "Aim reference", exact: true })).toHaveCount(0);
		await expect(dialog.locator(".fam-target-controls")).toHaveCount(0);
	}
	if (title === "Position" || title === "Focus") {
		await expect(dialog).toHaveAttribute("aria-modal", "true");
		await expect(dialog.getByRole("button", { name: "Expand", exact: true })).toHaveCount(0);
		await expect(dialog.getByTestId("moving-fixture-model")).toHaveCount(0);
		await expect(dialog.locator("canvas")).toHaveCount(0);
		await expectFlushTitle(page, title);
		if (title === "Position") {
			const pan = await slider(page, "Pan circle").boundingBox();
			const tilt = await slider(page, "Tilt angle").locator("..").boundingBox();
			const aim = await page.getByTestId("position-joystick").boundingBox();
			expect(Math.abs(aim!.width - aim!.height), "The Aim surface must be square").toBeLessThanOrEqual(1);
			expect(tilt!.y, "Tilt must sit below Pan").toBeGreaterThanOrEqual(pan!.y + pan!.height);
			expect(aim!.x, "Aim must be beside the Pan and Tilt stack").toBeGreaterThan(pan!.x + pan!.width);
			expect(aim!.x).toBeGreaterThan(tilt!.x + tilt!.width);
		}
	}
	const content = dialog.getByTestId("editor-page");
	await expect(content).toBeVisible();
	const overflow = await content.evaluate(element => ({ width: element.scrollWidth - element.clientWidth, height: element.scrollHeight - element.clientHeight }));
	expect(overflow.width, "The active editor must fit without horizontal scrolling").toBeLessThanOrEqual(1);
	expect(overflow.height, "The active editor must fit without vertical scrolling").toBeLessThanOrEqual(1);
	const controls = dialog.locator("button:visible, input:visible, [role=slider]:visible, [role=application]:visible, [role=img]:visible, output:visible, .fam-result-swatch:visible, .fam-color-matches b:visible, .fam-color-matches small:visible, .fam-match-status:visible");
	const clipping = await controls.evaluateAll(elements => elements.flatMap(element => {
		const bounds = element.getBoundingClientRect();
		let ancestor = element.parentElement;
		while (ancestor) {
			const style = getComputedStyle(ancestor), parent = ancestor.getBoundingClientRect();
			if ((/hidden|auto|scroll|clip/.test(style.overflowX) && (bounds.left < parent.left - 1 || bounds.right > parent.right + 1)) ||
				(/hidden|auto|scroll|clip/.test(style.overflowY) && (bounds.top < parent.top - 1 || bounds.bottom > parent.bottom + 1))) {
				return [`${element.getAttribute("aria-label") ?? element.textContent}: clipped by ${ancestor.className}`];
			}
			ancestor = ancestor.parentElement;
		}
		return [];
	}));
	expect(clipping, "Every control and preview must be wholly visible").toEqual([]);
	if (title === "Focus") {
		const overlap = await dialog.getByRole("group", { name: "Beam angle and focus diagram", exact: true }).evaluate(diagram => {
			const labels = [...diagram.querySelectorAll("text")], handles = [...diagram.querySelectorAll(".fam-guide-handle")];
			return labels.flatMap(label => {
				const text = label.getBoundingClientRect();
				return handles.some(handle => {
					const bounds = handle.getBoundingClientRect();
					return Math.min(text.right, bounds.right) - Math.max(text.left, bounds.left) > 1 && Math.min(text.bottom, bounds.bottom) - Math.max(text.top, bounds.top) > 1;
				}) ? [label.textContent] : [];
			});
		});
		expect(overlap, "Focus diagram text must not overlap the draggable beam handles").toEqual([]);
		const readable = await dialog.getByRole("group", { name: "Beam angle and focus diagram", exact: true }).evaluate(diagram => {
			const edge = diagram.querySelector<SVGGraphicsElement>(".fam-beam-angle-hit")!;
			const matrix = edge.getScreenCTM()!;
			return {
				labelHeights: [...diagram.querySelectorAll("text")].map(label => label.getBoundingClientRect().height),
				angleHit: Number.parseFloat(getComputedStyle(edge).strokeWidth) * Math.hypot(matrix.a, matrix.b),
				focusHit: diagram.querySelector(".fam-focus-hit")!.getBoundingClientRect().width,
				handleSizes: [...diagram.querySelectorAll(".fam-guide-handle")].map(handle => handle.getBoundingClientRect().height),
			};
		});
		for (const height of readable.labelHeights) expect(height, "Focus labels retain readable screen size").toBeGreaterThanOrEqual(10);
		expect(readable.angleHit, "Beam edge hit area retains touch size").toBeGreaterThanOrEqual(43);
		expect(readable.focusHit, "Focus plane hit area retains touch size").toBeGreaterThanOrEqual(43);
		for (const size of readable.handleSizes) expect(size, "Visible beam handles must not shrink with the diagram").toBeGreaterThanOrEqual(16);
	}
	const faderCollisions = await dialog.locator(".fam-range-fader").evaluateAll(faders => faders.flatMap(fader => {
		const output = fader.querySelector("output")?.getBoundingClientRect();
		if (!output) return [];
		return [...fader.querySelectorAll(".fam-range-handle")].some(handle => {
			const bounds = handle.getBoundingClientRect();
			return Math.min(output.right, bounds.right) - Math.max(output.left, bounds.left) > 1 &&
				Math.min(output.bottom, bounds.bottom) - Math.max(output.top, bounds.top) > 1;
		}) ? [fader.textContent] : [];
	}));
	expect(faderCollisions, "Fader values and touch indicators must remain separate").toEqual([]);
	await expectNoExtraOperatorControls(page);
}
async function expectFlushTitle(page: Page, title = "Color") {
	const dialog = page.getByRole("dialog", { name: `${title} Special Dialog`, exact: true });
	const bounds = await dialog.boundingBox(), chrome = await dialog.locator(".ui-modal-titlebar").boundingBox();
	expect(bounds).not.toBeNull(); expect(chrome).not.toBeNull();
	for (const delta of [chrome!.x - bounds!.x, chrome!.y - bounds!.y, bounds!.x + bounds!.width - chrome!.x - chrome!.width]) {
		expect(Math.abs(delta), "The standard modal title must be flush with the modal frame").toBeLessThanOrEqual(2);
	}
	if (title !== "Color") return;
	const color = await dialog.getByTestId("full-color-editor").boundingBox();
	const body = await dialog.getByTestId("editor-page").evaluate(element => {
		const bounds = element.getBoundingClientRect(), style = getComputedStyle(element);
		return { x: bounds.x + Number.parseFloat(style.paddingLeft), width: bounds.width - Number.parseFloat(style.paddingLeft) - Number.parseFloat(style.paddingRight) };
	});
	expect(Math.abs(color!.x - body.x)).toBeLessThanOrEqual(1);
	expect(Math.abs(color!.width - body.width), "Hue and saturation span the modal content width").toBeLessThanOrEqual(1);
	const ring = await slider(page, "Hue").boundingBox();
	const white = await slider(page, "White Blend").boundingBox();
	expect(ring!.width, "Expanded hue ring must be large enough for direct control").toBeGreaterThanOrEqual(300);
	expect(Math.abs(ring!.width - ring!.height)).toBeLessThanOrEqual(1);
	expect(white!.x).toBeGreaterThan(ring!.x + ring!.width);
	expect(white!.y).toBeGreaterThanOrEqual(color!.y);
	expect(white!.y + white!.height).toBeLessThanOrEqual(color!.y + color!.height + 1);
}
async function expectReadoutsFit(page: Page, hardware: boolean) {
	const readouts = page.locator(hardware ? ".hardware-encoder-target strong" : ".touch-encoder-value");
	await expect(readouts).toHaveCount(4);
	const overflow = await readouts.evaluateAll(values => values.map(value => ({
		text: value.textContent, width: value.scrollWidth - value.clientWidth, height: value.scrollHeight - value.clientHeight,
	})).filter(value => value.width > 1 || value.height > 1));
	expect(overflow, "Encoder values must remain readable without clipping").toEqual([]);
	const overlappingText = await readouts.evaluateAll((values, isHardware) => values.flatMap(value => {
		const card = value.closest(isHardware ? ".hardware-encoder-display" : ".touch-encoder-surface");
		const label = card?.querySelector(isHardware ? ".hardware-encoder-primary-labels b" : ".touch-encoder-labels b");
		if (!label) return [];
		const textBounds = (element: Element) => {
			const range = document.createRange();
			range.selectNodeContents(element);
			return [...range.getClientRects()].filter(rect => rect.width > 0 && rect.height > 0);
		};
		const labels = textBounds(label);
		const intersects = textBounds(value).some(readout => labels.some(labelBounds =>
			Math.min(readout.right, labelBounds.right) - Math.max(readout.left, labelBounds.left) > 1 &&
			Math.min(readout.bottom, labelBounds.bottom) - Math.max(readout.top, labelBounds.top) > 1,
		));
		return intersects ? [{
			label: label.textContent, value: value.textContent,
			labelTextRects: labels.map(rect => ({ top: rect.top, bottom: rect.bottom })),
			valueTextRects: textBounds(value).map(rect => ({ top: rect.top, bottom: rect.bottom })),
			layout: [card, label.parentElement, value.parentElement, value].map(element => {
				if (!element) return null;
				const style = getComputedStyle(element);
				const rect = element.getBoundingClientRect();
				return { element: element.className, top: rect.top, bottom: rect.bottom, width: rect.width,
					display: style.display, gridTemplateRows: style.gridTemplateRows, position: style.position,
					alignItems: style.alignItems, alignSelf: style.alignSelf, placeItems: style.placeItems,
					height: style.height, padding: style.padding, gap: style.gap,
				};
			}),
		}] : [];
	}), hardware);
	expect(overlappingText, "Encoder labels and values must not overlap").toEqual([]);
}


for (const hardware of [false, true]) {
	for (const story of stories) {
		test(`${story} preserves the application shell on ${hardware ? "hardware" : "software"}`, async ({ page }) => {
			const errors: string[] = [], liveRequests: string[] = [];
			page.on("pageerror", error => errors.push(error.message));
			page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
			page.on("request", request => { if (/\/api(?:\/|\?|$)|^wss?:/.test(request.url())) liveRequests.push(request.url()); });
			await openStory(page, story, hardware);
			await expect(page.locator(".fixture-abstraction-mockup .app-shell")).toBeVisible();
			await expect(page.locator(".fixture-abstraction-mockup .control-section.programmer")).toBeVisible();
			await expect(page.locator(".fixture-abstraction-mockup .family-tabs")).toBeVisible();
			if (story === "advanced-color" || story.startsWith("mixed-selection-")) {
				await expectEditorFits(page, "Color");
				await page.screenshot({ path: `${shots}/${story}-${hardware ? "hardware" : "software"}-special-dialog.png`, fullPage: true });
				await closeSpecial(page);
			}
			if (story === "fixture-configuration") {
				await expect(page.getByRole("dialog", { name: "Fixture configuration", exact: true })).toBeVisible();
				await page.screenshot({ path: `${shots}/${story}-${hardware ? "hardware" : "software"}-modal.png`, fullPage: true });
				await page.getByRole("button", { name: "Close fixture configuration", exact: true }).click();
			}
			await expect(page.locator(".fixture-abstraction-mockup .encoder-section-items > *")).toHaveCount(4);
			await expect(page.locator(".fixture-abstraction-mockup .encoder-section")).toHaveAttribute("data-encoder-surface", hardware ? "hardware" : "touch");
			await expect(page.getByTestId("encoder-area")).toBeInViewport({ ratio: 1 });
			await expectNoExtraOperatorControls(page);
			await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
			await page.screenshot({ path: `${shots}/${story}-${hardware ? "hardware" : "software"}.png`, fullPage: true });
			expect(errors).toEqual([]); expect(liveRequests).toEqual([]);
		});
	}
}

test("dismissing Special preserves its encoder page until a deliberate family page change", async ({ page }) => {
	for (const [story, name] of [["advanced-color", "Color"], ["position-angles", "Position"], ["beam-and-shutter", "Focus"]]) {
		await openStory(page, story);
		if (story !== "advanced-color") await openSpecial(page);
		await expect(page.getByRole("dialog", { name: `${name} Special Dialog`, exact: true })).toBeVisible();
		const before = await family(page, name).getAttribute("aria-label");
		await closeSpecial(page, name);
		await expect(page.getByRole("dialog", { name: /Special Dialog$/ })).toHaveCount(0);
		await expect(page.locator(".encoder-section-items > *")).toHaveCount(4);
		await expect(family(page, name)).toHaveAttribute("aria-label", before!);
		if (name !== "Focus") {
			await family(page, name).click();
			await expect(family(page, name)).toHaveAccessibleName(`${name} 2 of 2`);
		}
	}
});

test("compact color controls fill the encoder area and preserve slider values through expansion", async ({ page }) => {
	await openStory(page, "easy-rgbw");
	await setEncoder(page, 1, "Red", 42);
	const before = await page.getByTestId("encoder-area").boundingBox();
	await openSpecial(page);
	const inline = page.locator(".fam-inline-dialog"), bounds = await inline.boundingBox();
	expect(before).not.toBeNull(); expect(bounds).not.toBeNull();
	for (const key of ["x", "y", "width", "height"] as const) expect(Math.abs(before![key] - bounds![key])).toBeLessThanOrEqual(2);
	await expect(inline.locator(".ui-modal-titlebar, footer")).toHaveCount(0);
	await expect(inline.getByTestId("color-matches")).toHaveCount(0);
	await expect(page.getByRole("application", { name: "Color picker", exact: true })).toBeVisible();
	await expect(slider(page, "Hue")).toHaveCount(0);
	await expect(slider(page, "Saturation")).toHaveCount(0);
	const pickerBounds = await page.getByTestId("color-picker").boundingBox();
	const blendBounds = await slider(page, "White Blend").locator("..").boundingBox();
	const whiteButton = await page.getByRole("button", { name: "Switch to White balance", exact: true }).boundingBox();
	const expandButton = await inline.getByRole("button", { name: "Expand", exact: true }).boundingBox();
	expect(pickerBounds!.x + pickerBounds!.width).toBeLessThan(blendBounds!.x);
	expect(whiteButton!.y - blendBounds!.y - blendBounds!.height).toBeGreaterThanOrEqual(0);
	expect(whiteButton!.y - blendBounds!.y - blendBounds!.height).toBeLessThanOrEqual(30);
	expect(Math.abs(whiteButton!.x - blendBounds!.x)).toBeLessThanOrEqual(1);
	expect(Math.abs(expandButton!.y - whiteButton!.y)).toBeLessThanOrEqual(1);
	await dragFader(page, "White Blend", "maximum");
	await slider(page, "White Blend").press("ArrowLeft");
	await expect(slider(page, "White Blend")).toHaveAttribute("aria-valuenow", "99");
	await page.getByRole("button", { name: "Switch to White balance", exact: true }).click();
	await expect(slider(page, "White Blend")).toHaveCount(0);
	await expect(slider(page, "Hue")).toHaveCount(0);
	await dragFader(page, "Temperature", "maximum");
	await slider(page, "Temperature").press("ArrowLeft");
	await dragFader(page, "Duv", "minimum");
	await slider(page, "Duv").press("ArrowRight");
	await expect(slider(page, "Temperature")).toHaveAttribute("aria-valuenow", "19900");
	await expect(slider(page, "Duv")).toHaveAttribute("aria-valuenow", "-0.0295");
	await expectEditorFits(page, "Color");
	await page.getByRole("button", { name: "Switch to Color", exact: true }).click();
	await expect(slider(page, "White Blend")).toHaveAttribute("aria-valuenow", "99");
	await expandColor(page);
	await expect(slider(page, "White Blend")).toHaveAttribute("aria-valuenow", "99");
	await expect(slider(page, "Temperature")).toHaveAttribute("aria-valuenow", "19900");
	await expect(slider(page, "Duv")).toHaveAttribute("aria-valuenow", "-0.0295");
	await expectEditorFits(page, "Color");
	await expectFlushTitle(page);
	await expect.poll(() => page.getByRole("dialog", { name: "Color Special Dialog", exact: true }).evaluate(element => element.contains(document.activeElement))).toBe(true);
	await page.keyboard.press("Escape");
	await expect(page.getByRole("button", { name: "Special Dialog", exact: true })).toBeFocused();
	await expect(touchValue(page, 1, "Red")).toHaveText("42%");
	await expect(touchValue(page, 4, "White Blend")).toHaveText("99%");
	await openSpecial(page);
	await page.getByRole("button", { name: "Switch to White balance", exact: true }).click();
	await expect(slider(page, "Temperature")).toHaveAttribute("aria-valuenow", "19900");
	await expect(slider(page, "Duv")).toHaveAttribute("aria-valuenow", "-0.0295");
});

test("the compact 2D picker edits hue and saturation through pointer and keyboard gestures", async ({ page }) => {
	await openStory(page, "advanced-color");
	const picker = page.getByRole("application", { name: "Color picker", exact: true });
	const bounds = await picker.boundingBox(); expect(bounds).not.toBeNull();
	await page.mouse.move(bounds!.x + bounds!.width * .1, bounds!.y + bounds!.height * .8);
	await page.mouse.down();
	await page.mouse.move(bounds!.x + bounds!.width * 120 / 359, bounds!.y + bounds!.height * .25, { steps: 8 });
	await page.mouse.up();
	await expandColor(page);
	await expect.poll(async () => Number(await slider(page, "Hue").getAttribute("aria-valuenow"))).toBeCloseTo(120, 0);
	await expect(slider(page, "Saturation")).toHaveAttribute("aria-valuenow", "75");
	await closeSpecial(page);
	await expect(touchValue(page, 1, "Red")).toHaveText("25%");
	await expect(touchValue(page, 2, "Green")).toHaveText("100%");
	await expect(touchValue(page, 3, "Blue")).toHaveText("25%");
	await openSpecial(page);
	await picker.press("ArrowRight");
	await picker.press("ArrowDown");
	await expandColor(page);
	await expect(slider(page, "Hue")).toHaveAttribute("aria-valuenow", "121");
	await expect(slider(page, "Saturation")).toHaveAttribute("aria-valuenow", "74");
});

test("touch relative and hardware absolute editing retain the existing encoder controls", async ({ page }) => {
	await openStory(page, "easy-rgbw");
	await touchValue(page, 1, "Red").locator("..").locator(".touch-encoder-tap-negative").click();
	await expect(touchValue(page, 1, "Red")).toHaveText("99%");
	await openStory(page, "easy-rgbw", true);
	await setEncoder(page, 1, "Red", 42, true);
	await expect(page.getByRole("button", { name: "Encoder 1: Red, 42%", exact: true })).toBeVisible();
});

test("White Blend selects zero, midpoint and full white while preserving RGB", async ({ page }) => {
	await openStory(page, "easy-rgbw");
	for (const value of [0, 50, 100]) {
		await openSpecial(page);
		if (value === 50) await clickFader(page, "White Blend", .5);
		else await slider(page, "White Blend").press(value === 100 ? "End" : "Home");
		await expect(slider(page, "White Blend")).toHaveAttribute("aria-valuenow", String(value));
		await closeSpecial(page);
		await expect(touchValue(page, 4, "White Blend")).toHaveText(`${value}%`);
		await expect(touchValue(page, 1, "Red")).toHaveText("100%");
	}
});

test("fixture configuration edits calibration inside its dedicated modal", async ({ page }) => {
	await openStory(page, "fixture-configuration");
	const configuration = page.getByRole("dialog", { name: "Fixture configuration", exact: true });
	await configuration.getByRole("button", { name: "Installed fixture", exact: true }).click();
	await configuration.getByRole("button", { name: "Invert Pan", exact: true }).click();
	await configuration.getByLabel("Pan zero offset (°)", { exact: true }).fill("15");
	await expect(configuration.getByLabel("Mounting calibration", { exact: true }).getByText("-45°", { exact: true })).toBeVisible();
	await configuration.getByRole("button", { name: "Save configuration", exact: true }).click();
	await expect(configuration.getByText("Configuration saved.", { exact: true })).toBeVisible();
});

test("zoom, iris and strobe retain physical units in their existing encoder families", async ({ page }) => {
	await openStory(page, "beam-and-shutter");
	await setEncoder(page, 1, "Zoom", 32);
	await expect(touchValue(page, 1, "Zoom")).toHaveText("32°");
	await family(page, "Beam").click();
	await setEncoder(page, 1, "Iris", 0);
	await expect(touchValue(page, 1, "Iris")).toHaveText("0%");
	await family(page, "Intensity").click();
	await touchValue(page, 2, "Shutter").click();
	await page.locator('.ui-modal-stack-layer[data-modal-top="true"]').getByRole("button", { name: "Random slow", exact: true }).click();
	await expect(touchValue(page, 2, "Shutter")).toHaveText("Random slow");
	await setEncoder(page, 3, "Strobe", 12.5);
	await expect(touchValue(page, 3, "Strobe")).toHaveText("12.5 Hz");
});

test("media grayscale, RGB tint and independent intensity preserve their meanings", async ({ page }) => {
	await openStory(page, "media-color", false, "colorMode:advanced;easyLayout:rgbwauv");
	await expect(family(page, "Color")).toHaveAccessibleName("Color");
	await setEncoder(page, 4, "White Blend", 100);
	const grayscale = await mediaTileColors(page);
	expect(grayscale.length).toBeGreaterThan(1);
	expect(grayscale.every(([red, green, blue]) => red === green && green === blue)).toBe(true);
	await setEncoder(page, 2, "Green", 0);
	await setEncoder(page, 3, "Blue", 0);
	const tinted = await mediaTileColors(page);
	expect(tinted.some(([red]) => red > 0)).toBe(true);
	expect(tinted.every(([, green, blue]) => green === 0 && blue === 0)).toBe(true);
	await expect(touchValue(page, 4, "White Blend")).toHaveText("100%");
	await family(page, "Intensity").click();
	await setEncoder(page, 1, "Intensity", 40);
	await family(page, "Color").click();
	const dimmed = await mediaTileColors(page);
	expect(Math.max(...dimmed.map(([red]) => red))).toBeLessThan(Math.max(...tinted.map(([red]) => red)));
	await expect(touchValue(page, 4, "White Blend")).toHaveText("100%");
	await expect(touchValue(page, 2, "Green")).toHaveText("0%");
});

test("Position paging preserves angles until a point or offset is edited", async ({ page }) => {
	await openStory(page, "position-angles");
	await expect(touchValue(page, 1, "Pan")).toHaveText("24°");
	await expect(touchValue(page, 2, "Tilt")).toHaveText("42°");
	await family(page, "Position").click();
	await expect(touchValue(page, 1, "Point")).toHaveText("Origin");
	await family(page, "Position").click();
	await expect(touchValue(page, 1, "Pan")).toHaveText("24°");
	await expect(touchValue(page, 2, "Tilt")).toHaveText("42°");
	await family(page, "Position").click();
	await setEncoder(page, 2, "X", 2);
	await choosePoint(page, "Stage center");
	await expect(touchValue(page, 2, "X")).toContainText("2");
	await openSpecial(page);
	await expect(page.getByRole("button", { name: "Aim reference", exact: true })).toHaveCount(0);
	await expect(page.getByRole("dialog", { name: "Position Special Dialog", exact: true }).locator("input")).toHaveCount(1);
	await closeSpecial(page, "Position");
	await expect(touchValue(page, 1, "Point")).toHaveText("Stage center");
	await expect(touchValue(page, 2, "X")).toContainText("2");
	await family(page, "Position").click();
	await expect(touchValue(page, 1, "Pan")).not.toHaveText("24°");
	await setEncoder(page, 1, "Pan", 30);
	await family(page, "Position").click();
	await family(page, "Position").click();
	await expect(touchValue(page, 1, "Pan")).toHaveText("30°");
});

test("the modal Position circle preserves multiple turns and its tilt fader edits angles", async ({ page }) => {
	await openStory(page, "position-angles");
	await setEncoder(page, 1, "Pan", 0);
	await openSpecial(page);
	const pan = slider(page, "Pan circle");
	await expect(pan).toHaveAttribute("aria-valuemin", "-720");
	await expect(pan).toHaveAttribute("aria-valuemax", "720");
	const ring = await pan.boundingBox(); expect(ring).not.toBeNull();
	const pointAt = (degrees: number) => ({ x: ring!.x + ring!.width * (.5 + .35 * Math.sin(degrees * Math.PI / 180)), y: ring!.y + ring!.height * (.5 - .35 * Math.cos(degrees * Math.PI / 180)) });
	await page.mouse.move(pointAt(0).x, pointAt(0).y);
	await page.mouse.down();
	for (let angle = 15; angle <= 450; angle += 15) await page.mouse.move(pointAt(angle).x, pointAt(angle).y);
	await page.mouse.up();
	await expect.poll(async () => Number(await pan.getAttribute("aria-valuenow"))).toBeCloseTo(450, 0);
	await page.mouse.down();
	for (let angle = 75; angle >= -810; angle -= 15) await page.mouse.move(pointAt(angle).x, pointAt(angle).y);
	await page.mouse.up();
	await expect.poll(async () => Number(await pan.getAttribute("aria-valuenow"))).toBeCloseTo(-450, 0);
	await page.getByRole("button", { name: "Increase pan by 90 degrees", exact: true }).click();
	await expect(pan).toHaveAttribute("aria-valuenow", "-360");
	await page.getByRole("button", { name: "Reset pan to zero", exact: true }).click();
	await expect(pan).toHaveAttribute("aria-valuenow", "0");
	await page.getByRole("button", { name: "Decrease pan by 90 degrees", exact: true }).click();
	await expect(pan).toHaveAttribute("aria-valuenow", "-90");
	await slider(page, "Tilt angle").press("Home");
	await expect(slider(page, "Tilt angle")).toHaveAttribute("aria-valuenow", "-135");
	await slider(page, "Tilt angle").press("End");
	await expect(slider(page, "Tilt angle")).toHaveAttribute("aria-valuenow", "135");
	await expectEditorFits(page, "Position");
	await page.screenshot({ path: `${shots}/position-multiple-turns.png`, fullPage: true });
	await closeSpecial(page, "Position");
	await expect(touchValue(page, 1, "Pan")).toContainText("-90");
	await expect(touchValue(page, 2, "Tilt")).toContainText("135");
});

test("mount motion supplied by Storybook changes resolved angles while retaining the target", async ({ page }) => {
	await openStory(page, "position-tracked-target");
	await expect(touchValue(page, 1, "Point")).toHaveText("Stage center");
	await openSpecial(page);
	const originalPan = Number(await slider(page, "Pan circle").getAttribute("aria-valuenow"));
	const originalTilt = Number(await slider(page, "Tilt angle").getAttribute("aria-valuenow"));
	await openStory(page, "position-tracked-target", false, "mountLift:1;mountYaw:15");
	await expect(touchValue(page, 1, "Point")).toHaveText("Stage center");
	for (const [index, axis] of ["X", "Y", "Z"].entries()) await expect(touchValue(page, index + 2, axis)).toContainText("0");
	await openSpecial(page);
	expect(Number(await slider(page, "Pan circle").getAttribute("aria-valuenow"))).toBeCloseTo(originalPan - 15, 2);
	expect(Number(await slider(page, "Tilt angle").getAttribute("aria-valuenow"))).toBeLessThan(originalTilt);
});

test("Focus beam edges and focus plane respond to direct dragging", async ({ page }) => {
	await openStory(page, "beam-and-shutter");
	await openSpecial(page);
	const dialog = page.getByRole("dialog", { name: "Focus Special Dialog", exact: true });
	await expect(dialog).toHaveAttribute("aria-modal", "true");
	const diagram = page.getByRole("group", { name: "Beam angle and focus diagram", exact: true });
	const rect = await diagram.boundingBox(); expect(rect).not.toBeNull();
	const angle = slider(page, "Beam opening angle"), focus = slider(page, "Focus position");
	const firstHandle = angle.locator(".fam-guide-handle").first(), handleBounds = await firstHandle.boundingBox(); expect(handleBounds).not.toBeNull();
	await page.mouse.move(handleBounds!.x + handleBounds!.width / 2, handleBounds!.y + handleBounds!.height / 2);
	await page.mouse.down();
	await page.mouse.move(handleBounds!.x + handleBounds!.width / 2, handleBounds!.y - rect!.height * .1, { steps: 8 });
	await page.mouse.up();
	expect(Number(await angle.getAttribute("aria-valuenow"))).toBeGreaterThan(24);
	const focusHandle = focus.locator(".fam-focus-handle"), focusBounds = await focusHandle.boundingBox(); expect(focusBounds).not.toBeNull();
	await page.mouse.move(focusBounds!.x + focusBounds!.width / 2, focusBounds!.y + focusBounds!.height / 2);
	await page.mouse.down();
	await page.mouse.move(focusBounds!.x + focusBounds!.width / 2 + rect!.width * .2, focusBounds!.y + focusBounds!.height / 2, { steps: 8 });
	await page.mouse.up();
	expect(Number(await focus.getAttribute("aria-valuenow"))).toBeGreaterThan(50);
	await angle.press("Home"); await focus.press("End");
	await expect(angle).toHaveAttribute("aria-valuenow", "8");
	await expect(focus).toHaveAttribute("aria-valuenow", "100");
	await expectEditorFits(page, "Focus");
	await expect(dialog.getByRole("button", { name: "Expand", exact: true })).toHaveCount(0);
	await page.screenshot({ path: `${shots}/focus-beam-drag.png`, fullPage: true });
	await closeSpecial(page, "Focus");
	await expect(touchValue(page, 1, "Zoom")).toHaveText("8°");
	await expect(touchValue(page, 2, "Focus")).toHaveText("100%");
});

function matchRow(page: Page, index: number) { return page.getByTestId(`color-match-${["a7", "root", "auro"][index]}`); }
async function expectRangeValues(page: Page, attribute: string, expected: number[]) {
	await showColorTab(page, "Details");
	for (const [index, value] of expected.entries()) {
		await expect.poll(async () => Number(await matchRow(page, index).getAttribute(`data-${attribute}`)), `Fixture ${index + 1} receives its ordered ${attribute} value`).toBeCloseTo(value, attribute === "duv" ? 5 : 2);
	}
}

test("Shift first and last hues spread over the shortest arc with clockwise half-turn ties", async ({ page }) => {
	await openStory(page, "mixed-selection-magenta");
	await expandColor(page);
	await clickHue(page, 300, true);
	await clickHue(page, 60, true);
	await expect(slider(page, "Hue")).toHaveAttribute("aria-valuetext", "300 through 60 degrees");
	await expandColor(page);
	await expectRangeValues(page, "hue", [300, 0, 60]);
	await expect(page.getByLabel("JBLED A7 estimated output", { exact: true })).toHaveCSS("background-color", "rgb(255, 0, 255)");
	await expect(page.getByLabel("Cameo ROOT PAR 6 estimated output", { exact: true })).toHaveCSS("background-color", "rgb(255, 0, 0)");
	await expect(matchRow(page, 2)).toContainText("Yellow");
	await clickHue(page, 0);
	await clickHue(page, 180, true);
	await expectRangeValues(page, "hue", [0, 90, 180]);
	await expect(page.getByLabel("Cameo ROOT PAR 6 estimated output", { exact: true })).toHaveCSS("background-color", "rgb(128, 255, 0)");
	await showColorTab(page, "Color");
	await expect(slider(page, "Hue")).toHaveAttribute("aria-valuetext", "0 through 180 degrees");
	await clickHue(page, 240, true);
	await expectRangeValues(page, "hue", [0, 300, 240]);
	await page.screenshot({ path: `${shots}/color-range-shortest-hue-arc.png`, fullPage: true });
});

test("descending slider ranges interpolate each selected fixture and clear independently", async ({ page }) => {
	await openStory(page, "mixed-selection-magenta");
	await expandColor(page);
	await clickFader(page, "White Blend", .8);
	await clickFader(page, "White Blend", .2, true);
	await expect(slider(page, "White Blend")).toHaveAttribute("aria-valuetext", "80% through 20%");
	await expectRangeValues(page, "white", [80, 50, 20]);
	await clickFader(page, "Temperature", 18000 / 19000);
	await clickFader(page, "Temperature", 1000 / 19000, true);
	await expectRangeValues(page, "temperature", [19000, 10500, 2000]);
	await clickFader(page, "Duv", .9);
	await clickFader(page, "Duv", .1, true);
	await expectRangeValues(page, "duv", [.024, 0, -.024]);
	await dragFader(page, "Saturation", "maximum");
	await clickFader(page, "Saturation", .2, true);
	await expectRangeValues(page, "saturation", [100, 60, 20]);
	const colors = await page.getByTestId("color-matches").getByLabel(/estimated output$/).evaluateAll(elements => elements.map(element => getComputedStyle(element).backgroundColor));
	expect(new Set(colors).size).toBeGreaterThan(1);
	await clickFader(page, "White Blend", .5);
	await expectRangeValues(page, "white", [50, 50, 50]);
	await expectRangeValues(page, "temperature", [19000, 10500, 2000]);
	await expectRangeValues(page, "duv", [.024, 0, -.024]);
	await expectRangeValues(page, "saturation", [100, 60, 20]);
	await closeSpecial(page);
	await openSpecial(page);
	await expandColor(page);
	await expectRangeValues(page, "temperature", [19000, 10500, 2000]);
	await expectRangeValues(page, "duv", [.024, 0, -.024]);
	await page.screenshot({ path: `${shots}/color-descending-ranges.png`, fullPage: true });
});

test("a Shift drag sets range endpoints and normal keyboard editing collapses only that control", async ({ page }) => {
	await openStory(page, "mixed-selection-magenta");
	await expandColor(page);
	const control = slider(page, "White Blend"), bounds = await control.boundingBox(); expect(bounds).not.toBeNull();
	await page.keyboard.down("Shift");
	await page.mouse.move(bounds!.x + bounds!.width * .2, bounds!.y + bounds!.height / 2);
	await page.mouse.down();
	await page.mouse.move(bounds!.x + bounds!.width * .8, bounds!.y + bounds!.height / 2, { steps: 8 });
	await page.mouse.up();
	await page.keyboard.up("Shift");
	await expectRangeValues(page, "white", [20, 50, 80]);
	await showColorTab(page, "Color");
	await control.press("Shift+ArrowLeft");
	await expectRangeValues(page, "white", [20, 49.5, 79]);
	await showColorTab(page, "Color");
	await control.press("End");
	await expectRangeValues(page, "white", [100, 100, 100]);
});

test("RGBWAUV paging retains unsupported UV in the per-fixture comparison", async ({ page }) => {
	await openStory(page, "mixed-selection-magenta", false, "colorMode:easy;easyLayout:rgbwauv");
	await closeSpecial(page);
	await family(page, "Color").click();
	await setEncoder(page, 2, "UV", 25);
	await openSpecial(page);
	await expandColor(page);
	await showColorTab(page, "Details");
	await expect(matchRow(page, 0)).toContainText(/UV.*unavailable|UV.*unsupported/);
	await expect(matchRow(page, 2)).toContainText(/UV.*unavailable|UV.*unsupported/);
	await expect(page.getByLabel("JBLED A7 estimated output", { exact: true })).toHaveCSS("background-color", "rgb(255, 0, 255)");
	await closeSpecial(page);
	await expect(touchValue(page, 2, "UV")).toHaveText("25%");
	await family(page, "Color").click();
	await expect(touchValue(page, 1, "Red")).toHaveText("100%");
	await expect(touchValue(page, 2, "Green")).toHaveText("0%");
	await expect(touchValue(page, 3, "Blue")).toHaveText("100%");
});

for (const hardware of [false, true]) {
	for (const viewport of viewports) {
		test(`color, position, focus and media fit ${viewport.width}px ${hardware ? "hardware" : "software"}`, async ({ page }) => {
			await page.setViewportSize(viewport);
			await openStory(page, "advanced-color", hardware);
			const inline = page.locator(".fam-inline-dialog");
			if (await inline.count()) {
				await expect(inline.getByTestId("color-matches")).toHaveCount(0);
				await expectEditorFits(page, "Color");
				await page.screenshot({ path: `${shots}/color-plane-compact-${hardware ? "hardware" : "software"}-${viewport.width}.png`, fullPage: true });
				await page.getByRole("button", { name: "Switch to White balance", exact: true }).click();
				await expect(slider(page, "Hue")).toHaveCount(0);
				await expect(slider(page, "White Blend")).toHaveCount(0);
				await expect(slider(page, "Temperature")).toBeVisible();
				await expect(slider(page, "Duv")).toBeVisible();
				await expectEditorFits(page, "Color");
				await page.screenshot({ path: `${shots}/white-gradient-compact-${hardware ? "hardware" : "software"}-${viewport.width}.png`, fullPage: true });
			}
			await expandColor(page);
			for (const label of ["Hue", "Saturation", "White Blend", "Temperature", "Duv"]) await expect(slider(page, label)).toBeVisible();
			await expect(page.getByRole("button", { name: /^Switch to / })).toHaveCount(0);
			await expectEditorFits(page, "Color");
			await expectFlushTitle(page);
			await page.screenshot({ path: `${shots}/color-ring-full-modal-${hardware ? "hardware" : "software"}-${viewport.width}.png`, fullPage: true });
			await closeSpecial(page);
			await expectReadoutsFit(page, hardware);
			await family(page, "Color").click();
			await setEncoder(page, 1, "Temperature", 20000, hardware);
			await setEncoder(page, 2, "Green / Magenta", -.03, hardware);
			await expectReadoutsFit(page, hardware);
			await openStory(page, "position-tracked-target", hardware);
			await expectReadoutsFit(page, hardware);
			await setEncoder(page, 2, "X", -100, hardware);
			await expectReadoutsFit(page, hardware);
			await openSpecial(page);
			await expectEditorFits(page, "Position");
			await page.screenshot({ path: `${shots}/position-target-${hardware ? "hardware" : "software"}-${viewport.width}.png`, fullPage: true });
			await closeSpecial(page, "Position");
			await family(page, "Position").click();
			await openSpecial(page);
			await expectEditorFits(page, "Position");
			await page.screenshot({ path: `${shots}/position-angle-${hardware ? "hardware" : "software"}-${viewport.width}.png`, fullPage: true });
			await openStory(page, "beam-and-shutter", hardware);
			await openSpecial(page);
			const angle = slider(page, "Beam opening angle"), focus = slider(page, "Focus position");
			await angle.focus();
			await expect(angle).toBeFocused();
			await page.keyboard.press("End");
			await expect(angle).toHaveAttribute("aria-valuenow", "48");
			await focus.focus();
			await expect(focus).toBeFocused();
			await page.keyboard.press("End");
			await expect(focus).toHaveAttribute("aria-valuenow", "100");
			await expectEditorFits(page, "Focus");
			await page.screenshot({ path: `${shots}/focus-diagram-${hardware ? "hardware" : "software"}-${viewport.width}.png`, fullPage: true });
			await openStory(page, "media-color", hardware);
			await openSpecial(page);
			const preview = page.getByRole("button", { name: "Switch to Preview", exact: true });
			if (await preview.count()) {
				await preview.click();
				await expect(processedMedia(page)).toBeVisible();
				await expectEditorFits(page, "Color");
			}
			await expandColor(page);
			await expect(slider(page, "Hue")).toBeVisible();
			await expect(slider(page, "White Blend")).toBeVisible();
			await expect(slider(page, "Temperature")).toHaveCount(0);
			await expect(slider(page, "Duv")).toHaveCount(0);
			await expect(processedMedia(page)).toHaveCount(0);
			await showColorTab(page, "Preview");
			await expect(processedMedia(page)).toBeVisible();
			await expectEditorFits(page, "Color");
			await page.screenshot({ path: `${shots}/media-ring-modal-${hardware ? "hardware" : "software"}-${viewport.width}.png`, fullPage: true });
		});
	}
}

for (const hardware of [false, true]) {
	for (const viewport of viewports) {
		test(`mixed color comparisons fit ${viewport.width}px ${hardware ? "hardware" : "software"}`, async ({ page }) => {
			await page.setViewportSize(viewport);
			for (const white of [false, true]) {
				const example = white ? "warm-white" : "magenta";
				await openStory(page, `mixed-selection-${example}`, hardware);
				await expectEditorFits(page, "Color");
				await expandColor(page);
				await expectEditorFits(page, "Color");
				await expectFlushTitle(page);
				await showColorTab(page, "Details");
				await expectEditorFits(page, "Color");
				const matches = page.getByTestId("color-matches");
				for (let index = 0; index < 3; index++) await expect(matchRow(page, index)).toBeVisible();
				if (white) {
					await expect(matchRow(page, 0)).toContainText("RGB mixed white");
					await expect(matchRow(page, 1)).toContainText("RGB + white + amber");
					await expect(matchRow(page, 2)).toContainText("Warm White");
					await expectRangeValues(page, "temperature", [3200, 3200, 3200]);
				} else {
					await expect(page.getByLabel("JBLED A7 estimated output", { exact: true })).toHaveCSS("background-color", "rgb(255, 0, 255)");
					await expect(page.getByLabel("Cameo ROOT PAR 6 estimated output", { exact: true })).toHaveCSS("background-color", "rgb(255, 0, 255)");
					await expect(page.getByLabel("Cameo AURO SPOT estimated output", { exact: true })).toHaveCSS("background-color", "rgb(52, 43, 199)");
					await expect(matchRow(page, 2)).toContainText("≈ Approx.");
				}
				const overflow = await matches.locator("b, small, .fam-match-status").evaluateAll(elements => elements.map(element => ({
					text: element.textContent, width: element.scrollWidth - element.clientWidth, height: element.scrollHeight - element.clientHeight,
				})).filter(element => element.width > 1 || element.height > 1));
				expect(overflow, "Every fixture comparison label must be readable").toEqual([]);
				await page.screenshot({ path: `${shots}/mixed-${example}-ring-full-${hardware ? "hardware" : "software"}-${viewport.width}.png`, fullPage: true });
			}
		});
	}
}

test("Target aim preserves its encoder page and modal geometry until joystick release", async ({ page }) => {
	await openStory(page, "position-tracked-target");
	await openSpecial(page);
	const joystick = page.getByRole("application", { name: "Position aim joystick", exact: true });
	const pan = slider(page, "Pan circle"), initial = Number(await pan.getAttribute("aria-valuenow"));
	const bounds = await joystick.boundingBox(); expect(bounds).not.toBeNull();
	await page.mouse.move(bounds!.x + bounds!.width / 2, bounds!.y + bounds!.height / 2);
	await page.mouse.down();
	await page.waitForTimeout(200);
	await page.mouse.up();
	await expect(page.locator('.family-tabs button[aria-label="Position 2 of 2"]')).toHaveCount(1);
	expect(await joystick.boundingBox()).toEqual(bounds);
	expect(Number(await pan.getAttribute("aria-valuenow"))).toBe(initial);
	await expect(joystick).toHaveAttribute("data-active", "false");
	await page.mouse.move(bounds!.x + bounds!.width * .75, bounds!.y + bounds!.height * .5);
	await page.mouse.down();
	await page.waitForTimeout(350);
	const first = Number(await pan.getAttribute("aria-valuenow"));
	await expect(page.locator('.family-tabs button[aria-label="Position 2 of 2"]')).toHaveCount(1);
	expect(await joystick.boundingBox()).toEqual(bounds);
	await page.waitForTimeout(350);
	const last = Number(await pan.getAttribute("aria-valuenow"));
	expect(first).toBeGreaterThan(initial); expect(last).toBeGreaterThan(first);
	await page.mouse.up();
	await expect(page.locator('.family-tabs button[aria-label="Position 1 of 2"]')).toHaveCount(1);
	expect(await joystick.boundingBox()).toEqual(bounds);
	await expect(page.getByRole("dialog", { name: "Position Special Dialog", exact: true })).toBeVisible();
	await closeSpecial(page, "Position");
	await expect(family(page, "Position")).toHaveAccessibleName("Position 1 of 2");
});

test("beam angle handles remain draggable when the focus plane is at its far end", async ({ page }) => {
	await openStory(page, "beam-and-shutter");
	await openSpecial(page);
	const angle = slider(page, "Beam opening angle"), focus = slider(page, "Focus position");
	await focus.press("End");
	const handle = angle.locator(".fam-guide-handle").first(), bounds = await handle.boundingBox(); expect(bounds).not.toBeNull();
	await page.mouse.move(bounds!.x + bounds!.width / 2, bounds!.y + bounds!.height / 2);
	await page.mouse.down();
	await page.mouse.move(bounds!.x + bounds!.width / 2, bounds!.y - 12, { steps: 8 });
	await page.mouse.up();
	await expect(focus).toHaveAttribute("aria-valuenow", "100");
	expect(Number(await angle.getAttribute("aria-valuenow"))).toBeGreaterThan(24);
});

test("Storybook Easy and Advanced settings preserve the live recipe without reloading", async ({ page }) => {
	await page.setViewportSize({ width: 1496, height: 1000 });
	await page.goto(`/?path=/story/${prefix}easy-rgbw&addonPanel=storybook/controls/panel`);
	await page.getByRole("tab", { name: /^Controls/ }).click();
	const settings = page.getByRole("group", { name: "colorMode", exact: true });
	await expect(settings).toBeVisible();
	const preview = page.frameLocator("#storybook-preview-iframe");
	const workspace = preview.getByTestId("existing-workspace");
	await expect(workspace).toBeVisible();
	const original = await workspace.innerHTML();
	const value = (slot: number, name: string) => preview.getByRole("button", { name: `Set Enc ${slot} · ${name} value`, exact: true });
	const edit = async (slot: number, name: string, next: number) => {
		await value(slot, name).click();
		await expect(preview.locator('.ui-modal-stack-layer[data-modal-top="true"]')).toBeVisible();
		await page.keyboard.type(String(next)); await page.keyboard.press("Enter");
		await expect(preview.locator('.ui-modal-stack-layer[data-modal-top="true"]')).toHaveCount(0);
	};
	await edit(1, "Red", 42); await edit(4, "White Blend", 50);
	await settings.getByRole("radio", { name: "advanced", exact: true }).check();
	await expect(value(1, "Red")).toHaveText("42%");
	await expect(value(4, "White Blend")).toHaveText("50%");
	await preview.getByRole("button", { name: "Color 1 of 2", exact: true }).click();
	await edit(1, "Temperature", 4200); await edit(2, "Green / Magenta", -.01);
	await settings.getByRole("radio", { name: "easy", exact: true }).check();
	await expect(value(1, "Red")).toHaveText("42%");
	await expect(value(4, "White Blend")).toHaveText("50%");
	await expect(workspace).toHaveJSProperty("innerHTML", original);
	await settings.getByRole("radio", { name: "advanced", exact: true }).check();
	await preview.getByRole("button", { name: "Color 1 of 2", exact: true }).click();
	await expect(value(1, "Temperature")).toContainText("4200");
	await expect(value(2, "Green / Magenta")).toContainText("-0.010");
	await expect(workspace).toHaveJSProperty("innerHTML", original);
	await page.screenshot({ path: `${shots}/storybook-settings-recipe-preserved.png`, fullPage: true });
});

test("compact 2D Shift endpoints spread both hue and saturation in fixture order", async ({ page }) => {
	await openStory(page, "mixed-selection-magenta");
	const picker = page.getByRole("application", { name: "Color picker", exact: true });
	const bounds = await picker.boundingBox(); expect(bounds).not.toBeNull();
	await page.keyboard.down("Shift");
	await page.mouse.click(bounds!.x + bounds!.width * 300 / 359, bounds!.y + bounds!.height * .2);
	await page.mouse.click(bounds!.x + bounds!.width * 60 / 359, bounds!.y + bounds!.height * .6);
	await page.keyboard.up("Shift");
	await expandColor(page);
	await expectRangeValues(page, "hue", [300, 0, 60]);
	await expectRangeValues(page, "saturation", [80, 60, 40]);
	await showColorTab(page, "Color");
	await expect(slider(page, "Hue")).toHaveAttribute("aria-valuetext", "300 through 60 degrees");
	await expect(slider(page, "Saturation")).toHaveAttribute("aria-valuetext", "80% through 40%");
});

test("held aim joystick has gentle center control and stops on center, release, blur, cancel and close", async ({ page }) => {
	await openStory(page, "position-angles");
	await setEncoder(page, 1, "Pan", 0); await setEncoder(page, 2, "Tilt", 0);
	await openSpecial(page);
	const joystick = page.getByRole("application", { name: "Position aim joystick", exact: true });
	const pan = slider(page, "Pan circle");
	const bounds = await joystick.boundingBox(); expect(bounds).not.toBeNull();
	const sample = () => pan.evaluate(element => ({ value: Number(element.getAttribute("aria-valuenow")), time: performance.now() }));
	const hold = async (fraction: number) => {
		await page.mouse.move(bounds!.x + bounds!.width * fraction, bounds!.y + bounds!.height / 2);
		await page.mouse.down();
	};
	// The timed stationary hold is the interaction being verified; no pointer moves occur during these waits.
	await hold(.625);
	await page.waitForTimeout(150);
	const slowStart = await sample();
	await page.waitForTimeout(400);
	const slowMiddle = await sample();
	await page.waitForTimeout(400);
	const slowEnd = await sample();
	expect(slowMiddle.value - slowStart.value, "A stationary held pointer must keep moving pan").toBeGreaterThan(1);
	expect(slowEnd.value - slowMiddle.value, "Pan must continue in the next interval without another pointer move").toBeGreaterThan(1);
	await page.mouse.up();
	const released = await sample();
	await expect(joystick).toHaveAttribute("data-active", "false");
	expect(await page.getByTestId("joystick-marker").evaluate(element => ({ left: (element as HTMLElement).style.left, top: (element as HTMLElement).style.top }))).toEqual({ left: "50%", top: "50%" });
	await page.waitForTimeout(300);
	expect((await sample()).value).toBeCloseTo(released.value, 1);
	await hold(.75);
	await page.waitForTimeout(150);
	const fastStart = await sample();
	await page.waitForTimeout(800);
	const fastEnd = await sample();
	await page.mouse.up();
	const slowRate = (slowEnd.value - slowStart.value) / (slowEnd.time - slowStart.time);
	const fastRate = (fastEnd.value - fastStart.value) / (fastEnd.time - fastStart.time);
	expect(fastRate / slowRate, "The central region must be gentler than a linear response while outer deflection moves faster").toBeGreaterThan(3.5);
	expect(fastRate / slowRate).toBeLessThan(7);
	const tilt = slider(page, "Tilt angle");
	await expect(tilt).toHaveAttribute("aria-valuenow", "0");
	await page.mouse.move(bounds!.x + bounds!.width / 2, bounds!.y + bounds!.height * .25);
	await page.mouse.down();
	await page.waitForTimeout(350);
	expect(Number(await tilt.getAttribute("aria-valuenow")), "Vertical deflection must move Tilt").toBeGreaterThan(3);
	await page.mouse.move(bounds!.x + bounds!.width / 2, bounds!.y + bounds!.height / 2);
	const centered = { pan: (await sample()).value, tilt: Number(await tilt.getAttribute("aria-valuenow")) };
	await page.waitForTimeout(300);
	expect((await sample()).value).toBeCloseTo(centered.pan, 1);
	expect(Number(await tilt.getAttribute("aria-valuenow")), "Returning to center must stop while the pointer is still held").toBeCloseTo(centered.tilt, 1);
	await page.mouse.up();
	await hold(.75);
	await page.waitForTimeout(150);
	await page.evaluate(() => window.dispatchEvent(new Event("blur")));
	const blurred = await sample();
	await expect(joystick).toHaveAttribute("data-active", "false");
	await page.waitForTimeout(300);
	expect((await sample()).value).toBeCloseTo(blurred.value, 1);
	await page.mouse.up();
	await hold(.75);
	await page.waitForTimeout(150);
	await joystick.dispatchEvent("pointercancel", { pointerId: 1, pointerType: "mouse" });
	const cancelled = await sample();
	await expect(joystick).toHaveAttribute("data-active", "false");
	await page.waitForTimeout(300);
	expect((await sample()).value).toBeCloseTo(cancelled.value, 1);
	await page.mouse.up();
	await hold(.75);
	await page.waitForTimeout(150);
	await page.keyboard.press("Escape");
	await expect(joystick).toHaveCount(0);
	const closed = await touchValue(page, 1, "Pan").textContent();
	await page.waitForTimeout(300);
	await expect(touchValue(page, 1, "Pan")).toHaveText(closed!);
	await page.mouse.up();
	await page.screenshot({ path: `${shots}/joystick-stopped-after-close.png`, fullPage: true });
});
