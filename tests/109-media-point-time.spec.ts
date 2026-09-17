import path from "node:path";
import { fileURLToPath } from "node:url";
import { test as base, expect, type Page } from "@playwright/test";
import { createServer, type ViteDevServer } from "vite";
import artifactResolver from "../tools/artifact-paths.cjs";

/**
 * TL-429: the Media Server's own layer page shows and takes In and Out points as mm:ss.ff at the
 * server's frame rate, and Settings stores that rate. The Media Server API is answered in the page,
 * so the test never reaches a Media Server that happens to run on this machine.
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

const OUTPUT_ID = "6b1f0c2a-1111-4a2b-8c3d-000000000001";

function layer(index: number) {
	return {
		index,
		address: { folder: 1, file: index + 1, class: "library" },
		playMode: "Loop",
		playModeDmx: 0,
		dimmer: 1,
		scaleX: 1,
		scaleY: 1,
		scalingMode: "fit",
		positionX: 0,
		positionY: 0,
		rotation: 0,
		grayscale: 0,
		volume: 1,
		tintRed: 1,
		tintGreen: 1,
		tintBlue: 1,
		speedMultiplier: "1×",
		speedMultiplierDmx: 127,
		playbackBpm: null,
		blur: 0,
		sourceStatus: { state: "ready", failure: null },
		mask: {
			address: { folder: 0, file: 0, class: "blank" },
			scaleX: 1,
			scaleY: 1,
			positionX: 0,
			positionY: 0,
			invert: false,
			opacity: 0,
			source: "luminance",
			active: false,
		},
		effects: Array.from({ length: 4 }, (_, slot) => ({
			index: slot,
			effectType: null,
			label: "None",
			enabled: false,
			mix: 0,
			supported: true,
			capabilityDetail: null,
			parameters: [],
		})),
		effectBanks: [0, 1].map((bank) => ({
			index: bank,
			select: 0,
			strength: 0,
			parameters: [0, 0, 0, 0],
		})),
		blendMode: "normal",
		strobeHz: null,
		inPoint: 0,
		outPoint: 0,
		visualizerControls: [0, 0, 0, 0],
		visualizerChannels: [],
		visualizerParameters: null,
		model: 0,
		modelPan: 0,
		modelTilt: 0,
		modelStatus: "flat",
		drawing: true,
	};
}

interface Stub {
	frameRate: number;
	playbackTakeover: boolean;
	layers: ReturnType<typeof layer>[];
	writes: Array<{ path: string; body: Record<string, unknown> }>;
}

function output(stub: Stub) {
	return {
		id: OUTPUT_ID,
		name: "Main",
		layerCount: 2,
		dmxActive: false,
		playbackTakeover: stub.playbackTakeover,
		frameRate: stub.frameRate,
		master: {
			dimmer: 1,
			volume: 1,
			tintRed: 1,
			tintGreen: 1,
			tintBlue: 1,
			flipMirror: "none",
			mask: { folder: 0, file: 0, class: "blank" },
			scaleX: 1,
			scaleY: 1,
			scalingMode: "fit",
			positionX: 0,
			positionY: 0,
			rotation: 0,
			maskPositionX: 0,
			maskPositionY: 0,
			shaperLeft: 0,
			shaperRight: 0,
			shaperTop: 0,
			shaperBottom: 0,
			shaperLeftRotation: 0,
			shaperRightRotation: 0,
			shaperTopRotation: 0,
			shaperBottomRotation: 0,
			shaperRotation: 0,
			opacityCycle: "Off",
			opacityCycleDmx: 0,
		},
		layers: stub.layers,
	};
}

function playback(stub: Stub) {
	return {
		switchHoldMillis: 500,
		maximumSwitchHoldMillis: 10_000,
		frameRate: stub.frameRate,
		maximumFrameRate: 120,
	};
}

async function stubMediaServer(page: Page): Promise<Stub> {
	const stub: Stub = {
		frameRate: 25,
		playbackTakeover: false,
		layers: [layer(0), layer(1)],
		writes: [],
	};
	// Only the server API: the app's own modules live under `/src/shared/api/` too.
	const api = (url: URL) => url.pathname.startsWith("/api/");
	await page.routeWebSocket(api, () => undefined);
	await page.route(api, async (route) => {
		const { pathname } = new URL(route.request().url());
		const json = (body: unknown) => route.fulfill({ json: body });
		const body = () =>
			(route.request().postDataJSON() ?? {}) as Record<string, unknown>;
		if (pathname === "/api/v2/outputs") return json([output(stub)]);
		if (pathname === "/api/v2/health")
			return json({ status: "ok", instance: "test", outputs: 1 });
		if (pathname === "/api/v2/catalog")
			return json({ revision: 1, itemCount: 0, folders: [] });
		if (pathname === "/api/v2/folder-presentations")
			return json({ folders: [] });
		if (pathname === "/api/v2/visualizers" || pathname === "/api/v2/models")
			return json([]);
		if (pathname === "/api/v2/playback") return json(playback(stub));
		if (pathname === "/api/v2/playback/update") {
			const edit = body();
			stub.writes.push({ path: pathname, body: edit });
			if (typeof edit.frameRate === "number") stub.frameRate = edit.frameRate;
			return json(playback(stub));
		}
		const takeover = /\/playback\/(take-over|release)$/u.exec(pathname);
		if (takeover) {
			stub.playbackTakeover = takeover[1] === "take-over";
			return json(output(stub));
		}
		const update = /\/outputs\/[^/]+\/layers\/(\d+)\/update$/u.exec(pathname);
		if (update) {
			const edit = body();
			stub.writes.push({ path: pathname, body: edit });
			const target = stub.layers[Number(update[1])];
			if (typeof edit.inPoint === "number") target.inPoint = edit.inPoint;
			if (typeof edit.outPoint === "number") target.outPoint = edit.outPoint;
			return json(output(stub));
		}
		// Everything else stays pending, so no failure toast covers the page.
	});
	return stub;
}

async function typeInto(page: Page, dialogName: string, text: string) {
	const dialog = page.getByRole("dialog", { name: dialogName });
	await expect(dialog).toBeVisible();
	for (let index = 0; index < 12; index += 1)
		await page.keyboard.press("Backspace");
	await page.keyboard.type(text);
	await page.keyboard.press("Enter");
}

test("TL-429 @ui › the layer page types In and Out points as mm:ss.ff at the server's frame rate", async ({
	page,
	mediaUrl,
}) => {
	const stub = await stubMediaServer(page);
	await page.goto(`${mediaUrl}/`, { waitUntil: "domcontentloaded" });
	// The switch's drawn track takes the pointer; the input itself carries the state.
	const takeOver = page.getByRole("switch", { name: "Take over playback" });
	await takeOver.check({ force: true });
	await expect(takeOver).toBeChecked();
	await page.getByRole("tab", { name: "Playback" }).click();
	const playbackTab = page.getByRole("tabpanel", { name: "Playback controls" });
	await expect(
		playbackTab.getByRole("heading", { name: "Playback range" }),
	).toBeVisible();
	await expect(
		playbackTab.getByRole("slider", { name: "In point" }),
	).toHaveCount(0);

	const inPoint = playbackTab.getByRole("button", {
		name: /^In point: 00:00\.00/,
	});
	await expect(inPoint).toBeVisible();
	await inPoint.click();
	await typeInto(page, "In point (mm:ss.ff)", "00:49.30");
	await expect(
		page.getByRole("dialog", { name: "In point (mm:ss.ff)" }),
	).toContainText("Frames must be below 25 at 25 fps");
	await typeInto(page, "In point (mm:ss.ff)", "00:49.09");
	await expect(
		playbackTab.getByText("00:49.09", { exact: true }),
	).toBeVisible();

	await playbackTab
		.getByRole("button", { name: /^Out point: End of clip/ })
		.click();
	await typeInto(page, "Out point (mm:ss.ff before end)", "1:00.00");
	await expect(playbackTab.getByText("01:00.00 before end")).toBeVisible();

	const layerWrites = stub.writes
		.filter((write) => write.path.includes("/layers/"))
		.map((write) => write.body);
	expect(layerWrites).toEqual([{ inPoint: 1234 }, { outPoint: 1500 }]);

	// At 30 fps the same frame counts read as other times.
	await page.goto(`${mediaUrl}/settings`, { waitUntil: "domcontentloaded" });
	await page.getByRole("tab", { name: "Libraries" }).click();
	const points = page.getByRole("article", { name: "In and Out points" });
	await expect(points).toContainText("Applies immediately");
	const rate = points.getByLabel("Frame rate (fps)");
	await expect(rate).toHaveValue("25");
	await rate.fill("30");
	await expect.poll(() => stub.frameRate).toBe(30);
	expect(stub.writes.at(-1)?.body).toMatchObject({ frameRate: 30 });

	await page.goto(`${mediaUrl}/`, { waitUntil: "domcontentloaded" });
	await page.getByRole("tab", { name: "Playback" }).click();
	await expect(
		page.getByRole("button", { name: /^In point: 00:41\.04/ }),
	).toBeVisible();
	await expect(
		page.getByRole("button", { name: /^Out point: 00:50\.00 before end/ }),
	).toBeVisible();
});
