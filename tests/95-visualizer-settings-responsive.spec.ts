import path from "node:path";
import { fileURLToPath } from "node:url";
import { test as base, expect, type Page } from "@playwright/test";
import { createServer, type ViteDevServer } from "vite";
import artifactResolver from "../tools/artifact-paths.cjs";

/**
 * The PreViz Editor served by its own Vite configuration, with the Tauri bridge answered in the
 * page: the Visualizer settings page is a view of one settings record and needs no desk.
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

const SETTINGS = {
	source: "planning_software",
	host: "",
	port: 0,
	user: "",
	inputOverrides: [],
	artNetInterface: null,
	sacnInterface: null,
	quality: null,
	theme: "light_on_dark",
	ambient: 0.3,
	exposure: 1,
	showLabels: true,
	showSelection: true,
	floorGrid: null,
	blender: "",
	persistence: 0.1,
	persistenceFalloff: 2,
	crowdAmount: 0.5,
	background: null,
	fog: 0.2,
	lampFogCloudiness: 0.2,
	lampFogTurbulence: 0.2,
	laserFogCloudiness: 0.2,
	laserFogTurbulence: 0.2,
	laserBrightness: 1,
};

async function openSettingsPage(page: Page, editorUrl: string) {
	await page.addInitScript((settings) => {
		// Only the settings are answered; every other request waits, so no error toast covers the page.
		(window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
			invoke: (command: string, args: { settings?: unknown }) => {
				if (command === "renderer_settings") return Promise.resolve(settings);
				if (command === "save_renderer_settings")
					return Promise.resolve(args.settings);
				return new Promise(() => undefined);
			},
			transformCallback: () => 0,
		};
	}, SETTINGS);
	await page.goto(editorUrl);
	await page.getByRole("button", { name: "Settings", exact: true }).click();
	const scroller = page.locator(".viz-renderer-settings-scroll");
	await expect(scroller.getByRole("heading", { name: "Visualizer" })).toBeVisible();
	return scroller;
}

/** Every operable control on the page. */
const CONTROLS =
	".horizontal-touch-fader, .ui-select-trigger, .ui-switch-control, input:not([type=range]):not([type=checkbox])";

for (const viewport of [
	{ name: "narrow and short", width: 560, height: 380 },
	{ name: "wide and short", width: 1280, height: 360 },
	{ name: "narrow and tall", width: 480, height: 900 },
]) {
	test(`TL-444 @ui › every Visualizer setting stays reachable in a ${viewport.name} window`, async ({
		page,
		editorUrl,
	}) => {
		await page.setViewportSize(viewport);
		const scroller = await openSettingsPage(page, editorUrl);

		const fits = await scroller.evaluate(
			(element) => element.scrollHeight <= element.clientHeight,
		);
		const controls = scroller.locator(CONTROLS);
		const count = await controls.count();
		expect(count, "the page draws its controls").toBeGreaterThan(15);

		for (let index = 0; index < count; index += 1) {
			const control = controls.nth(index);
			await control.scrollIntoViewIfNeeded();
			const reachable = await control.evaluate((element) => {
				const box = element.getBoundingClientRect();
				const hit = document.elementFromPoint(
					box.left + box.width / 2,
					box.top + box.height / 2,
				);
				return Boolean(hit && element.contains(hit));
			});
			expect(reachable, `control ${index} can be pressed`).toBe(true);
		}

		// Focusing a switch must not scroll the clipped page box, which would push the page's top
		// out of reach and its last controls below the window.
		for (const box of await scroller.locator("input[type=checkbox]").all())
			await box.focus();
		const shifted = await page
			.locator(".viz-renderer-settings")
			.evaluate((element) => element.scrollTop);
		expect(shifted).toBe(0);

		await scroller.evaluate((element) => {
			element.scrollTop = element.scrollHeight;
		});
		const bottom = await scroller.evaluate(
			(element) => element.getBoundingClientRect().bottom,
		);
		expect(bottom).toBeLessThanOrEqual(viewport.height);
		const last = scroller.locator(CONTROLS).last();
		await expect(last).toBeInViewport();
		if (!fits)
			expect(
				await scroller.evaluate((element) => element.scrollTop),
				"the page scrolls when its controls exceed the window",
			).toBeGreaterThan(0);

		const overflow = await scroller.evaluate(
			(element) => element.scrollWidth - element.clientWidth,
		);
		expect(overflow, "nothing sits beyond the page's right edge").toBe(0);
	});
}

test("TL-444 @ui › Crowd amount is a Feature, not a Picture setting", async ({
	page,
	editorUrl,
}) => {
	await page.setViewportSize({ width: 1280, height: 720 });
	const scroller = await openSettingsPage(page, editorUrl);
	const group = (title: string) =>
		scroller.locator(".viz-renderer-settings-grid > section").filter({
			has: page.getByRole("heading", { name: title, exact: true }),
		});
	await expect(group("Features")).toContainText("Crowd amount");
	await expect(group("Picture")).not.toContainText("Crowd amount");
});
