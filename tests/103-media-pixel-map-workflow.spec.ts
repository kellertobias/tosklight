import path from "node:path";
import { fileURLToPath } from "node:url";
import {
	test as base,
	type CDPSession,
	expect,
	type Locator,
	type Page,
} from "@playwright/test";
import { createServer, type ViteDevServer } from "vite";
import artifactResolver from "../tools/artifact-paths.cjs";

/**
 * PIXEL-009: the Pixel Map dock of the Media Server web interface, served by its own Vite
 * configuration. The API is answered in the page by a small in-memory server that keeps what was
 * saved, so a reload proves the map comes back from the server's copy and the test never reaches a
 * Media Server that happens to run on this machine.
 */
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const { artifactPaths } = artifactResolver;

const test = base.extend<object, { mediaUrl: string }>({
	mediaUrl: [
		// biome-ignore lint/correctness/noEmptyPattern: Playwright fixture callbacks require destructuring.
		async ({}, use) => {
			const server: ViteDevServer = await createServer({
				configFile: path.join(root, "apps/media/vite.config.ts"),
				root: path.join(root, "apps/media"),
				cacheDir: `${artifactPaths.viteCache}/media-playwright`,
				server: { port: 0, strictPort: false, host: "127.0.0.1" },
				logLevel: "error",
			});
			await server.listen();
			const address = server.httpServer?.address();
			if (!address || typeof address === "string")
				throw new Error("The Media Server dev server has no port");
			try {
				await use(`http://127.0.0.1:${address.port}`);
			} finally {
				await server.close();
			}
		},
		{ scope: "worker", timeout: 120_000 },
	],
});

const HDMI_ID = "6b1f0c2a-1111-4a2b-8c3d-000000000001";
const PROJECTOR_ID = "6b1f0c2a-1111-4a2b-8c3d-000000000002";

type Point = { x: number; y: number };
type Shape = { id: string; name: string; start: Point; end: Point };
type PixelMap = {
	mode: string;
	zones: (Shape & Record<string, unknown>)[];
	routes: Record<string, unknown>[];
	handoffs: Record<string, unknown>[];
	regions: (Shape & Record<string, unknown>)[];
};

const emptyMap = (): PixelMap => ({
	mode: "direct",
	zones: [],
	routes: [],
	handoffs: [],
	regions: [],
});

const strip = (
	id: string,
	name: string,
	left: number,
	universe: number,
	startAddress: number,
) => ({
	id,
	name,
	start: { x: left, y: 0 },
	end: { x: left + 0.05, y: 1 },
	columns: 1,
	rows: 30,
	layout: { name: "RGB", components: ["red", "green", "blue"] },
	order: "column-major",
	universe,
	startAddress,
	enabled: true,
	footprint: 90,
});

const route = (universe: number, protocol: string) => ({
	id: `route-${universe}`,
	name: `Universe ${universe}`,
	protocol,
	universe,
	destination: null,
	enabled: true,
});

/** The reference layout, already stored, for the tests that drive the picture. */
const referenceMap = (): PixelMap => ({
	mode: "direct",
	zones: [
		strip("zone-left", "Left strip", 0, 1, 1),
		strip("zone-right", "Right strip", 0.95, 2, 1),
	],
	routes: [route(1, "art-net"), route(2, "sacn")],
	handoffs: [],
	regions: [
		{
			id: "region-centre",
			name: "Centre",
			start: { x: 0.3, y: 0.2 },
			end: { x: 0.7, y: 0.8 },
			rotation: "clockwise-90",
			fit: "fill",
			enabled: true,
		},
	],
});

const PICTURE = {
	targetKind: "monitor",
	monitorBy: "index",
	monitorValue: "0",
	fullscreen: false,
	width: 1920,
	height: 1080,
	presentation: "display-synchronized",
	framesPerSecond: null,
	soundOutputKind: "system-default",
	soundOutputName: null,
	personality: "two-layers",
	protocol: "art-net",
	universe: 9,
	startAddress: 177,
};

const OUTPUTS = [
	{ id: HDMI_ID, name: "HDMI" },
	{ id: PROJECTOR_ID, name: "Projector" },
];

// A picture to place shapes against: the preview route answers with this instead of a video frame.
const PREVIEW = `<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360"><defs><linearGradient id="g"><stop offset="0" stop-color="#203a8f"/><stop offset="1" stop-color="#b3206b"/></linearGradient></defs><rect width="640" height="360" fill="url(#g)"/></svg>`;

/** An in-memory Media Server holding one pixel map per output, and every edit it received. */
async function serve(page: Page, initial: Record<string, PixelMap>) {
	const stored: Record<string, PixelMap> = {
		[HDMI_ID]: emptyMap(),
		[PROJECTOR_ID]: emptyMap(),
		...structuredClone(initial),
	};
	const edits: { output: string; body: Record<string, unknown> }[] = [];
	const configuration = (id: string) => ({
		id,
		name: OUTPUTS.find((output) => output.id === id)?.name ?? "HDMI",
		...PICTURE,
		...(id === PROJECTOR_ID ? { width: 1280, height: 800 } : {}),
		availableMonitors: [],
		availableSoundOutputs: ["System output"],
		pixelMap: stored[id],
		tempoSource: "playback-bpm-channel",
		speedGroup: null,
		active: PICTURE,
		picturePendingRestart: false,
		soundPendingRestart: false,
		dmxPendingRestart: false,
		takesEffectOnRestart: false,
		restartFields: [],
	});
	// Only the server API: the app's own modules live under `/src/shared/api/` too.
	const api = (url: URL) => url.pathname.startsWith("/api/");
	await page.routeWebSocket(api, () => undefined);
	await page.route(api, async (request) => {
		const { pathname } = new URL(request.request().url());
		const json = (body: unknown) => request.fulfill({ json: body });
		if (pathname === "/api/v2/outputs")
			return json(
				OUTPUTS.map((output) => ({
					...output,
					layerCount: 0,
					layers: [],
					master: {},
					dmxActive: false,
					playbackTakeover: false,
				})),
			);
		if (pathname === "/api/v2/health")
			return json({ status: "ok", instance: "test", outputs: 2 });
		if (/^\/api\/v2\/outputs\/[^/]+\/preview$/u.test(pathname))
			return request.fulfill({
				contentType: "image/svg+xml",
				body: PREVIEW,
			});
		const output =
			/^\/api\/v2\/outputs\/([^/]+)\/configuration(\/update)?$/u.exec(pathname);
		if (output) {
			const [, id, update] = output;
			if (update) {
				const body = request.request().postDataJSON() as Record<
					string,
					unknown
				>;
				edits.push({ output: id, body });
				if (body.pixelMap) stored[id] = body.pixelMap as PixelMap;
			}
			return json(configuration(id));
		}
		// Everything else stays pending, so no failure toast covers the page.
	});
	return { stored, edits };
}

async function openPixelMap(page: Page, mediaUrl: string) {
	// A pending request would hold the load event forever.
	await page.goto(`${mediaUrl}/pixel-map`, { waitUntil: "domcontentloaded" });
	await expect(picture(page)).toBeVisible();
}

const picture = (page: Page) =>
	page.getByRole("group", { name: /^Output picture, / });
const shape = (page: Page, name: string) =>
	picture(page).getByRole("button", { name });
const row = (page: Page, name: string) =>
	page.getByRole("row", { name, exact: true });
const field = (page: Page, label: string) =>
	page.getByLabel(label, { exact: true });
const handle = (page: Page, corner: string) =>
	picture(page).locator(`.media-pixel-handle[data-handle="${corner}"]`);
const saveButton = (page: Page) =>
	page.getByRole("button", { name: "Save pixel map" });

async function setField(page: Page, label: string, value: string) {
	await field(page, label).fill(value);
	await expect(field(page, label)).toHaveValue(value);
}

/** Renames a row; its fields are labelled by the new name from then on. */
async function rename(page: Page, from: string, to: string) {
	await field(page, `${from} name`).fill(to);
	await expect(field(page, `${to} name`)).toHaveValue(to);
}

async function choose(page: Page, label: string, option: string) {
	await page.getByRole("button", { name: label, exact: true }).click();
	await page.getByRole("option", { name: option, exact: true }).click();
}

async function numberOf(page: Page, label: string) {
	return Number(await field(page, label).inputValue());
}

async function centre(target: Locator) {
	const box = await target.boundingBox();
	if (!box) throw new Error("The target is not drawn");
	return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

async function mouseDrag(page: Page, from: Point, dx: number, dy: number) {
	await page.mouse.move(from.x, from.y);
	await page.mouse.down();
	for (let step = 1; step <= 8; step += 1)
		await page.mouse.move(from.x + (dx * step) / 8, from.y + (dy * step) / 8);
	await page.mouse.up();
}

async function touchDrag(
	session: CDPSession,
	from: Point,
	dx: number,
	dy: number,
) {
	await session.send("Input.dispatchTouchEvent", {
		type: "touchStart",
		touchPoints: [{ x: from.x, y: from.y }],
	});
	for (let step = 1; step <= 8; step += 1)
		await session.send("Input.dispatchTouchEvent", {
			type: "touchMove",
			touchPoints: [
				{ x: from.x + (dx * step) / 8, y: from.y + (dy * step) / 8 },
			],
		});
	await session.send("Input.dispatchTouchEvent", {
		type: "touchEnd",
		touchPoints: [],
	});
}

async function canvasSize(page: Page) {
	const box = await picture(page).boundingBox();
	if (!box) throw new Error("The picture is not drawn");
	return box;
}

test.describe("docs/testing/14-media-and-running-panes.md", () => {
	test.beforeEach(async ({ page }) => {
		await page.setViewportSize({ width: 1280, height: 800 });
	});

	test("PIXEL-009 @ui › the dock shows the picture beside tables that fit without sideways scrolling", async ({
		page,
		mediaUrl,
	}) => {
		await serve(page, { [HDMI_ID]: referenceMap() });
		await openPixelMap(page, mediaUrl);
		await expect(
			page
				.getByRole("complementary", { name: "Media Server sections" })
				.getByRole("button", { name: "Pixel Map" }),
		).toHaveAttribute("aria-current", "page");
		const tabs = page.getByRole("tablist");
		await expect(tabs.getByRole("tab")).toHaveText([
			"Display Regions",
			"Pixel Zones",
		]);

		// The picture sits beside the configuration, not above it.
		const pictureBox = await canvasSize(page);
		const tablesBox = await page
			.locator(".media-pixel-map-tables")
			.boundingBox();
		expect(tablesBox?.x).toBeGreaterThan(pictureBox.x + pictureBox.width - 1);
		await expect(page.locator(".media-pixel-canvas-frame")).toBeVisible();

		const tables = page.locator(".media-pixel-table-scroll");
		const expectNoSidewaysScroll = async () => {
			for (const table of await tables.all()) {
				const overflow = await table.evaluate(
					(element) => element.scrollWidth - element.clientWidth,
				);
				expect(overflow).toBeLessThanOrEqual(1);
			}
		};
		await expect(
			page.getByRole("region", { name: "Display region placement" }),
		).toBeVisible();
		await expect(
			page.getByRole("region", { name: "Display region presentation" }),
		).toBeVisible();
		await expectNoSidewaysScroll();

		await tabs.getByRole("tab", { name: "Pixel Zones" }).click();
		await expect(
			page.getByRole("region", { name: "Pixel zone placement" }),
		).toBeVisible();
		await expect(
			page.getByRole("region", { name: "Pixel zone patch" }),
		).toBeVisible();
		await expectNoSidewaysScroll();
		// Every touch target in the tables is finger-sized.
		for (const control of await page
			.locator(
				".media-pixel-table button, .media-pixel-table input:not([type=checkbox])",
			)
			.all()) {
			const box = await control.boundingBox();
			if (!box) continue;
			expect(box.height).toBeGreaterThanOrEqual(32);
		}
	});

	test("PIXEL-009 @ui › rows and shapes stay selected together in both directions", async ({
		page,
		mediaUrl,
	}) => {
		await serve(page, { [HDMI_ID]: referenceMap() });
		await openPixelMap(page, mediaUrl);
		await expect(shape(page, "Centre display region")).toHaveAttribute(
			"aria-pressed",
			"true",
		);
		await expect(row(page, "Centre")).toHaveAttribute("aria-selected", "true");
		// The zones are drawn for reference but do not answer a press on this tab.
		await expect(shape(page, "Left strip pixel zone")).toBeDisabled();

		await page.getByRole("tab", { name: "Pixel Zones" }).click();
		await expect(row(page, "Left strip")).toHaveAttribute(
			"aria-selected",
			"true",
		);
		await expect(row(page, "Left strip patch")).toHaveAttribute(
			"aria-selected",
			"true",
		);

		// Picture to tables: a tap selects the zone in both tables and does not move it.
		await shape(page, "Right strip pixel zone").click();
		await expect(shape(page, "Right strip pixel zone")).toHaveAttribute(
			"aria-pressed",
			"true",
		);
		await expect(shape(page, "Left strip pixel zone")).toHaveAttribute(
			"aria-pressed",
			"false",
		);
		await expect(row(page, "Right strip")).toHaveAttribute(
			"aria-selected",
			"true",
		);
		await expect(row(page, "Right strip patch")).toHaveAttribute(
			"aria-selected",
			"true",
		);
		expect(await numberOf(page, "Right strip left")).toBe(0.95);
		await expect(saveButton(page)).toBeDisabled();

		// Tables to picture, from the patch table as well as the placement table.
		await row(page, "Left strip patch")
			.getByText("Left strip", { exact: true })
			.click();
		await expect(shape(page, "Left strip pixel zone")).toHaveAttribute(
			"aria-pressed",
			"true",
		);
		await expect(row(page, "Left strip")).toHaveAttribute(
			"aria-selected",
			"true",
		);
		await field(page, "Right strip output address").click();
		await expect(shape(page, "Right strip pixel zone")).toHaveAttribute(
			"aria-pressed",
			"true",
		);
	});

	test("PIXEL-009 @ui › shapes move and resize by mouse, finger, and keyboard", async ({
		page,
		mediaUrl,
	}) => {
		await serve(page, { [HDMI_ID]: referenceMap() });
		await openPixelMap(page, mediaUrl);
		const canvas = await canvasSize(page);
		const centreRegion = shape(page, "Centre display region");

		// A mouse drag on the body moves the region by the dragged share of the canvas.
		await mouseDrag(
			page,
			await centre(centreRegion),
			canvas.width * 0.1,
			canvas.height * 0.1,
		);
		await expect.poll(() => numberOf(page, "Centre left")).toBeCloseTo(0.4, 1);
		expect(await numberOf(page, "Centre top")).toBeCloseTo(0.3, 1);
		// Its size is kept exactly.
		expect(
			(await numberOf(page, "Centre right")) -
				(await numberOf(page, "Centre left")),
		).toBeCloseTo(0.4, 3);
		await expect(page.getByText("Unsaved changes")).toBeVisible();

		// A move stops at the canvas edge rather than pushing the region off it.
		await mouseDrag(page, await centre(centreRegion), canvas.width, 0);
		await expect.poll(() => numberOf(page, "Centre right")).toBe(1);
		expect(await numberOf(page, "Centre left")).toBeCloseTo(0.6, 3);

		// A corner handle resizes; it cannot cross the opposite corner.
		await mouseDrag(
			page,
			await centre(handle(page, "top-left")),
			-canvas.width * 0.2,
			0,
		);
		await expect.poll(() => numberOf(page, "Centre left")).toBeCloseTo(0.4, 1);
		expect(await numberOf(page, "Centre right")).toBe(1);
		await mouseDrag(
			page,
			await centre(handle(page, "bottom-right")),
			-canvas.width,
			-canvas.height,
		);
		await expect
			.poll(async () => {
				const left = await numberOf(page, "Centre left");
				const right = await numberOf(page, "Centre right");
				return Number((right - left).toFixed(3));
			})
			.toBe(0.01);

		// A finger drags a zone on the other tab, without scrolling the page.
		await page.getByRole("tab", { name: "Pixel Zones" }).click();
		const rightStrip = shape(page, "Right strip pixel zone");
		const session = await page.context().newCDPSession(page);
		await session.send("Emulation.setTouchEmulationEnabled", {
			enabled: true,
			maxTouchPoints: 1,
		});
		const start = await centre(rightStrip);
		await touchDrag(session, start, -canvas.width * 0.25, 0);
		await expect(rightStrip).toHaveAttribute("aria-pressed", "true");
		await expect
			.poll(() => numberOf(page, "Right strip left"))
			.toBeCloseTo(0.7, 1);
		expect(await numberOf(page, "Right strip top")).toBe(0);
		expect(
			await page.evaluate(() => document.scrollingElement?.scrollTop),
		).toBe(0);
		await session.send("Emulation.setTouchEmulationEnabled", {
			enabled: false,
		});

		// The keyboard nudges the selected shape, and Alt with an arrow resizes it.
		const before = await numberOf(page, "Right strip left");
		await rightStrip.focus();
		await page.keyboard.press("ArrowLeft");
		await expect
			.poll(() => numberOf(page, "Right strip left"))
			.toBeCloseTo(before - 0.01, 3);
		await page.keyboard.press("Alt+ArrowDown");
		expect(await numberOf(page, "Right strip bottom")).toBe(1);
		await page.keyboard.press("Alt+ArrowLeft");
		await expect
			.poll(async () => {
				const left = await numberOf(page, "Right strip left");
				const right = await numberOf(page, "Right strip right");
				return Number((right - left).toFixed(3));
			})
			.toBe(0.04);
		// Moving a zone changes where it samples, never its patch.
		expect(await numberOf(page, "Right strip output address")).toBe(1);
	});

	test("PIXEL-009 @ui › the reference layout is built on two outputs, saved, and reopened", async ({
		page,
		mediaUrl,
	}) => {
		const server = await serve(page, {});
		await openPixelMap(page, mediaUrl);
		await expect(page.getByText(/No display region yet/)).toBeVisible();

		// HDMI: a centre slice turned clockwise.
		await page.getByRole("button", { name: "Add display region" }).click();
		await rename(page, "Screen 1", "Centre");
		await setField(page, "Centre left", "0.333");
		await setField(page, "Centre right", "0.667");
		await choose(page, "Centre rotation", "Turned clockwise");
		await expect(shape(page, "Centre display region")).toHaveAttribute(
			"aria-pressed",
			"true",
		);

		// Left and right RGB strips, one over Art-Net and one over sACN.
		await page.getByRole("tab", { name: "Pixel Zones" }).click();
		await page.getByRole("button", { name: "Add output route" }).click();
		await page.getByRole("button", { name: "Add output route" }).click();
		await choose(page, "Universe 2 protocol", "sACN");
		for (const [name, left, right, universe] of [
			["Left strip", "0", "0.05", "1"],
			["Right strip", "0.95", "1", "2"],
		]) {
			await page.getByRole("button", { name: "Add pixel zone" }).click();
			const added = `Zone ${name === "Left strip" ? 1 : 2}`;
			await rename(page, added, name);
			await setField(page, `${name} left`, left);
			await setField(page, `${name} right`, right);
			await setField(page, `${name} top`, "0");
			await setField(page, `${name} bottom`, "1");
			await setField(page, `${name} pixels across`, "1");
			await setField(page, `${name} pixels down`, "30");
			await setField(page, `${name} output universe`, universe);
			await setField(page, `${name} output address`, "1");
			await expect(
				row(page, `${name} patch`).getByText("90", { exact: true }),
			).toBeVisible();
		}
		await expect(page.getByLabel("Pixel map problems")).toHaveCount(0);
		await saveButton(page).click();
		await expect(page.getByText("Saved · Applies immediately")).toBeVisible();

		// The save carries only the pixel map, so the source canvas is untouched.
		expect(server.edits).toHaveLength(1);
		expect(server.edits[0].output).toBe(HDMI_ID);
		expect(Object.keys(server.edits[0].body).sort()).toEqual([
			"pixelMap",
			"requestId",
		]);
		const hdmi = server.stored[HDMI_ID];
		expect(hdmi.regions).toEqual([
			expect.objectContaining({
				name: "Centre",
				start: { x: 0.333, y: 0 },
				end: { x: 0.667, y: 1 },
				rotation: "clockwise-90",
			}),
		]);
		expect(hdmi.zones).toEqual([
			expect.objectContaining({
				name: "Left strip",
				universe: 1,
				footprint: 90,
				layout: { name: "RGB", components: ["red", "green", "blue"] },
			}),
			expect.objectContaining({ name: "Right strip", universe: 2 }),
		]);
		expect(hdmi.routes).toEqual([
			expect.objectContaining({ universe: 1, protocol: "art-net" }),
			expect.objectContaining({ universe: 2, protocol: "sacn" }),
		]);

		// Projector: two independent regions overlapping the HDMI slice.
		await page
			.locator(".media-pixel-map-toolbar")
			.getByRole("button", { name: "HDMI" })
			.click();
		await page.getByRole("option", { name: "Projector" }).click();
		await expect(
			page.getByRole("group", { name: "Output picture, 1280 by 800" }),
		).toBeVisible();
		await page.getByRole("tab", { name: "Display Regions" }).click();
		for (const [name, left, right] of [
			["Projection left", "0", "0.5"],
			["Projection right", "0.4", "1"],
		]) {
			await page.getByRole("button", { name: "Add display region" }).click();
			const added = `Screen ${name === "Projection left" ? 1 : 2}`;
			await rename(page, added, name);
			await setField(page, `${name} left`, left);
			await setField(page, `${name} right`, right);
		}
		await choose(page, "Projection right fit", "Fit on the screen");
		await saveButton(page).click();
		await expect(page.getByText("Saved · Applies immediately")).toBeVisible();
		expect(server.edits).toHaveLength(2);
		expect(server.edits[1].output).toBe(PROJECTOR_ID);
		expect(Object.keys(server.edits[1].body).sort()).toEqual([
			"pixelMap",
			"requestId",
		]);
		expect(server.stored[PROJECTOR_ID].regions).toHaveLength(2);
		expect(server.stored[PROJECTOR_ID].zones).toEqual([]);
		// The projector's edit left the HDMI map exactly as it was.
		expect(server.stored[HDMI_ID]).toEqual(hdmi);

		// Reload: both outputs reopen with exactly what was saved.
		await page.reload({ waitUntil: "domcontentloaded" });
		await expect(
			page.getByRole("group", { name: "Output picture, 1920 by 1080" }),
		).toBeVisible();
		await expect(field(page, "Centre left")).toHaveValue("0.333");
		await expect(
			page.getByRole("button", { name: "Centre rotation" }),
		).toHaveText("Turned clockwise");
		await expect(shape(page, "Centre display region")).toBeVisible();
		await page.getByRole("tab", { name: "Pixel Zones" }).click();
		await expect(field(page, "Left strip pixels down")).toHaveValue("30");
		await expect(field(page, "Right strip left")).toHaveValue("0.95");
		await expect(
			page.getByRole("button", { name: "Universe 2 protocol" }),
		).toHaveText("sACN");
		await expect(shape(page, "Left strip pixel zone")).toBeVisible();
		await expect(shape(page, "Right strip pixel zone")).toBeVisible();
		await expect(saveButton(page)).toBeDisabled();

		await page
			.locator(".media-pixel-map-toolbar")
			.getByRole("button", { name: "HDMI" })
			.click();
		await page.getByRole("option", { name: "Projector" }).click();
		await page.getByRole("tab", { name: "Display Regions" }).click();
		await expect(field(page, "Projection right left")).toHaveValue("0.4");
		await expect(
			page.getByRole("button", { name: "Projection right fit" }),
		).toHaveText("Fit on the screen");
		await expect(shape(page, "Projection left display region")).toBeVisible();
		await expect(
			picture(page).getByRole("button", { name: /pixel zone$/ }),
		).toHaveCount(0);
	});

	test("PIXEL-009 @ui › desk merge and validation remain in the workflow", async ({
		page,
		mediaUrl,
	}) => {
		const server = await serve(page, { [HDMI_ID]: referenceMap() });
		await openPixelMap(page, mediaUrl);
		await page.getByRole("tab", { name: "Pixel Zones" }).click();

		// A zone on a universe no route carries is named and blocks the save.
		await setField(page, "Right strip output universe", "7");
		await expect(page.getByLabel("Pixel map problems")).toContainText(
			"Right strip sends universe 7, which no enabled output route carries.",
		);
		await expect(saveButton(page)).toBeDisabled();
		await setField(page, "Right strip output universe", "2");
		await expect(page.getByLabel("Pixel map problems")).toHaveCount(0);

		await choose(page, "Direct Media Server output", "Desk merge");
		await shape(page, "Right strip pixel zone").click();
		const handoff = page.getByRole("group", {
			name: "Right strip desk handoff",
		});
		await expect(handoff.getByLabel("Desk input universe")).toHaveValue("2");
		await saveButton(page).click();
		await expect(page.getByText("Saved · Applies immediately")).toBeVisible();
		expect(server.stored[HDMI_ID].mode).toBe("desk-merge");
		expect(server.stored[HDMI_ID].handoffs).toHaveLength(2);
	});
});
