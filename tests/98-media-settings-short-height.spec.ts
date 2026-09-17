import path from "node:path";
import { fileURLToPath } from "node:url";
import {
	test as base,
	expect,
	type Locator,
	type Page,
} from "@playwright/test";
import { createServer, type ViteDevServer } from "vite";
import artifactResolver from "../tools/artifact-paths.cjs";

/**
 * The Media Server web interface served by its own Vite configuration. Its API is answered in the
 * page, so the test never reaches a Media Server that happens to run on this machine.
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
const ADDRESSES = {
	artNetListen: "127.0.0.1:6454",
	sacnListen: "127.0.0.1:5568",
	citpListen: "127.0.0.1:4809",
	httpListen: "127.0.0.1:8080",
	speedGroupEndpoint: null,
};
const PICTURE = {
	targetKind: "monitor",
	monitorBy: "index",
	monitorValue: "0",
	fullscreen: false,
	width: 1280,
	height: 720,
	presentation: "display-synchronized",
	framesPerSecond: null,
	soundOutputKind: "system-default",
	soundOutputName: null,
	personality: "two-layers",
	protocol: "art-net",
	universe: 9,
	startAddress: 177,
};

function outputs() {
	// Two outputs make every Settings section taller than a short window.
	return ["Main", "Side"].map((name, index) => ({
		id: index === 0 ? OUTPUT_ID : `${OUTPUT_ID.slice(0, -1)}2`,
		name,
		layerCount: 0,
		layers: [],
		master: {},
		dmxActive: false,
		playbackTakeover: false,
	}));
}

function configuration(id: string, name: string) {
	return {
		id,
		name,
		...PICTURE,
		availableMonitors: [],
		availableSoundOutputs: ["System output"],
		pixelMap: {
			mode: "direct",
			zones: [],
			routes: [],
			handoffs: [],
			regions: [],
		},
		tempoSource: "playback-bpm-channel",
		speedGroup: null,
		active: PICTURE,
		picturePendingRestart: false,
		soundPendingRestart: false,
		dmxPendingRestart: false,
		takesEffectOnRestart: true,
	};
}

const NETWORK = {
	sameComputerPreset: false,
	stored: ADDRESSES,
	activeSameComputerPreset: false,
	activeStored: ADDRESSES,
	resolved: ADDRESSES,
	citpAdvertisedPort: 4809,
	takesEffectOnRestart: true,
	// A pending restart draws the Network section's own action, "Revert to current settings".
	pendingRestart: true,
	warnings: [],
};

async function openSettings(page: Page, mediaUrl: string) {
	// Only the server API: the app's own modules live under `/src/shared/api/` too.
	const api = (url: URL) => url.pathname.startsWith("/api/");
	await page.routeWebSocket(api, () => undefined);
	await page.route(api, async (route) => {
		const { pathname } = new URL(route.request().url());
		const json = (body: unknown) => route.fulfill({ json: body });
		if (pathname === "/api/v2/outputs") return json(outputs());
		if (pathname === "/api/v2/health")
			return json({ status: "ok", instance: "test", outputs: 2 });
		if (pathname === "/api/v2/network") return json(NETWORK);
		if (pathname === "/api/v2/network/update") return json(NETWORK);
		const output =
			/^\/api\/v2\/outputs\/([^/]+)\/configuration(\/update)?$/u.exec(pathname);
		if (output) {
			const found = outputs().find((entry) => entry.id === output[1]);
			return json(configuration(output[1], found?.name ?? "Main"));
		}
		// Everything else stays pending, so no failure toast covers the page.
	});
	// A pending preview image would hold the load event forever.
	await page.goto(`${mediaUrl}/settings`, { waitUntil: "domcontentloaded" });
	const content = page.getByRole("region", { name: "Settings section" });
	await expect(content.getByLabel("Art-Net")).toBeVisible();
	return content;
}

const dock = (page: Page) =>
	page.getByRole("complementary", { name: "Media Server sections" });

async function scrollState(scroller: Locator) {
	return scroller.evaluate((element) => ({
		top: element.scrollTop,
		max: element.scrollHeight - element.clientHeight,
	}));
}

/** Whether a pointer at the element's centre would land on it. */
async function pressable(target: Locator) {
	return target.evaluate((element) => {
		const box = element.getBoundingClientRect();
		const hit = document.elementFromPoint(
			box.left + box.width / 2,
			box.top + box.height / 2,
		);
		return Boolean(hit && element.contains(hit));
	});
}

async function expectAtBottom(scroller: Locator) {
	await expect
		.poll(async () => {
			const { top, max } = await scrollState(scroller);
			return max - top;
		})
		.toBeLessThanOrEqual(1);
}

const LAST_CONTROL = "input, button, .ui-select-trigger";

for (const viewport of [
	{ name: "short and narrow", width: 700, height: 300 },
	{ name: "short and wide", width: 1280, height: 420 },
]) {
	test.describe(`Settings in a ${viewport.name} window`, () => {
		test.beforeEach(async ({ page }) => {
			await page.setViewportSize(viewport);
		});

		test(`TL-455 @ui › the section scrolls to its last control with the mouse wheel and small trackpad steps (${viewport.name})`, async ({
			page,
			mediaUrl,
		}) => {
			const content = await openSettings(page, mediaUrl);
			expect((await scrollState(content)).max).toBeGreaterThan(0);
			const dockTop = (await scrollState(dock(page))).top;
			const box = await content.boundingBox();
			if (!box) throw new Error("The Settings section is not drawn");
			await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
			// A trackpad sends many small deltas; a wheel sends a few large ones.
			for (let step = 0; step < 40; step += 1) await page.mouse.wheel(0, 12);
			await expect
				.poll(async () => (await scrollState(content)).top)
				.toBeGreaterThan(0);
			for (let step = 0; step < 20; step += 1) await page.mouse.wheel(0, 240);
			await expectAtBottom(content);
			const last = content.locator(LAST_CONTROL).last();
			await expect(last).toBeInViewport({ ratio: 1 });
			expect(await pressable(last)).toBe(true);
			await expect(
				content.getByRole("button", { name: "Revert to current settings" }),
			).toBeAttached();

			// The wheel over the section never moves the dock, and the page itself never scrolls.
			expect((await scrollState(dock(page))).top).toBe(dockTop);
			expect(
				await page.evaluate(() => document.scrollingElement?.scrollTop),
			).toBe(0);
		});

		test(`TL-455 @ui › the section scrolls with the keyboard and a touch swipe (${viewport.name})`, async ({
			page,
			mediaUrl,
		}) => {
			const content = await openSettings(page, mediaUrl);
			await content.focus();
			await expect(content).toBeFocused();
			await page.keyboard.press("End");
			await expectAtBottom(content);
			await page.keyboard.press("Home");
			await expect.poll(async () => (await scrollState(content)).top).toBe(0);
			await page.keyboard.press("PageDown");
			await expect
				.poll(async () => (await scrollState(content)).top)
				.toBeGreaterThan(0);

			// Tabbing to the last control brings it into view as well.
			const revert = content.getByRole("button", {
				name: "Revert to current settings",
			});
			await revert.focus();
			await expect(revert).toBeInViewport({ ratio: 1 });

			await content.evaluate((element) => {
				element.scrollTop = 0;
			});
			const box = await content.boundingBox();
			if (!box) throw new Error("The Settings section is not drawn");
			const session = await page.context().newCDPSession(page);
			const x = box.x + box.width / 2;
			for (let swipe = 0; swipe < 8; swipe += 1)
				await session.send("Input.synthesizeScrollGesture", {
					x,
					y: box.y + box.height - 20,
					yDistance: -Math.max(80, box.height - 60),
					gestureSourceType: "touch",
					speed: 2400,
				});
			await expectAtBottom(content);
			await expect(content.locator(LAST_CONTROL).last()).toBeInViewport({
				ratio: 1,
			});
		});

		test(`TL-455 @ui › the dock scrolls as one piece and keeps the selected destination in view (${viewport.name})`, async ({
			page,
			mediaUrl,
		}) => {
			await openSettings(page, mediaUrl);
			const sidebar = dock(page);
			const settings = sidebar.getByRole("button", { name: "Settings" });
			await expect(settings).toHaveAttribute("aria-current", "page");
			await expect(settings).toBeInViewport({ ratio: 1 });
			expect(await pressable(settings)).toBe(true);

			// The destination list is not a second scroller inside the dock.
			const list = sidebar.getByRole("navigation", {
				name: "Media Server destinations",
			});
			expect((await scrollState(list)).max).toBe(0);

			const box = await sidebar.boundingBox();
			if (!box) throw new Error("The dock is not drawn");
			await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
			await page.mouse.wheel(0, -2000);
			await expect.poll(async () => (await scrollState(sidebar)).top).toBe(0);
			await expect(
				sidebar.getByRole("button", { name: "Playback" }),
			).toBeInViewport();
			await page.mouse.wheel(0, 2000);
			await expectAtBottom(sidebar);
			await expect(settings).toBeInViewport();
			await expect(sidebar.getByText("Take over playback")).toBeInViewport();
			await expect(sidebar.getByText("Connected")).toBeInViewport();
			expect(
				await page.evaluate(() => document.scrollingElement?.scrollTop ?? 0),
			).toBe(0);
		});

		test(`TL-455 @ui › switching sections starts the chosen section at its top (${viewport.name})`, async ({
			page,
			mediaUrl,
		}) => {
			const content = await openSettings(page, mediaUrl);
			await content.evaluate((element) => {
				element.scrollTop = element.scrollHeight;
			});
			await page.getByRole("tab", { name: "Picture" }).click();
			await expect(
				content.getByRole("heading", { name: "Picture" }),
			).toBeInViewport();
			expect((await scrollState(content)).top).toBe(0);
			await expect(page.getByRole("tab", { name: "Picture" })).toBeInViewport();
			await content.focus();
			await page.keyboard.press("End");
			await expectAtBottom(content);
			await expect(content.locator(LAST_CONTROL).last()).toBeInViewport({
				ratio: 1,
			});
		});
	});
}
