import path from "node:path";
import { fileURLToPath } from "node:url";
import { test as base, expect, type Page, type WebSocketRoute } from "@playwright/test";
import { createServer, type ViteDevServer } from "vite";
import artifactResolver from "../tools/artifact-paths.cjs";
import type { AutomationView, Playlist } from "../apps/media/src/shared/api/automation";
import { DEFAULT_PLAYLIST_TRANSFORM } from "../apps/media/src/features/automation/playlistTransform";

// Acceptance contract: docs/testing/45-pixel-playlist-scheduling.md (PIXEL-020).
// These browser journeys exercise the real Pixel UI against route-backed configuration and
// telemetry fixtures. They do not prove media decoding, GPU crossfades or actual DMX reception.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const { artifactPaths } = artifactResolver;
const test = base.extend<object, { mediaUrl: string }>({
	mediaUrl: [async ({}, use) => {
		const server: ViteDevServer = await createServer({
			configFile: path.join(root, "apps/media/vite.config.ts"), root: path.join(root, "apps/media"),
			cacheDir: `${artifactPaths.viteCache}/media-playlist-playwright`,
			server: { port: 0, strictPort: false, host: "127.0.0.1" }, logLevel: "error",
		});
		await server.listen();
		const address = server.httpServer?.address();
		if (!address || typeof address === "string") throw new Error("Pixel fixture has no listening port");
		try { await use(`http://127.0.0.1:${address.port}`); } finally { await server.close(); }
	}, { scope: "worker", timeout: 120_000 }],
});

const OUTPUT = "6b1f0c2a-1111-4a2b-8c3d-000000000001";
const PLAYLIST = "6b1f0c2a-1111-4a2b-8c3d-000000000002";
const IMAGE = "6b1f0c2a-1111-4a2b-8c3d-000000000003";
const VIDEO = "6b1f0c2a-1111-4a2b-8c3d-000000000004";
const CREATED_PLAYLIST = "6b1f0c2a-1111-4a2b-8c3d-000000000005";

function initialPlaylist(): Playlist {
	return { id: PLAYLIST, name: "Lobby information", enabled: true, priority: 5, layers: [2, 3], masterDimmer: 1, masterVolume: 1, defaultFadeMillis: 1000, auxiliaryLayers: [], entries: [
		{ id: "image-entry", assetId: IMAGE, plays: 1, holdMillis: 10000, fadeMillis: 1000, transform: { ...DEFAULT_PLAYLIST_TRANSFORM } },
		{ id: "video-entry", assetId: VIDEO, plays: 2, holdMillis: null, fadeMillis: 500, transform: { ...DEFAULT_PLAYLIST_TRANSFORM } },
	] };
}

async function stubPixel(page: Page) {
	const view: AutomationView = { automationRevision: 1, configuration: { playlists: [initialPlaylist()], events: [], outputs: [{ outputId: OUTPUT, enabled: false, fallbackPlaylistId: null, dmxSilenceSeconds: 10 }] }, timeZone: "Europe/Berlin", status: [{ outputId: OUTPUT, owner: "disabled", playlistId: null, eventId: null, playlistName: null, entryIndex: null, play: 0, phase: "idle", error: null, dmxAgeMillis: null, resumeInMillis: null, lastOccurrenceOutcome: null, lastOccurrenceName: null }], upcoming: [] };
	const sockets = new Set<WebSocketRoute>();
	const actions: Array<Record<string, unknown>> = [];
	const edits: Array<{ path: string; body: Record<string, unknown> }> = [];
	let readGate: Promise<void> | undefined;
	const output = { id: OUTPUT, name: "Main display", layerCount: 8, layers: [], master: {}, dmxActive: false, playbackTakeover: false, ownershipGeneration: 0 };
	let previewId: string | null = null;
	const previewResult = (action: Record<string, unknown>) => {
		const request = action.request as { outputId: string; previewId: string; sequence: number; playlist: Playlist; entryId: string };
		if (action.type === "automation-preview-step") {
			if (previewId !== request.previewId) output.ownershipGeneration++;
			previewId = request.previewId; output.playbackTakeover = true;
			return { active: { outputId: OUTPUT, previewId, sequence: request.sequence, ownershipGeneration: output.ownershipGeneration, playlistId: request.playlist.id, entryId: request.entryId }, output };
		}
		if (previewId === request.previewId) { previewId = null; output.playbackTakeover = false; output.ownershipGeneration++; }
		return { active: null, output };
	};
	const publish = () => { for (const socket of sockets) socket.send(JSON.stringify({ audio: { bpm: 0, capturing: false }, deskIdentity: null, automationRevision: view.automationRevision, automation: view.status })); };
	await page.routeWebSocket((url) => url.pathname === "/api/v2/telemetry", (socket) => {
		sockets.add(socket);
		socket.onClose(() => sockets.delete(socket));
		socket.onMessage((message) => {
			const action = JSON.parse(String(message)); actions.push(action);
			if (action.type === "automation-set-enabled") {
				view.configuration.outputs[0].enabled = Boolean(action.enabled);
				view.status[0].owner = action.enabled ? "fallback" : "disabled";
				view.status[0].playlistId = action.enabled ? PLAYLIST : null;
				view.status[0].playlistName = action.enabled ? "Lobby information" : null;
				view.automationRevision += 1;
			}
			const preview = action.type === "automation-preview-step" || action.type === "automation-stop-preview" ? previewResult(action) : undefined;
			socket.send(JSON.stringify({ type: "automation-action-result", actionId: action.actionId, ok: true, message: null, preview }));
			publish();
		});
		publish();
	});
	await page.route((url) => url.pathname.startsWith("/api/"), async (route) => {
		const pathname = new URL(route.request().url()).pathname;
		const json = (value: unknown) => route.fulfill({ json: value });
		if (pathname === "/api/v2/health") return json({ status: "ok", instance: "Playlist acceptance", outputs: 1 });
		if (pathname === "/api/v2/outputs") return json([output]);
		if (pathname === `/api/v2/outputs/${OUTPUT}/preview`) return route.fulfill({ path: path.join(root, "docs/marketing/assets/stills/pixel-eight-layer-composite.png") });
		if (pathname === "/api/v2/catalog") return json({ revision: 1, itemCount: 2, folders: [{ folder: 1, name: "Marketing", items: [
			{ id: IMAGE, file: 1, name: "Opening hours", kind: "image", width: 1920, height: 1080, frames: 1, intrinsicBpm: null, enabled: true },
			{ id: VIDEO, file: 2, name: "Venue film", kind: "video", width: 1920, height: 1080, frames: 250, durationMillis: 10000, intrinsicBpm: null, enabled: true },
		] }] });
		if (pathname.endsWith("/thumbnail")) return route.fulfill({ path: path.join(root, "docs/marketing/assets/stills/pixel-eight-layer-composite.png") });
		if (pathname === "/api/v2/effects" || pathname === "/api/v2/models") return json([]);
		if (pathname === "/api/v2/automation") { await readGate; return json(view); }
		const edit = (route.request().postDataJSON() ?? {}) as Record<string, unknown>;
		if (pathname.startsWith("/api/v2/automation/")) {
			edits.push({ path: pathname, body: edit });
			const { requestId: _requestId, ...change } = edit;
			if (pathname === "/api/v2/automation/playlists/create") {
				view.configuration.playlists.push({ ...change, id: "another-browser", name: "Another browser's draft" } as unknown as Playlist, { ...change, id: CREATED_PLAYLIST } as unknown as Playlist);
			} else if (/^\/api\/v2\/automation\/playlists\/[^/]+\/update$/.test(pathname)) {
				const id = pathname.split("/").at(-2);
				Object.assign(view.configuration.playlists.find((playlist) => playlist.id === id)!, change);
			}
			else if (pathname === `/api/v2/automation/outputs/${OUTPUT}/update`) Object.assign(view.configuration.outputs[0], change);
			else if (pathname === "/api/v2/automation/events/create") {
				view.configuration.events.push({ ...change, id: "scheduled-event" } as unknown as AutomationView["configuration"]["events"][number]);
				view.upcoming = [{ eventId: "scheduled-event", playlistId: PLAYLIST, outputId: OUTPUT, name: String(change.name), start: "2026-11-23T22:00:00+01:00", end: "2026-11-24T02:00:00+01:00", priority: 5, winning: true, reason: "Highest priority" }];
			}
			else return route.fulfill({ status: 404, json: { code: "unexpected-fixture-route", message: pathname } });
			view.automationRevision += 1; await json(view); publish(); return;
		}
		return route.fulfill({ status: 404, json: { code: "unexpected-fixture-route", message: pathname } });
	});
	return { view, output, actions, edits, publish, holdReads: () => { let release!: () => void; readGate = new Promise((resolve) => { release = resolve; }); return () => { readGate = undefined; release(); }; } };
}

async function pick(page: Page, label: string, option: string) {
	await page.getByRole("button", { name: label, exact: true }).click();
	await page.getByRole("option", { name: option, exact: true }).click();
}

test("@ui › Pixel playlist edits survive reload and preserve ordered assets and fixed layers", async ({ page, mediaUrl }, testInfo) => {
	const stub = await stubPixel(page);
	await page.goto(`${mediaUrl}/playlists`, { waitUntil: "domcontentloaded" });
	await expect(page.getByRole("heading", { name: "Lobby information" })).toBeVisible();
	for (const width of [1440, 768, 390]) {
		await page.setViewportSize({ width, height: 1000 });
		await assertHeaderButtonFits(page, "New playlist");
	}
	await page.setViewportSize({ width: 1440, height: 1000 });
	const releaseReload = stub.holdReads();
	const newButton = await page.getByRole("button", { name: "New playlist", exact: true }).boundingBox();
	if (!newButton) throw new Error("New playlist has no clickable bounds");
	await page.mouse.click(newButton.x + newButton.width / 2, newButton.y + newButton.height - 3);
	await expect(page.getByRole("article", { name: "Edit New playlist", exact: true })).toBeVisible();
	expect(stub.view.configuration.playlists.find((playlist) => playlist.id === CREATED_PLAYLIST)).toMatchObject({ enabled: false, entries: [] });
	await expect(page.getByRole("switch", { name: "Playlist enabled", exact: true })).not.toBeChecked();
	releaseReload();
	await page.getByLabel("Playlist name", { exact: true }).fill("Created draft");
	await expect.poll(() => stub.view.configuration.playlists.find((playlist) => playlist.id === CREATED_PLAYLIST)?.name).toBe("Created draft");
	await page.getByRole("navigation", { name: "Playlists", exact: true }).getByRole("button", { name: /^Lobby information/ }).click();
	await page.getByRole("button", { name: "Move clip 1 down" }).click();
	await page.getByLabel("Clip 1 plays", { exact: true }).fill("3");
	await page.getByLabel("Clip 1 fade to next (s)", { exact: true }).fill("2");
	await page.getByLabel("Clip 1 scale X", { exact: true }).fill("1.35");
	await page.getByLabel("Clip 1 scale Y", { exact: true }).fill("0.7");
	await page.getByLabel("Clip 1 position X", { exact: true }).fill("-0.4");
	await page.getByLabel("Clip 1 position Y", { exact: true }).fill("0.25");
	await page.getByLabel("Clip 1 rotation (°)", { exact: true }).fill("30");
	await pick(page, "Playback layer A", "Layer 4");
	await pick(page, "Layer 1 behavior", "Fixed content");
	await pick(page, "Layer 1 content", "001 · Opening hours");
	await expect.poll(() => stub.view.configuration.playlists[0]).toMatchObject({ layers: [4, 3], entries: [
		{ id: "video-entry", assetId: VIDEO, plays: 3, fadeMillis: 2000, transform: { scaleX: 1.35, scaleY: 0.7, positionX: -0.4, positionY: 0.25, rotation: 30 } }, { id: "image-entry", assetId: IMAGE, transform: DEFAULT_PLAYLIST_TRANSFORM },
	], auxiliaryLayers: [{ layer: 1, mode: "fixed", assetId: IMAGE }] });
	await page.reload({ waitUntil: "domcontentloaded" });
	await expect(page.getByLabel("Clip 1 plays", { exact: true })).toHaveValue("3");
	await expect(page.getByLabel("Clip 1 fade to next (s)", { exact: true })).toHaveValue("2");
	await expect(page.getByLabel("Clip 1 scale X", { exact: true })).toHaveValue("1.35");
	await expect(page.getByLabel("Clip 1 scale Y", { exact: true })).toHaveValue("0.7");
	await expect(page.getByLabel("Clip 1 position X", { exact: true })).toHaveValue("-0.4");
	await expect(page.getByLabel("Clip 1 position Y", { exact: true })).toHaveValue("0.25");
	await expect(page.getByLabel("Clip 1 rotation (°)", { exact: true })).toHaveValue("30");
	await expect(page.getByRole("button", { name: "Playback layer A", exact: true })).toHaveText("Layer 4");
	await expect(page.getByRole("button", { name: "Layer 1 content", exact: true })).toContainText("Opening hours");
	await page.getByRole("button", { name: "Preview clip 1", exact: true }).click();
	await expect(page.getByText("Previewing Lobby information · Step 1", { exact: true })).toBeVisible();
	await expect(page.getByRole("img", { name: "Live playlist step preview on Main display" })).toBeVisible();
	await page.getByLabel("Clip 1 rotation (°)", { exact: true }).fill("45");
	await expect.poll(() => stub.actions.filter((action) => action.type === "automation-preview-step").at(-1)).toMatchObject({ request: { entryId: "video-entry", playlist: { entries: [{ transform: { rotation: 45 } }, {}] } } });
	await page.getByRole("button", { name: "Preview clip 2", exact: true }).click();
	await expect(page.getByText("Previewing Lobby information · Step 2", { exact: true })).toBeVisible();
	await expect.poll(() => stub.actions.filter((action) => action.type === "automation-preview-step").at(-1)).toMatchObject({ request: { entryId: "image-entry" } });
	await page.getByRole("button", { name: "Stop preview", exact: true }).click();
	await expect(page.getByRole("button", { name: "Stop preview", exact: true })).toBeDisabled();
	expect(stub.output.playbackTakeover).toBe(false);
	for (const width of [1440, 768, 390]) {
		await page.setViewportSize({ width, height: 1000 });
		await page.locator(".pixel-automation-page").evaluate((element) => { element.scrollTop = 0; });
		await expect(page.locator("body")).not.toHaveCSS("overflow-x", "scroll");
		expect(await page.locator(".pixel-automation-page").evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
		await page.screenshot({ path: testInfo.outputPath(`pixel-playlists-${width}.png`) });
	}
	expect(stub.edits.every((edit) => typeof edit.body.requestId === "string")).toBe(true);
});

test("@ui › Pixel weekly scheduling saves overnight windows and receives DMX ownership over live telemetry", async ({ page, mediaUrl }, testInfo) => {
	const stub = await stubPixel(page);
	await page.goto(`${mediaUrl}/schedule`, { waitUntil: "domcontentloaded" });
	for (const width of [1440, 768, 390]) { await page.setViewportSize({ width, height: 1000 }); await assertHeaderButtonFits(page, "New event"); }
	await page.setViewportSize({ width: 1440, height: 1000 });
	await pick(page, "Fallback playlist", "Lobby information");
	await page.getByLabel("DMX silence timeout (s)", { exact: true }).fill("20");
	await page.getByRole("button", { name: "New event", exact: true }).click();
	await pick(page, "Event type", "Weekly");
	await page.getByLabel("Start time", { exact: true }).fill("22:00:00");
	await page.getByLabel("Loop until an end time", { exact: true }).check({ force: true });
	await page.getByLabel("End time", { exact: true }).fill("02:00:00");
	await page.getByLabel("End day offset", { exact: true }).fill("1");
	await page.getByRole("button", { name: "Create event", exact: true }).click();
	await expect.poll(() => stub.view.configuration.events).toHaveLength(1);
	expect(stub.view.configuration.events[0].trigger).toMatchObject({ kind: "weekly", weekdays: [0], startTime: "22:00:00", endTime: "02:00:00", endDayOffset: 1 });
	await expect.poll(() => stub.view.configuration.outputs[0]).toMatchObject({ fallbackPlaylistId: PLAYLIST, dmxSilenceSeconds: 20 });
	await page.getByRole("switch", { name: "Scheduling enabled", exact: true }).locator("..").click();
	await expect(page.getByRole("switch", { name: "Scheduling enabled", exact: true })).toBeChecked();
	await expect.poll(() => stub.actions.filter((action) => action.type === "automation-set-enabled")).toHaveLength(1);
	await expect(page.getByRole("heading", { name: "Fallback playlist", exact: true })).toBeVisible();
	stub.view.status[0] = { ...stub.view.status[0], owner: "dmx", dmxAgeMillis: 2500, resumeInMillis: 17500 };
	stub.publish();
	await expect(page.getByRole("heading", { name: "DMX controlling", exact: true })).toBeVisible();
	await expect(page.getByText(/Scheduling resumes in 17.5 s/)).toBeVisible();
	for (const width of [1440, 768, 390]) {
		await page.setViewportSize({ width, height: 1000 });
		await page.locator(".pixel-automation-page").evaluate((element) => { element.scrollTop = 0; });
		expect(await page.locator(".pixel-automation-page").evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
		await page.screenshot({ path: testInfo.outputPath(`pixel-schedule-${width}.png`) });
	}
	await page.reload({ waitUntil: "domcontentloaded" });
	await expect(page.getByRole("button", { name: "Fallback playlist", exact: true })).toHaveText("Lobby information");
	await expect(page.getByLabel("DMX silence timeout (s)", { exact: true })).toHaveValue("20");
	expect(stub.actions.filter((action) => action.type === "automation-set-enabled")).toHaveLength(1);
});

async function assertHeaderButtonFits(page: Page, label: string) {
	const button = page.getByRole("button", { name: label, exact: true });
	await expect(button).toBeVisible();
	const geometry = await button.evaluate((element) => {
		const bounds = element.getBoundingClientRect();
		const header = element.closest(".ui-window-header")!.getBoundingClientRect();
		const hit = document.elementFromPoint(bounds.x + bounds.width / 2, bounds.bottom - 2);
		return { top: bounds.top, bottom: bounds.bottom, headerTop: header.top, headerBottom: header.bottom, bottomHit: element.contains(hit) };
	});
	expect(geometry.top).toBeGreaterThanOrEqual(geometry.headerTop);
	expect(geometry.bottom).toBeLessThanOrEqual(geometry.headerBottom);
	expect(geometry.bottomHit).toBe(true);
}
