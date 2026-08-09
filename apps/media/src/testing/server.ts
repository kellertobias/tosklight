// A stand-in for the Media Server.
//
// Tests drive the real client and the real resource cache; only the socket is replaced. That way
// a test proving a rollback is proving the code an operator runs, not a mock of it.

import { vi } from "vitest";
import type {
	AudioPanelView,
	CatalogView,
	DmxMapView,
	Health,
	ImportsView,
	LogsView,
	NetworkView,
	OutputView,
	ServerLogLevelView,
	TextSlotView,
	VisualizerView,
} from "../shared/api/generated/media-wire";
import { resetResources } from "../shared/api/resource";

export interface StubbedServer {
	outputs: OutputView[];
	catalog: CatalogView;
	health: Health;
	visualizers: VisualizerView[];
	network: NetworkView;
	text: TextSlotView[];
	audio: AudioPanelView;
	logs: LogsView;
	serverLogLevel: ServerLogLevelView;
	imports: ImportsView;
	/** Set to make the next write fail with this code and status. */
	refuseWrites: { code: string; message: string; status: number } | undefined;
	/** Every path written to, in order. */
	writes: string[];
}

export function stubServer(
	overrides: Partial<StubbedServer> = {},
): StubbedServer {
	const server: StubbedServer = {
		outputs: [anOutput()],
		catalog: aCatalog(),
		visualizers: [aVisualizer()],
		network: aNetwork(),
		text: [aClock(), aCountdown()],
		audio: anAudioPanel(),
		logs: aLog(),
		serverLogLevel: { level: "info", resetsOnRestart: true },
		imports: anImportState(),
		health: {
			status: "ok",
			instance: "test-instance",
			outputs: 1,
			catalogRevision: 3,
			catalogItems: 2,
		},
		refuseWrites: undefined,
		writes: [],
		...overrides,
	};

	resetResources();
	vi.stubGlobal(
		"fetch",
		vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
			const path = String(input).replace("/api/v2", "");
			if (init?.method === "POST" || path.endsWith("/reset")) {
				server.writes.push(path);
				if (server.refuseWrites) {
					const { code, message, status } = server.refuseWrites;
					return jsonResponse({ code, message }, status);
				}
			}
			if (path === "/health") return jsonResponse(server.health);
			if (path === "/catalog") return jsonResponse(server.catalog);
			if (path === "/visualizers") return jsonResponse(server.visualizers);
			if (path === "/fixtures")
				return jsonResponse([
					"ToskLight Media Layer.gdtf",
					"ToskLight Media Master.gdtf",
				]);
			if (path === "/outputs") return jsonResponse(server.outputs);
			const dmxMap = path.match(/^\/outputs\/([^/]+)\/dmx-map$/u);
			if (dmxMap) {
				const output = server.outputs.find(
					(candidate) => candidate.id === dmxMap[1],
				);
				return output
					? jsonResponse(aDmxMap(output.id, output.name))
					: jsonResponse({ code: "unknown-output", message: "no" }, 404);
			}
			if (path === "/network") return jsonResponse(server.network);
			if (path === "/text") return jsonResponse(server.text);
			if (path === "/audio") return jsonResponse(server.audio);
			if (path === "/logs/level") return jsonResponse(server.serverLogLevel);
			if (path === "/logs/level/update") {
				const body = JSON.parse(String(init?.body ?? "{}"));
				server.serverLogLevel.level = body.level;
				return jsonResponse(server.serverLogLevel);
			}
			if (path.startsWith("/logs")) return jsonResponse(server.logs);
			if (path === "/library/imports") return jsonResponse(server.imports);
			const imported = writeImport(server, path);
			if (imported) return imported;
			const library = writeLibrary(server, path, init);
			if (library) return library;
			if (path === "/network/update") {
				const body = JSON.parse(String(init?.body ?? "{}"));
				for (const field of [
					"artNetListen",
					"sacnListen",
					"citpListen",
					"httpListen",
				] as const) {
					if (body[field] !== undefined)
						server.network.stored[field] = body[field];
				}
				if (body.sameComputerPreset !== undefined) {
					server.network.sameComputerPreset = body.sameComputerPreset;
				}
				if (body.speedGroupEndpoint !== undefined) {
					server.network.stored.speedGroupEndpoint = body.speedGroupEndpoint;
				}
				server.network.resolved = { ...server.network.stored };
				return jsonResponse(server.network);
			}

			if (path === "/audio/update") {
				const body = JSON.parse(String(init?.body ?? "{}"));
				server.audio.settings = { ...server.audio.settings, ...body };
				return jsonResponse(server.audio.settings);
			}

			const text = writeText(server, path, init);
			if (text) return text;

			if (path.endsWith("/reset")) return new Response(null, { status: 204 });

			const tuned = path.match(/^\/visualizers\/(\d+)\/(\d+)\/update$/u);
			if (tuned) {
				const body = JSON.parse(String(init?.body ?? "{}"));
				const found = server.visualizers.find(
					(candidate) =>
						candidate.address.folder === Number(tuned[1]) &&
						candidate.address.file === Number(tuned[2]),
				);
				if (!found) {
					return jsonResponse(
						{ code: "unknown-visualizer", message: "no" },
						404,
					);
				}
				if (body.name !== undefined) found.name = body.name;
				if (body.parameters !== undefined) found.parameters = body.parameters;
				return jsonResponse(found);
			}

			const update = path.match(/^\/outputs\/([^/]+)\/layers\/(\d+)\/update$/u);
			if (update) {
				const body = JSON.parse(String(init?.body ?? "{}"));
				const output = server.outputs.find(
					(candidate) => candidate.id === update[1],
				);
				if (!output)
					return jsonResponse({ code: "unknown-output", message: "no" }, 404);
				const layer = output.layers[Number(update[2])];
				if (body.dimmer !== undefined) layer.dimmer = body.dimmer;
				if (body.folder !== undefined) layer.address.folder = body.folder;
				if (body.file !== undefined) layer.address.file = body.file;
				return jsonResponse(output);
			}
			return jsonResponse({ code: "unknown-route", message: path }, 404);
		}),
	);
	return server;
}

function writeLibrary(
	server: StubbedServer,
	path: string,
	init?: RequestInit,
): Response | undefined {
	const itemUpdate = path.match(/^\/library\/items\/([^/]+)\/update$/u);
	if (itemUpdate) {
		const body = JSON.parse(String(init?.body ?? "{}"));
		const located = server.catalog.folders
			.flatMap((folder) => folder.items.map((item) => ({ folder, item })))
			.find(({ item }) => item.id === decodeURIComponent(itemUpdate[1]));
		if (!located)
			return jsonResponse(
				{ code: "library-item-not-updated", message: "no item" },
				404,
			);
		if (body.name !== undefined) located.item.name = body.name;
		if (body.folder !== undefined && body.file !== undefined) {
			located.folder.items = located.folder.items.filter(
				(candidate) => candidate.id !== located.item.id,
			);
			let destination = server.catalog.folders.find(
				(candidate) => candidate.folder === body.folder,
			);
			if (!destination) {
				destination = { folder: body.folder, name: null, items: [] };
				server.catalog.folders.push(destination);
			}
			located.item.file = body.file;
			destination.items.push(located.item);
		}
		return jsonResponse(server.catalog);
	}
	const folderUpdate = path.match(/^\/library\/folders\/(\d+)\/update$/u);
	if (folderUpdate) {
		const body = JSON.parse(String(init?.body ?? "{}"));
		const folder = server.catalog.folders.find(
			(candidate) => candidate.folder === Number(folderUpdate[1]),
		);
		if (!folder)
			return jsonResponse(
				{ code: "library-folder-not-updated", message: "no folder" },
				404,
			);
		folder.name = body.name.trim() || null;
		return jsonResponse(server.catalog);
	}
	const upload = path.match(/^\/library\/(\d+)\/(\d+)\/upload\?/u);
	if (upload) {
		return jsonResponse({
			jobId: "upload-job",
			address: {
				folder: Number(upload[1]),
				file: Number(upload[2]),
				class: "library",
			},
		});
	}
	return undefined;
}

/// Starting and stopping imports.
function writeImport(
	server: StubbedServer,
	path: string,
): Response | undefined {
	if (path === "/library/import") {
		if (!server.imports.canImport) {
			return jsonResponse(
				{ code: "cannot-import", message: "FFmpeg is not installed" },
				503,
			);
		}
		server.imports.jobs = server.imports.pending.map((item, index) => ({
			id: `job-${index}`,
			address: item.address,
			filename: item.filename,
			state: "running",
			fraction: 0.5,
			framesDone: 50,
			framesTotal: 100,
			reason: null,
		}));
		server.imports.pending = [];
		return jsonResponse(server.imports);
	}

	const cancelled = path.match(/^\/library\/imports\/([^/]+)\/cancel$/u);
	if (cancelled) {
		const job = server.imports.jobs.find(
			(candidate) => candidate.id === cancelled[1],
		);
		if (!job)
			return jsonResponse({ code: "unknown-import", message: "no" }, 404);
		job.state = "cancelled";
		return new Response(null, { status: 204 });
	}
	return undefined;
}

/// Writing text sources.
function writeText(
	server: StubbedServer,
	path: string,
	init?: RequestInit,
): Response | undefined {
	if (path === "/text/create") {
		const body = JSON.parse(String(init?.body ?? "{}"));
		const created: TextSlotView = {
			address: {
				folder: body.folder,
				file: body.file,
				class: "text-bank",
			},
			name: body.name,
			enabled: true,
			kind: body.kind,
			text: body.text ?? null,
			durationSeconds: body.durationSeconds ?? null,
			targetUnixMillis: body.targetUnixMillis ?? null,
			style: body.style ?? aClock().style,
		};
		server.text.push(created);
		return jsonResponse(created);
	}

	const written = path.match(/^\/text\/(\d+)\/(\d+)\/(update|delete)$/u);
	if (written) {
		const body = JSON.parse(String(init?.body ?? "{}"));
		const at = server.text.findIndex(
			(candidate) =>
				candidate.address.folder === Number(written[1]) &&
				candidate.address.file === Number(written[2]),
		);
		if (at < 0)
			return jsonResponse({ code: "unknown-text", message: "no" }, 404);
		if (written[3] === "delete") {
			server.text.splice(at, 1);
			return jsonResponse(server.text);
		}
		const slot = server.text[at];
		if (body.name !== undefined) slot.name = body.name;
		if (body.enabled !== undefined) slot.enabled = body.enabled;
		if (body.kind !== undefined) slot.kind = body.kind;
		if (body.text !== undefined) slot.text = body.text;
		if (body.durationSeconds !== undefined) {
			slot.durationSeconds = body.durationSeconds;
		}
		if (body.style !== undefined) slot.style = body.style;
		return jsonResponse(slot);
	}
	return undefined;
}

function jsonResponse(body: unknown, status = 200): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { "content-type": "application/json" },
	});
}

export function anOutput(overrides: Partial<OutputView> = {}): OutputView {
	return {
		id: "11111111-1111-4111-8111-111111111111",
		name: "Main",
		layerCount: 2,
		dmxActive: false,
		master: {
			dimmer: 1,
			volume: 1,
			tintRed: 1,
			tintGreen: 1,
			tintBlue: 1,
			flipMirror: "none",
			mask: { folder: 0, file: 0, class: "blank" },
		},
		layers: [aLayer(0), aLayer(1)],
		...overrides,
	};
}

export function aLayer(index: number): OutputView["layers"][number] {
	return {
		index,
		address: { folder: 1, file: index + 1, class: "library" },
		playMode: "Loop",
		dimmer: 1,
		scaleX: 1,
		scaleY: 1,
		positionX: 0,
		positionY: 0,
		rotation: 0,
		grayscale: 0,
		sourceStatus: { state: "ready", failure: null },
		mask: {
			address: { folder: 0, file: 0, class: "blank" },
			scaleX: 1,
			scaleY: 1,
			invert: false,
			opacity: 0,
			source: "luminance",
			active: false,
		},
		drawing: true,
	};
}

export function aVisualizer(
	overrides: Partial<VisualizerView> = {},
): VisualizerView {
	return {
		address: { folder: 220, file: 1, class: "generated-visualizer" },
		typeId: 0,
		kind: "Equalizer Bars",
		name: "Equalizer Bars",
		uses: ["count", "size", "primary"],
		parameters: {
			count: 32,
			size: 0.05,
			speed: 1,
			amount: 1,
			radius: 0.3,
			thickness: 0.01,
			reactivity: 1,
			decay: 0.1,
			zoom: 1,
			iterations: 64,
			threshold: 0.5,
			smoothing: 0.5,
			gravity: 0.5,
			lifetime: 2,
			curvature: 0.2,
			primaryRed: 0.1,
			primaryGreen: 0.84,
			primaryBlue: 0.93,
			secondaryRed: 1,
			secondaryGreen: 0.7,
			secondaryBlue: 0.06,
			mirror: false,
			filled: false,
			wireframe: false,
			mode: 0,
		},
		...overrides,
	};
}

export function aCatalog(): CatalogView {
	return {
		revision: 3,
		itemCount: 2,
		folders: [
			{
				folder: 1,
				name: "Looks",
				items: [
					{
						id: "asset-a",
						file: 1,
						name: "Blue haze",
						kind: "video",
						width: 1920,
						height: 1080,
						frames: 600,
						intrinsicBpm: 120,
					},
					{
						id: "asset-b",
						file: 2,
						name: "Static grid",
						kind: "image",
						width: 1920,
						height: 1080,
						frames: null,
						intrinsicBpm: null,
					},
				],
			},
		],
	};
}

export function aDmxMap(outputId: string, outputName: string): DmxMapView {
	return {
		outputId,
		outputName,
		universe: 3,
		startAddress: 100,
		personality: "twoLayers",
		layerCount: 2,
		channels: [
			{
				absoluteChannel: 100,
				localOffset: 0,
				group: { kind: "layer", number: 1 },
				name: "Folder",
				resolution: "byte",
				defaultValue: 0,
				valueSets: [],
				implemented: true,
				implementationNote: null,
			},
		],
	};
}

export function aNetwork(overrides: Partial<NetworkView> = {}): NetworkView {
	const stored = {
		artNetListen: "0.0.0.0:6454",
		sacnListen: "0.0.0.0:5568",
		citpListen: "0.0.0.0:4811",
		httpListen: "127.0.0.1:8080",
		speedGroupEndpoint: null,
	};
	return {
		sameComputerPreset: false,
		stored,
		resolved: { ...stored },
		citpAdvertisedPort: 4811,
		takesEffectOnRestart: true,
		...overrides,
	};
}

export function aTextStyle(): TextSlotView["style"] {
	return {
		family: "sans-serif",
		size: 0.2,
		bold: false,
		italic: false,
		alignment: "center",
		red: 1,
		green: 1,
		blue: 1,
	};
}

export function aClock(overrides: Partial<TextSlotView> = {}): TextSlotView {
	return {
		address: { folder: 200, file: 1, class: "text-bank" },
		name: "Clock",
		enabled: true,
		kind: "clock",
		text: null,
		durationSeconds: null,
		targetUnixMillis: null,
		style: aTextStyle(),
		...overrides,
	};
}

export function aCountdown(
	overrides: Partial<TextSlotView> = {},
): TextSlotView {
	return {
		address: { folder: 200, file: 2, class: "text-bank" },
		name: "Ten minutes",
		enabled: true,
		kind: "countdown-duration",
		text: null,
		durationSeconds: 600,
		targetUnixMillis: null,
		style: aTextStyle(),
		...overrides,
	};
}

export function anAudioPanel(
	overrides: Partial<AudioPanelView> = {},
): AudioPanelView {
	return {
		settings: {
			deviceBy: "system-default",
			deviceValue: null,
			inputGain: 1,
			beatSensitivity: 1,
			eqBass: 1,
			eqMid: 1,
			eqTreble: 1,
			availableDevices: ["Built-in Microphone", "Desk feed"],
			deviceTakesEffectOnRestart: true,
		},
		analysis: {
			capturing: true,
			device: "Desk feed",
			detail: null,
			waveform: { points: [0, 0.5, -0.5, 0.25] },
			spectrum: [0.2, 0.5, 0.1],
			bands: { bass: 0.6, mid: 0.3, treble: 0.1 },
			energy: 0.4,
			peak: 0.8,
			beat: 0.9,
			bpm: 128,
			beatPhase: 0.25,
		},
		...overrides,
	};
}

export function aLog(overrides: Partial<LogsView> = {}): LogsView {
	return {
		records: [
			{
				sequence: 1,
				millisSinceStart: 120,
				level: "info",
				target: "media_runtime",
				message: "media server starting",
			},
			{
				sequence: 2,
				millisSinceStart: 340,
				level: "warn",
				target: "media_runtime",
				message: "no audio input; generated sources will run on silence",
			},
		],
		newest: 2,
		dropped: 0,
		capacity: 2000,
		...overrides,
	};
}

export function anImportState(
	overrides: Partial<ImportsView> = {},
): ImportsView {
	return {
		canImport: true,
		pending: [
			{
				address: { folder: 1, file: 4, class: "library" },
				name: "LoopTest",
				filename: "004-LoopTest.mp4",
			},
		],
		jobs: [],
		...overrides,
	};
}
