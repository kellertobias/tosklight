import { deflateSync } from "node:zlib";
import type { Locator, Page, Route } from "@playwright/test";
import type {
	CueMediaPreviewIndex,
	DiscoveredMediaOutput,
	MediaServerDiscovery,
} from "../apps/light-desktop/src/api/generated/light-wire";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import { BrowserDesktops } from "./bench/window-system/desktopScenario";
import { PaneType } from "./bench/window-system/paneTypes";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	objects,
	putObject,
} from "./support/catalog";

test.use({ viewport: { width: 1600, height: 1100 } });

const DISCOVER = "/api/v2/media-servers/discover";
const MEDIA_PREVIEW = /\/api\/v2\/cues\/([^/]+)\/media-preview/;
const RACK_A_OUTPUT = "00000000-0000-4000-8000-000000000061";
const RACK_B_OUTPUT = "00000000-0000-4000-8000-000000000062";

type Rgba = [number, number, number, number];

/** A solid PNG; `inset` leaves a transparent border like a scaled-down layer. */
function png(width: number, height: number, colour: Rgba, inset = 0): Buffer {
	const crcTable = Array.from({ length: 256 }, (_, n) => {
		let c = n;
		for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
		return c >>> 0;
	});
	const crc = (bytes: Buffer) => {
		let c = 0xffffffff;
		for (const byte of bytes) c = crcTable[(c ^ byte) & 0xff] ^ (c >>> 8);
		return (c ^ 0xffffffff) >>> 0;
	};
	const chunk = (type: string, data: Buffer) => {
		const length = Buffer.alloc(4);
		length.writeUInt32BE(data.length);
		const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
		const sum = Buffer.alloc(4);
		sum.writeUInt32BE(crc(body));
		return Buffer.concat([length, body, sum]);
	};
	const header = Buffer.alloc(13);
	header.writeUInt32BE(width, 0);
	header.writeUInt32BE(height, 4);
	header[8] = 8;
	header[9] = 6;
	const rows = Buffer.alloc((width * 4 + 1) * height);
	for (let y = 0; y < height; y++) {
		const row = y * (width * 4 + 1);
		for (let x = 0; x < width; x++) {
			const inside =
				x >= inset && y >= inset && x < width - inset && y < height - inset;
			const pixel = inside ? colour : ([0, 0, 0, 0] as Rgba);
			Buffer.from(pixel).copy(rows, row + 1 + x * 4);
		}
	}
	return Buffer.concat([
		Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
		chunk("IHDR", header),
		chunk("IDAT", deflateSync(rows)),
		chunk("IEND", Buffer.alloc(0)),
	]);
}

function output(overrides: Partial<DiscoveredMediaOutput>): DiscoveredMediaOutput {
	return {
		id: RACK_A_OUTPUT,
		name: "Main",
		personality: "two-layers",
		protocol: "sacn",
		universe: 101,
		startAddress: 355,
		dmxPendingRestart: false,
		mode: "2 layers",
		tempoSource: "playback-bpm-channel",
		speedGroup: null,
		issue: null,
		...overrides,
	};
}

const DISCOVERY: MediaServerDiscovery = {
	discoveryError: null,
	servers: [
		{
			key: "192.0.2.61:4809",
			name: "ToskLight Pixel Media - Rack A",
			host: "192.0.2.61",
			citpPort: 4809,
			status: "ready",
			instance: "Rack A",
			error: null,
			outputs: [output({})],
		},
		{
			key: "192.0.2.62:4809",
			name: "ToskLight Pixel Media - Rack B",
			host: "192.0.2.62",
			citpPort: 4809,
			status: "ready",
			instance: "Rack B",
			error: null,
			outputs: [
				output({
					id: RACK_B_OUTPUT,
					name: "Side",
					protocol: "art-net",
					universe: 1,
					startAddress: 180,
				}),
			],
		},
	],
};

interface MediaServer {
	fixture_id: string;
	endpoint: { ip_address: string } | null;
	layers: Array<{ fixture_id: string; head_index: number }>;
}

interface Servers {
	a: MediaServer;
	b: MediaServer;
}

/** Patches both discovered servers through Show Patch › Media Servers, as an operator does. */
async function patchServers(page: Page, api: ApiDriver): Promise<Servers> {
	await page.route(`**${DISCOVER}`, (route) =>
		route.fulfill({ json: DISCOVERY }),
	);
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Show Patch", exact: true }).click();
	await page.getByRole("tab", { name: "Media Servers", exact: true }).click();
	for (const title of ["Rack A · Main", "Rack B · Side"]) {
		const card = page.locator(".media-server-card").filter({ hasText: title });
		await card.getByRole("button", { name: "Patch suggested" }).click();
		await expect(card.getByText("Patched", { exact: true })).toBeVisible();
	}
	// The patch reaches the output engine just after the card reports it.
	let fixtures: MediaServer[] = [];
	await expect
		.poll(async () => {
			({ fixtures } = await api.request<{ fixtures: MediaServer[] }>(
				"GET",
				"/api/v2/media-servers",
			));
			return fixtures.map((fixture) => fixture.endpoint?.ip_address).sort();
		})
		.toEqual(["192.0.2.61", "192.0.2.62"]);
	const byHost = (host: string) => {
		const found = fixtures.find((fixture) => fixture.endpoint?.ip_address === host);
		if (!found) throw new Error(`Media Server ${host} was not patched`);
		found.layers.sort((left, right) => left.head_index - right.head_index);
		return found;
	};
	return { a: byHost("192.0.2.61"), b: byHost("192.0.2.62") };
}

function change(fixtureId: string, attribute: string, raw: number) {
	return {
		fixture_id: fixtureId,
		attribute,
		value: { kind: "normalized", value: raw / 255 },
		automatic_restore: false,
	};
}

function cue(id: string, number: number, name: string, changes: unknown[]) {
	return {
		id,
		number,
		name,
		fade_millis: 0,
		delay_millis: 0,
		trigger: { type: "manual" },
		changes,
		group_changes: [],
	};
}

async function putCueList(
	api: ApiDriver,
	id: string,
	name: string,
	cues: unknown[],
	revision = 0,
) {
	await putObject(
		api,
		"cue_list",
		id,
		{
			id,
			name,
			priority: 0,
			mode: "sequence",
			looped: false,
			chaser_step_millis: 1_000,
			speed_group: null,
			intensity_priority_mode: "htp",
			wrap_mode: "off",
			restart_mode: "first_cue",
			force_cue_timing: false,
			disable_cue_timing: false,
			chaser_xfade_percent: 0,
			speed_multiplier: 1,
			cues,
		},
		revision,
	);
}

function playback(
	number: number,
	name: string,
	cueListId: string,
	extra: Record<string, unknown> = {},
) {
	return {
		number,
		name,
		target: { type: "cue_list", cue_list_id: cueListId },
		buttons: ["go", "none", "none"],
		fader: "master",
		go_activates: true,
		auto_off: false,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
		...extra,
	};
}

async function revisionOf(api: ApiDriver, kind: string, id: string) {
	const found = (await objects(api, kind)).find((entry) => entry.id === id);
	if (!found) throw new Error(`${kind} ${id} is missing`);
	return found;
}

/**
 * Serves each Cue's Media Server picture from what the test scripts, and records every request.
 * A different preview key is a different picture, so a changed Cue visibly changes its image.
 */
class FakePictures {
	readonly requests: Array<{ cueId: string; key: string }> = [];
	private readonly behaviours = new Map<
		string,
		(key: string) => Parameters<Route["fulfill"]>[0]
	>();

	picture(cueId: string, colours: Record<string, Rgba> | Rgba, inset = 0) {
		this.behaviours.set(cueId, (key) => {
			const colour = Array.isArray(colours)
				? colours
				: (colours[key] ?? colours["*"]);
			return {
				status: 200,
				contentType: "image/png",
				headers: { "x-light-media-preview": "content" },
				body: png(32, 18, colour, inset),
			};
		});
	}

	empty(cueId: string) {
		this.behaviours.set(cueId, () => ({
			status: 200,
			contentType: "image/png",
			headers: { "x-light-media-preview": "empty" },
			body: png(32, 18, [0, 0, 0, 255]),
		}));
	}

	failure(cueId: string, state: "offline" | "missing" | "loading") {
		this.behaviours.set(cueId, () => ({
			status: state === "missing" ? 404 : 503,
			json: {
				state,
				error: `${state} in test`,
				retryable: state !== "missing",
			},
		}));
	}

	async install(page: Page) {
		await page.route(MEDIA_PREVIEW, (route) => {
			const url = new URL(route.request().url());
			const cueId = decodeURIComponent(url.pathname.match(MEDIA_PREVIEW)?.[1] ?? "");
			const key = url.searchParams.get("key") ?? "";
			this.requests.push({ cueId, key });
			const behaviour = this.behaviours.get(cueId);
			if (!behaviour)
				return route.fulfill({
					status: 404,
					json: { state: "missing", error: "unscripted", retryable: false },
				});
			return route.fulfill(behaviour(key));
		});
	}
}

/** The first pixel and a centre pixel of a displayed picture. */
async function pixels(image: Locator) {
	await expect(image).toHaveJSProperty("complete", true);
	return image.evaluate((element: HTMLImageElement) => {
		const canvas = document.createElement("canvas");
		canvas.width = element.naturalWidth;
		canvas.height = element.naturalHeight;
		const context = canvas.getContext("2d");
		if (!context) throw new Error("no 2D canvas");
		context.drawImage(element, 0, 0);
		const at = (x: number, y: number) =>
			Array.from(context.getImageData(x, y, 1, 1).data);
		return {
			corner: at(0, 0),
			centre: at(Math.floor(canvas.width / 2), Math.floor(canvas.height / 2)),
		};
	});
}

async function mediaIndex(api: ApiDriver, showId: string) {
	return api.request<CueMediaPreviewIndex>(
		"GET",
		"/api/v2/cues/media-previews",
		undefined,
		true,
		undefined,
		{ showId },
	);
}

const RED: Rgba = [220, 30, 30, 255];
const GREEN: Rgba = [30, 200, 60, 255];
const BLUE: Rgba = [30, 60, 220, 255];
const YELLOW: Rgba = [230, 210, 20, 255];

test.describe("docs/testing/25-media-cue-previews.md", () => {
	test("MEDIACUE-001 @ui › media-only Cues show their own server's Program or layer picture and explicit fallbacks", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const show = await loadCanonicalCopy(api, bench, "mediacue-001", "default-stage");
		const pictures = new FakePictures();
		await pictures.install(page);
		await desk.open(api.baseUrl);
		const servers = await patchServers(page, api);
		const dimmer = (await fixtureIdsByNumber(api))[1];

		const listId = crypto.randomUUID();
		const ids = {
			program: crypto.randomUUID(),
			layer: crypto.randomUUID(),
			otherServer: crypto.randomUUID(),
			lighting: crypto.randomUUID(),
			empty: crypto.randomUUID(),
			offline: crypto.randomUUID(),
			missing: crypto.randomUUID(),
		};
		const [a1, a2] = servers.a.layers.map((layer) => layer.fixture_id);
		const [b1] = servers.b.layers.map((layer) => layer.fixture_id);
		await putCueList(api, listId, "Media looks", [
			cue(ids.program, 1, "Program A", [
				change(a1, "media.folder", 1),
				change(a1, "media.file", 1),
				change(a1, "intensity", 255),
				change(servers.a.fixture_id, "intensity", 255),
			]),
			cue(ids.layer, 2, "Layer A2", [
				change(a2, "media.folder", 2),
				change(a2, "media.file", 3),
				change(a2, "intensity", 255),
			]),
			cue(ids.otherServer, 3, "Layer B1", [change(b1, "media.folder", 4)]),
			cue(ids.lighting, 4, "Lighting", [change(dimmer, "intensity", 255)]),
			cue(ids.empty, 5, "Blank A1", [change(a1, "media.file", 0)]),
			cue(ids.offline, 6, "Master B", [
				change(servers.b.fixture_id, "intensity", 128),
			]),
			cue(ids.missing, 7, "Layer B2", [
				change(servers.b.layers[1].fixture_id, "media.file", 9),
			]),
		]);
		await putObject(api, "playback", "201", playback(201, "Media looks", listId));

		// The desk itself decides which Cues are media Cues and which picture each one is.
		const index = await mediaIndex(api, show.id);
		const entry = (cueId: string) =>
			index.entries.find((candidate) => candidate.cue_id === cueId);
		expect(entry(ids.program)).toMatchObject({
			scope: "program",
			server_fixture_id: servers.a.fixture_id,
			output_id: RACK_A_OUTPUT,
		});
		expect(entry(ids.layer)).toMatchObject({
			scope: "layer",
			layer: 1,
			layer_fixture_id: a2,
			server_fixture_id: servers.a.fixture_id,
		});
		expect(entry(ids.otherServer)).toMatchObject({
			scope: "layer",
			layer: 0,
			server_fixture_id: servers.b.fixture_id,
			output_id: RACK_B_OUTPUT,
		});
		expect(entry(ids.offline)).toMatchObject({
			scope: "program",
			server_fixture_id: servers.b.fixture_id,
		});
		expect(entry(ids.lighting)).toBeUndefined();

		pictures.picture(ids.program, { "*": RED });
		pictures.picture(ids.layer, { "*": GREEN }, 4);
		pictures.picture(ids.otherServer, { "*": BLUE }, 4);
		pictures.empty(ids.empty);
		pictures.failure(ids.offline, "offline");
		pictures.failure(ids.missing, "missing");

		const desktops = new BrowserDesktops(page, async () => undefined);
		const layout = desktops.configure("Media Cue previews");
		const cuesPane = layout.addPane(
			PaneType.Cues,
			{ slug: "media-cues", column: 1, row: 1, width: 12, height: 10 },
			{ cueListSource: "fixed", fixedCueListNumber: 201 },
		);
		await layout.apply();
		const rows = cuesPane.root().locator(".cue-table tbody tr");
		const preview = (row: number) =>
			rows.nth(row).locator(".cue-preview-column [data-preview-kind='media']");

		// Cue 1: the whole output's Program image of Rack A, opaque.
		await expect(preview(0)).toHaveAttribute("data-media-state", "ready");
		await expect(preview(0)).toHaveAttribute("data-media-scope", "program");
		await expect(preview(0)).toHaveAttribute("data-media-server", servers.a.fixture_id);
		expect((await pixels(preview(0).locator("img"))).centre).toEqual(RED);

		// Cue 2: only layer 2 of Rack A, transparency kept, over the checkerboard.
		await expect(preview(1)).toHaveAttribute("data-media-scope", "layer");
		await expect(preview(1)).toHaveAttribute("data-media-layer", "1");
		await expect(preview(1)).toHaveAccessibleName("Open Cue 2 Layer 2 preview");
		const layerPixels = await pixels(preview(1).locator("img"));
		expect(layerPixels.centre).toEqual(GREEN);
		expect(layerPixels.corner[3]).toBe(0);
		await expect(preview(1).locator("img")).not.toHaveCSS("background-image", "none");

		// Cue 3: Rack B's own picture, never Rack A's.
		await expect(preview(2)).toHaveAttribute("data-media-server", servers.b.fixture_id);
		expect((await pixels(preview(2).locator("img"))).centre).toEqual(BLUE);

		// Cue 4 keeps its Stage preview path: no media picture is asked for or shown.
		await expect(
			rows.nth(3).locator("[data-preview-kind='media']"),
		).toHaveCount(0);
		expect(pictures.requests.some((request) => request.cueId === ids.lighting)).toBe(
			false,
		);

		// Empty, offline, and missing are named; none shows a broken or another Cue's picture.
		await expect(preview(4)).toHaveAttribute("data-media-state", "empty");
		await expect(preview(4)).toContainText("Empty media");
		await expect(preview(5)).toHaveAttribute("data-media-state", "offline");
		await expect(preview(5)).toContainText("Media Server offline");
		await expect(preview(5).locator("img")).toHaveCount(0);
		await expect(preview(6)).toHaveAttribute("data-media-state", "missing");
		await expect(preview(6)).toContainText("Media output missing");
		await expect(preview(6).locator("img")).toHaveCount(0);

		// Touching the offline placeholder asks again; the server now answers.
		pictures.picture(ids.offline, { "*": YELLOW });
		await preview(5).click();
		await expect(preview(5)).toHaveAttribute("data-media-state", "ready");
		expect((await pixels(preview(5).locator("img"))).centre).toEqual(YELLOW);

		// The picture opens larger on touch.
		await preview(1).click();
		const modal = page.getByRole("dialog", { name: "Cue 2 preview image" });
		await expect(modal.locator(".cuelist-preview-modal-body")).toHaveClass(/scope-layer/);
		await modal.getByRole("button", { name: "Close Cue preview" }).click();

		// Editing Cue 2 changes its preview key; the new picture replaces the old one.
		const before = entry(ids.layer)?.preview_key;
		pictures.picture(ids.layer, { "*": BLUE }, 4);
		const stored = await revisionOf(api, "cue_list", listId);
		const cues = stored.body.cues as Array<{ id: string; changes: unknown[] }>;
		cues[1].changes = [
			change(a2, "media.folder", 2),
			change(a2, "media.file", 4),
			change(a2, "intensity", 255),
		];
		await putCueList(api, listId, "Media looks", cues, stored.revision);
		await expect
			.poll(async () =>
				pictures.requests
					.filter((request) => request.cueId === ids.layer)
					.map((request) => request.key),
			)
			.toContainEqual(expect.not.stringMatching(`^${before}$`));
		await expect
			.poll(async () => (await pixels(preview(1).locator("img"))).centre)
			.toEqual(BLUE);
		await expect(preview(1)).not.toHaveAttribute(
			"data-media-preview-key",
			before ?? "",
		);
	});

	test("MEDIACUE-002 @ui › single-Cue Virtual Playbacks show their Cue preview by default and keep an operator image", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const show = await loadCanonicalCopy(api, bench, "mediacue-002", "default-stage");
		const pictures = new FakePictures();
		await pictures.install(page);
		await desk.open(api.baseUrl);
		const servers = await patchServers(page, api);
		const [a1] = servers.a.layers.map((layer) => layer.fixture_id);
		const [b1] = servers.b.layers.map((layer) => layer.fixture_id);

		const lists = {
			program: crypto.randomUUID(),
			layer: crypto.randomUUID(),
			configured: crypto.randomUUID(),
			offline: crypto.randomUUID(),
		};
		const cues = {
			program: crypto.randomUUID(),
			layer: crypto.randomUUID(),
			configured: crypto.randomUUID(),
			offline: crypto.randomUUID(),
			second: crypto.randomUUID(),
		};
		const programCue = cue(cues.program, 1, "Program A", [
			change(servers.a.fixture_id, "intensity", 255),
		]);
		await putCueList(api, lists.program, "Program A", [programCue]);
		await putCueList(api, lists.layer, "Layer B1", [
			cue(cues.layer, 1, "Layer B1", [change(b1, "media.folder", 5)]),
		]);
		await putCueList(api, lists.configured, "Configured", [
			cue(cues.configured, 1, "Layer A1", [change(a1, "media.folder", 6)]),
		]);
		await putCueList(api, lists.offline, "Offline", [
			cue(cues.offline, 1, "Master B", [
				change(servers.b.fixture_id, "intensity", 255),
			]),
		]);
		pictures.picture(cues.program, RED);
		pictures.picture(cues.layer, GREEN, 4);
		pictures.picture(cues.configured, BLUE);
		pictures.failure(cues.offline, "offline");

		const configuredImage = `data:image/png;base64,${png(8, 8, YELLOW).toString("base64")}`;
		const singleCue = (number: number, name: string, list: string, extra = {}) => ({
			...playback(number, name, list, extra),
			button_count: 1,
			has_fader: false,
		});
		const existingPage = (await objects(api, "playback_page")).find(
			(entry) => entry.id === "1",
		);
		await putObject(
			api,
			"playback_page",
			"1",
			{
				...(existingPage?.body ?? { number: 1, name: "Page 1", slots: {} }),
				virtual_playbacks: {
					...(existingPage?.body.virtual_playbacks ?? {}),
					"1001": singleCue(1001, "Program A", lists.program),
					"1002": singleCue(1002, "Layer B1", lists.layer),
					"1003": singleCue(1003, "Configured", lists.configured, {
						presentation_image: configuredImage,
					}),
					"1004": singleCue(1004, "Offline", lists.offline),
				},
			},
			existingPage?.revision ?? 0,
		);

		const desktops = new BrowserDesktops(page, async () => undefined);
		const layout = desktops.configure("Media Virtual Playbacks");
		const pane = layout.addPane(
			PaneType.VirtualPlaybacks,
			{ slug: "media-virtual", column: 1, row: 1, width: 12, height: 6 },
			{ rows: 1, columns: 4, pageMode: "pinned", pinnedPage: 1 },
		);
		await layout.apply();
		const tile = (number: number) =>
			pane.root().locator(`[data-virtual-playback-number="${number}"]`);

		// A grid of media-only single-Cue playbacks shows each Cue's own picture.
		await expect(tile(1001)).toHaveAttribute("data-image-source", "cue-preview");
		expect((await pixels(tile(1001).locator(".pool-card-image"))).centre).toEqual(RED);
		await expect(tile(1002)).toHaveAttribute("data-image-source", "cue-preview");
		await expect(tile(1002)).toHaveClass(/cue-preview-transparent/);
		const layer = await pixels(tile(1002).locator(".pool-card-image"));
		expect(layer.centre).toEqual(GREEN);
		expect(layer.corner[3]).toBe(0);

		// The operator's image wins; the Cue preview is never even shown there.
		await expect(tile(1003)).toHaveAttribute("data-image-source", "configured");
		expect((await pixels(tile(1003).locator(".pool-card-image"))).centre).toEqual(
			YELLOW,
		);

		// Offline is a named state, not an image.
		await expect(tile(1004)).toHaveAttribute(
			"data-preview-notice",
			"Media Server offline",
		);
		await expect(tile(1004).locator(".pool-card-image")).toHaveCount(0);

		// Adding a second Cue removes the automatic default; removing it brings it back.
		const stored = await revisionOf(api, "cue_list", lists.program);
		await putCueList(
			api,
			lists.program,
			"Program A",
			[
				programCue,
				cue(cues.second, 2, "Second", [
					change(servers.a.fixture_id, "intensity", 10),
				]),
			],
			stored.revision,
		);
		await expect(tile(1001).locator(".pool-card-image")).toHaveCount(0);
		await expect(tile(1001)).not.toHaveAttribute("data-image-source", /.*/);
		const twoCues = await revisionOf(api, "cue_list", lists.program);
		await putCueList(api, lists.program, "Program A", [programCue], twoCues.revision);
		await expect(tile(1001)).toHaveAttribute("data-image-source", "cue-preview");

		// Choosing an icon replaces the automatic image; the icon is the operator's choice.
		const current = await revisionOf(api, "playback_page", "1");
		const playbacks = current.body.virtual_playbacks as Record<
			string,
			Record<string, unknown>
		>;
		await putObject(
			api,
			"playback_page",
			"1",
			{
				...current.body,
				virtual_playbacks: {
					...playbacks,
					"1002": { ...playbacks["1002"], presentation_icon: "★" },
				},
			},
			current.revision,
		);
		await expect(tile(1002).locator(".pool-card-image")).toHaveCount(0);
		await expect(tile(1002).locator(".pool-card-icon")).toHaveText("★");
		await expect(tile(1002)).not.toHaveClass(/cue-preview-transparent/);

		// Editing the only Cue changes the automatic picture.
		const index = await mediaIndex(api, show.id);
		const before = index.entries.find((entry) => entry.cue_id === cues.program)
			?.preview_key;
		pictures.picture(cues.program, BLUE);
		const single = await revisionOf(api, "cue_list", lists.program);
		await putCueList(
			api,
			lists.program,
			"Program A",
			[
				cue(cues.program, 1, "Program A", [
					change(servers.a.fixture_id, "intensity", 200),
				]),
			],
			single.revision,
		);
		await expect
			.poll(async () => (await pixels(tile(1001).locator(".pool-card-image"))).centre)
			.toEqual(BLUE);
		const after = (await mediaIndex(api, show.id)).entries.find(
			(entry) => entry.cue_id === cues.program,
		)?.preview_key;
		expect(after).not.toBe(before);
	});
});
