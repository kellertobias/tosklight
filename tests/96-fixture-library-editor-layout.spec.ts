import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test as base, expect, type Locator, type Page } from "@playwright/test";
import { unzipSync, strFromU8 } from "fflate";
import { createServer, type ViteDevServer } from "vite";
import artifactResolver from "../tools/artifact-paths.cjs";
import { mockTauriIpc } from "./bench/window-system/tauriIpc";

/**
 * The fixture-library editor as the PreViz Editor hosts it, with the Tauri bridge answered in the
 * page: the library is one shipped package, and every other request waits.
 */
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const { artifactPaths } = artifactResolver;

const test = base.extend<object, { editorUrl: string }>({
	editorUrl: [
		async ({}, use) => {
			const server: ViteDevServer = await createServer({
				configFile: path.join(root, "apps/viz-editor/vite.config.ts"),
				root: path.join(root, "apps/viz-editor"),
				cacheDir: `${artifactPaths.viteCache}/viz-editor-playwright`,
				server: { port: 0, strictPort: false, host: "127.0.0.1" },
				logLevel: "error",
			});
			await server.listen();
			const address = server.httpServer?.address();
			if (!address || typeof address === "string")
				throw new Error("The PreViz Editor dev server has no port");
			try {
				await use(`http://127.0.0.1:${address.port}/`);
			} finally {
				await server.close();
			}
		},
		{ scope: "worker", timeout: 120_000 },
	],
});

/** A shipped spot with a gobo/color wheel, a Stage geometry, and a beam. */
function shippedProfile(file = "cameo--auro-spot-z300.toskfixture") {
	const archive = unzipSync(
		readFileSync(path.join(root, "assets/fixture-library", file)),
	);
	return JSON.parse(strFromU8(archive["fixture.json"])).profile;
}

const REGISTRY = [
	["intensity", "Intensity", "intensity", "continuous"],
	["color.wheel.1", "Color wheel", "color", "indexed"],
	["shutter", "Shutter / Strobe", "intensity", "indexed"],
	["pan", "Pan", "position", "continuous"],
	["tilt", "Tilt", "position", "continuous"],
	["control", "Control", "control", "indexed"],
].map(([id, label, family, value_type]) => ({
	id,
	label,
	family,
	value_type,
	default_unit: null,
}));

async function openEditor(page: Page, editorUrl: string, file?: string) {
	const profile = shippedProfile(file);
	await mockTauriIpc(
		page,
		(command, _args, answers) =>
			command in answers
				? answers[command]
				: new Promise(() => undefined),
		{
			library_profiles: [
				{
					id: profile.id,
					revision: profile.revision,
					manufacturer: profile.manufacturer,
					name: profile.name,
					profile,
				},
			],
			attribute_registry: REGISTRY,
			fixture_body_catalogue: [],
		} as Record<string, unknown>,
	);
	await page.goto(editorUrl);
	await page.getByRole("button", { name: "Settings", exact: true }).click();
	await page.getByRole("tab", { name: "Library", exact: true }).click();
	await page.getByRole("button", { name: new RegExp(`^${profile.manufacturer}`) }).click();
	await page.getByRole("button", { name: new RegExp(`^${profile.name}`) }).click();
	await page.getByRole("button", { name: "Edit as new revision", exact: true }).click();
	const editor = page.getByRole("dialog", { name: "Edit fixture profile" });
	await expect(editor).toBeVisible();
	return { editor, profile };
}

/** Every element inside `scope` (itself included) that currently shows a scrollbar, by class. */
async function scrollingRegions(scope: Locator) {
	return scope.evaluate((element) => {
		const found: string[] = [];
		for (const node of [element, ...element.querySelectorAll("*")]) {
			const style = getComputedStyle(node);
			const scrollsY =
				/(auto|scroll)/.test(style.overflowY) &&
				node.scrollHeight > node.clientHeight + 1;
			const scrollsX =
				/(auto|scroll)/.test(style.overflowX) &&
				node.scrollWidth > node.clientWidth + 1;
			if (scrollsY || scrollsX)
				found.push(
					`${String(node.className).split(" ")[0] || node.tagName}${scrollsY ? " y" : ""}${scrollsX ? " x" : ""}`,
				);
		}
		return found;
	});
}

/**
 * A tab button, pressed without the pointer: in a narrow window the title bar's tabs are crowded
 * together, which is not what these tests are about.
 */
async function openTab(dialog: Locator, name: string) {
	const tab = dialog.getByRole("tab", { name, exact: true });
	await tab.dispatchEvent("click");
	await expect(tab).toHaveAttribute("aria-selected", "true");
}

async function openModeEditor(page: Page, editor: Locator, name = "17-Channel") {
	await openTab(editor, "Modes");
	await page
		.getByRole("button", { name: `Edit channels for ${name}`, exact: true })
		.click();
	const mode = page.getByRole("dialog", { name: `Edit ${name} mode` });
	await expect(mode).toBeVisible();
	return mode;
}

/** The colours of a rendered element, read back from a screenshot of it. */
async function screenshotPixels(page: Page, target: Locator) {
	const png = (await target.screenshot()).toString("base64");
	return page.evaluate(async (data) => {
		const image = new Image();
		image.src = `data:image/png;base64,${data}`;
		await image.decode();
		const canvas = document.createElement("canvas");
		canvas.width = image.width;
		canvas.height = image.height;
		const context = canvas.getContext("2d");
		if (!context) throw new Error("no 2D context");
		context.drawImage(image, 0, 0);
		const { data: rgba } = context.getImageData(0, 0, image.width, image.height);
		const at = (x: number, y: number) => {
			const index = (Math.floor(y) * image.width + Math.floor(x)) * 4;
			return [rgba[index], rgba[index + 1], rgba[index + 2]];
		};
		const background = at(4, 4);
		let differing = 0;
		for (let offset = 0; offset < rgba.length; offset += 4) {
			const distance =
				Math.abs(rgba[offset] - background[0]) +
				Math.abs(rgba[offset + 1] - background[1]) +
				Math.abs(rgba[offset + 2] - background[2]);
			if (distance > 24) differing += 1;
		}
		return {
			background,
			lampShare: differing / (image.width * image.height),
		};
	}, png);
}

const VIEWPORTS = [
	{ name: "wide", width: 1280, height: 720 },
	{ name: "narrow", width: 760, height: 560 },
];

test("TL-446 @ui › the geometry preview shows the lamp on dark blue, without a beam", async ({
	page,
	editorUrl,
}) => {
	await page.setViewportSize({ width: 1280, height: 720 });
	const { editor } = await openEditor(page, editorUrl);
	await openTab(editor, "Geometry");
	await editor.getByRole("button", { name: "Moving head", exact: true }).click();
	const preview = editor.getByRole("region", { name: "Live geometry preview" });
	await expect(preview).toContainText("3 parts · 1 emitters");
	const stage = preview.getByRole("img", {
		name: "Fixture geometry hierarchy in three dimensions",
	});
	await expect(stage.locator("canvas")).toBeVisible();
	await expect(stage).toHaveCSS("background-color", "rgb(20, 35, 58)");

	const { background, lampShare } = await screenshotPixels(page, stage);
	const [red, green, blue] = background;
	// Dark blue, not the near-black it was: blue leads, and it is clearly lifted off black.
	expect(blue).toBeGreaterThan(red + 20);
	expect(blue).toBeGreaterThan(green + 10);
	expect(blue).toBeGreaterThan(40);
	expect(blue).toBeLessThan(90);
	// The lamp is framed, so it fills a real part of the view rather than a speck.
	expect(lampShare).toBeGreaterThan(0.04);
	await stage.screenshot({
		path: `${artifactPaths.results}/tl-446-geometry-preview.png`,
	});
});

for (const viewport of VIEWPORTS) {
	test(`TL-446 @ui › the ${viewport.name} mode editor scrolls in one place per tab`, async ({
		page,
		editorUrl,
	}) => {
		await page.setViewportSize(viewport);
		const { editor } = await openEditor(page, editorUrl);
		const mode = await openModeEditor(page, editor);

		await openTab(mode, "Heads");
		expect(await scrollingRegions(mode)).toEqual([]);

		// The channel table scrolls itself, both ways; the body around it does not scroll as well.
		await openTab(mode, "Channels");
		const table = mode.locator(".fixture-channel-table-wrap");
		await expect(table).toBeVisible();
		expect(await scrollingRegions(mode)).toEqual(["fixture-channel-table-wrap y x"]);
		const tableBottom = await table.evaluate(
			(element) => element.getBoundingClientRect().bottom,
		);
		expect(tableBottom).toBeLessThanOrEqual(viewport.height);

		// Control actions grow with their content inside the body, which alone scrolls.
		await openTab(mode, "Control actions");
		const add = mode.getByRole("button", { name: "Add control action", exact: true });
		for (let index = 0; index < 4; index += 1) await add.dispatchEvent("click");
		for (const assign of await mode
			.getByRole("button", { name: "Add channel assignment", exact: true })
			.all())
			await assign.click();
		const actions = mode.locator(".fixture-mode-control-actions");
		await expect(actions.locator(":scope > article")).toHaveCount(4);
		expect(await scrollingRegions(mode)).toEqual(["fixture-mode-editor-body y"]);
		const actionsFit = await actions.evaluate((element) => {
			const style = getComputedStyle(element);
			return (
				element.scrollHeight <= element.clientHeight + 1 &&
				style.height !== "100%" &&
				Number.parseFloat(style.marginTop) === 0
			);
		});
		expect(actionsFit, "the actions list is as tall as its actions").toBe(true);
		await mode.locator(".fixture-mode-editor-body").evaluate((element) => {
			element.scrollTop = element.scrollHeight;
		});
		await expect(mode.getByRole("button", { name: "Remove action" }).last()).toBeInViewport();
		// Without extra content, nothing scrolls at all.
		for (const remove of (
			await mode.getByRole("button", { name: "Remove action", exact: true }).all()
		).reverse())
			await remove.click();
		expect(await scrollingRegions(mode)).toEqual([]);

		await openTab(mode, "Color");
		expect(await scrollingRegions(mode)).toEqual(["fixture-mode-editor-body y"]);
		// Every wheel slot's DMX range is fully visible and editable.
		const slot = mode.locator(".color-wheel-editor > article").first();
		for (const label of ["DMX from", "DMX to"]) {
			const input = slot.getByLabel(label, { exact: true });
			await input.scrollIntoViewIfNeeded();
			await expect(input).toBeVisible();
			const room = await input.evaluate((element) => {
				const field = element as HTMLInputElement;
				const style = getComputedStyle(field);
				const measure = document.createElement("span");
				measure.style.font = style.font;
				measure.style.position = "absolute";
				measure.textContent = "255";
				document.body.append(measure);
				const needed = measure.getBoundingClientRect().width;
				measure.remove();
				const padding =
					Number.parseFloat(style.paddingLeft) + Number.parseFloat(style.paddingRight);
				return field.clientWidth - padding - needed;
			});
			expect(room, `${label} shows a three-digit value`).toBeGreaterThan(0);
			const box = await input.boundingBox();
			const slotBox = await slot.boundingBox();
			expect(box && slotBox && box.x + box.width <= slotBox.x + slotBox.width).toBe(
				true,
			);
			await input.fill("128");
			await expect(input).toHaveValue("128");
		}
		await page.screenshot({
			path: `${artifactPaths.results}/tl-446-mode-color-${viewport.name}.png`,
		});

		await openTab(mode, "Emitters & Motion");
		expect(await scrollingRegions(mode)).toEqual([]);
	});

	test(`TL-446 @ui › the ${viewport.name} channel mapping window scrolls only its function list`, async ({
		page,
		editorUrl,
	}) => {
		await page.setViewportSize(viewport);
		const { editor } = await openEditor(page, editorUrl);
		const mode = await openModeEditor(page, editor);
		await openTab(mode, "Channels");
		await mode.getByRole("button", { name: "Edit gobo.1 mapping", exact: true }).click();
		const mapping = page.getByRole("dialog", { name: "gobo.1 mapping" });
		await expect(mapping).toBeVisible();
		const list = mapping.locator(".fixture-function-table .fixture-channel-table-wrap");
		const regions = await scrollingRegions(mapping);
		expect(regions).toEqual([`fixture-channel-table-wrap y${viewport.width < 1100 ? " x" : ""}`]);
		const [frame, wrap] = await Promise.all([
			mapping.boundingBox(),
			list.boundingBox(),
		]);
		expect(frame && wrap && wrap.y + wrap.height <= frame.y + frame.height).toBe(true);
		// The header stays on screen while the list scrolls, and the range columns stay readable.
		await list.evaluate((element) => {
			element.scrollTop = element.scrollHeight;
		});
		await expect(list.getByRole("columnheader", { name: "DMX from" })).toBeInViewport();
		await expect(list.getByRole("columnheader", { name: "DMX to" })).toBeInViewport();
		await page.screenshot({
			path: `${artifactPaths.results}/tl-446-mapping-${viewport.name}.png`,
		});
	});
}

for (const viewport of VIEWPORTS) {
	test(`TL-447 @ui › the ${viewport.name} color wheel shows and edits each slot's display color`, async ({
		page,
		editorUrl,
	}) => {
		await page.setViewportSize(viewport);
		const { editor } = await openEditor(page, editorUrl);
		const mode = await openModeEditor(page, editor);
		await openTab(mode, "Color");
		const slots = mode.locator(".color-wheel-editor > article");
		await expect(slots).toHaveCount(9);
		// Unmeasured slots already show the colour their name gives the Visualizer.
		const open = slots.first();
		const openColor = open.getByRole("button", { name: /#FFFFFF/ });
		await openColor.scrollIntoViewIfNeeded();
		await expect(openColor).toBeVisible();
		await expect(open.getByText("From the slot name", { exact: true })).toBeVisible();
		const red = slots.nth(1);
		const picker = red.getByRole("button", { name: /^#/ });
		await picker.scrollIntoViewIfNeeded();
		const [pickerBox, slotBox] = await Promise.all([
			picker.boundingBox(),
			red.boundingBox(),
		]);
		expect(
			pickerBox && slotBox && pickerBox.x + pickerBox.width <= slotBox.x + slotBox.width,
		).toBe(true);
		await picker.click();
		await page.getByRole("option", { name: "Use color #06b6d4" }).click();
		await expect(red.getByText("Defined color", { exact: true })).toBeVisible();
		await expect(red.getByRole("button", { name: /#06B6D4/ })).toBeVisible();
		await expect(
			mode.getByRole("button", { name: "Fill slots from wheel functions", exact: true }),
		).toBeEnabled();
		expect(await scrollingRegions(mode)).toEqual(["fixture-mode-editor-body y"]);
		await page.screenshot({
			path: `${artifactPaths.results}/tl-447-wheel-color-${viewport.name}.png`,
		});
	});
}

test("TL-447 @ui › the Sun Strip configures the color of each of its ten pixels", async ({
	page,
	editorUrl,
}) => {
	await page.setViewportSize({ width: 1280, height: 720 });
	const { editor } = await openEditor(
		page,
		editorUrl,
		"showtec--sunstrip-led-rgb-42206.toskfixture",
	);
	const mode = await openModeEditor(page, editor, "30 Channel");
	await openTab(mode, "Color");
	const sections = mode.locator(".fixture-color-editor > section");
	// The shared master head plus ten pixels, each pixel with its own additive system.
	await expect(sections).toHaveCount(11);
	for (let pixel = 1; pixel <= 10; pixel++) {
		const section = sections.nth(pixel);
		await expect(section.getByRole("heading", { name: `pixel ${pixel}`, exact: true })).toBeVisible();
		await expect(section.locator(".color-emitter-list > article")).toHaveCount(3);
		const names = section.getByLabel("Emitter name", { exact: true });
		for (const [index, name] of ["Red", "Green", "Blue"].entries())
			await expect(names.nth(index)).toHaveValue(name);
	}
	await expect(sections.first().locator(".color-emitter-list")).toHaveCount(0);
	// Editing one pixel leaves the others as they were.
	const second = sections.nth(2).getByLabel("Emitter name", { exact: true }).first();
	await second.fill("Pixel 2 red");
	await expect(
		sections.nth(1).getByLabel("Emitter name", { exact: true }).first(),
	).toHaveValue("Red");
	await page.screenshot({ path: `${artifactPaths.results}/tl-447-sunstrip-color.png` });
});
