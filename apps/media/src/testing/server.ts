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
	/** Every requested path, including reads, in order. */
	requests: string[];
	/** Parsed JSON bodies written to live/configuration endpoints, in order. */
	writeBodies: unknown[];
	/** Test-only gate for reproducing a slow request without delaying initial reads. */
	holdWrites: Promise<void> | undefined;
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
		requests: [],
		writeBodies: [],
		holdWrites: undefined,
		...overrides,
	};

	resetResources();
	vi.stubGlobal(
		"fetch",
		vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
			const path = String(input).replace("/api/v2", "");
			server.requests.push(path);
			if (
				init?.method === "POST" ||
				path.endsWith("/reset") ||
				path.includes("/playback/")
			) {
				server.writes.push(path);
				if (typeof init?.body === "string") {
					try {
						server.writeBodies.push(JSON.parse(init.body));
					} catch {
						// Malformed-body tests deliberately exercise the server's error response.
					}
				}
				if (server.holdWrites) await server.holdWrites;
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
			const takeover = path.match(
				/^\/outputs\/([^/]+)\/playback\/(take-over|release)$/u,
			);
			if (takeover) {
				const output = server.outputs.find(
					(candidate) => candidate.id === takeover[1],
				);
				if (!output)
					return jsonResponse({ code: "unknown-output", message: "no" }, 404);
				output.playbackTakeover = takeover[2] === "take-over";
				return jsonResponse(output);
			}

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
				for (const key of [
					"playModeDmx",
					"scaleX",
					"scaleY",
					"scalingMode",
					"positionX",
					"positionY",
					"rotation",
					"volume",
					"tintRed",
					"tintGreen",
					"tintBlue",
					"grayscale",
					"speedMultiplierDmx",
					"playbackBpm",
				])
					if (body[key] !== undefined)
						(layer as unknown as Record<string, unknown>)[key] =
							body[key] === 0 && key === "playbackBpm" ? null : body[key];
				if (body.maskFolder !== undefined)
					layer.mask.address.folder = body.maskFolder;
				if (body.maskFile !== undefined)
					layer.mask.address.file = body.maskFile;
				if (body.maskScaleX !== undefined) layer.mask.scaleX = body.maskScaleX;
				if (body.maskScaleY !== undefined) layer.mask.scaleY = body.maskScaleY;
				if (body.maskInvert !== undefined) layer.mask.invert = body.maskInvert;
				if (body.maskOpacity !== undefined)
					layer.mask.opacity = body.maskOpacity;
				if (body.effectSlot !== undefined) {
					const effect = layer.effects[body.effectSlot];
					if (body.effectType === "none") {
						layer.effects[body.effectSlot] = {
							...effect,
							effectType: null,
							label: "None",
							enabled: false,
							mix: 0,
							parameters: [],
						};
					} else if (body.effectType === "analog-tv") {
						layer.effects[body.effectSlot] = analogTvEffect(body.effectSlot);
					} else if (body.effectType === "digital-tv") {
						layer.effects[body.effectSlot] = digitalTvEffect(body.effectSlot);
					} else if (body.effectType === "opacity-cycle") {
						layer.effects[body.effectSlot] = opacityCycleEffect(
							body.effectSlot,
						);
					} else if (body.effectType === "blur") {
						layer.effects[body.effectSlot] = blurEffect(body.effectSlot);
					} else if (body.effectType === "feedback") {
						layer.effects[body.effectSlot] = feedbackEffect(body.effectSlot);
					} else if (body.effectType === "beat-move") {
						layer.effects[body.effectSlot] = beatMoveEffect(body.effectSlot);
					} else if (body.effectType === "kaleidoscope") {
						layer.effects[body.effectSlot] = kaleidoscopeEffect(
							body.effectSlot,
						);
					} else if (body.effectType === "rasterize") {
						layer.effects[body.effectSlot] = rasterizeEffect(body.effectSlot);
					} else if (body.effectType === "beat-scan") {
						layer.effects[body.effectSlot] = beatScanEffect(body.effectSlot);
					} else if (body.effectType === "beat-scale-turn") {
						layer.effects[body.effectSlot] = beatScaleTurnEffect(
							body.effectSlot,
						);
					} else if (body.effectType === "beat-grid-wave") {
						layer.effects[body.effectSlot] = beatGridWaveEffect(
							body.effectSlot,
						);
					} else if (body.effectType === "beat-form-flash") {
						layer.effects[body.effectSlot] = beatFormFlashEffect(
							body.effectSlot,
						);
					}
					const selectedEffect = layer.effects[body.effectSlot];
					if (body.visualizerParameters !== undefined)
						selectedEffect.visualizerParameters = body.visualizerParameters;
					if (body.effectEnabled !== undefined)
						selectedEffect.enabled = body.effectEnabled;
					if (body.effectMix !== undefined) selectedEffect.mix = body.effectMix;
					if (body.cycleInterval !== undefined) {
						const values = {
							"every-beat": 0,
							"every-half-beat": 1,
							"every-second": 2,
						} as const;
						selectedEffect.parameters[0].value =
							values[body.cycleInterval as keyof typeof values];
					}
					if (body.blurAmount !== undefined)
						selectedEffect.parameters[0].value = body.blurAmount;
					if (body.feedbackAmount !== undefined)
						selectedEffect.parameters[0].value = body.feedbackAmount;
					if (body.feedbackMotion !== undefined)
						selectedEffect.parameters[1].value = body.feedbackMotion;
					if (body.feedbackDirection !== undefined) {
						const directions = [
							"top",
							"bottom",
							"left",
							"right",
							"rotate-left",
							"rotate-right",
						];
						selectedEffect.parameters[2].value = directions.indexOf(
							body.feedbackDirection,
						);
					}
					if (body.beatMoveAmount !== undefined)
						selectedEffect.parameters[0].value = body.beatMoveAmount;
					if (body.beatMoveDirection !== undefined) {
						selectedEffect.parameters[1].value = [
							"up",
							"down",
							"left",
							"right",
						].indexOf(body.beatMoveDirection);
					}
					if (body.beatMoveDecay !== undefined)
						selectedEffect.parameters[2].value = body.beatMoveDecay;
					if (body.kaleidoscopeRepetitions !== undefined)
						selectedEffect.parameters[0].value = body.kaleidoscopeRepetitions;
					if (body.kaleidoscopeAngle !== undefined)
						selectedEffect.parameters[1].value = body.kaleidoscopeAngle;
					if (body.rasterizeMode !== undefined)
						selectedEffect.parameters[0].value =
							body.rasterizeMode === "cmyk" ? 1 : 0;
					if (body.rasterizeDotSize !== undefined)
						selectedEffect.parameters[1].value = body.rasterizeDotSize;
					if (body.beatScanWidth !== undefined)
						selectedEffect.parameters[0].value = body.beatScanWidth;
					if (body.beatScanEdge !== undefined)
						selectedEffect.parameters[1].value =
							body.beatScanEdge === "soft" ? 1 : 0;
					if (body.beatScanFalloff !== undefined)
						selectedEffect.parameters[2].value = body.beatScanFalloff;
					if (body.beatScanDuration !== undefined)
						selectedEffect.parameters[3].value = body.beatScanDuration;
					if (body.beatScaleAmount !== undefined)
						selectedEffect.parameters[0].value = body.beatScaleAmount;
					if (body.beatTurnEnabled !== undefined)
						selectedEffect.parameters[1].value = body.beatTurnEnabled ? 1 : 0;
					if (body.beatTurnRotation !== undefined)
						selectedEffect.parameters[2].value = body.beatTurnRotation;
					if (body.beatScaleDecay !== undefined)
						selectedEffect.parameters[3].value = body.beatScaleDecay;
					if (body.beatGridDensity !== undefined)
						selectedEffect.parameters[0].value = body.beatGridDensity;
					if (body.beatGridHeight !== undefined)
						selectedEffect.parameters[1].value = body.beatGridHeight;
					if (body.beatGridDuration !== undefined)
						selectedEffect.parameters[2].value = body.beatGridDuration;
					if (body.beatGridOrigin !== undefined)
						selectedEffect.parameters[3].value = [
							"centre",
							"top",
							"right",
							"bottom",
							"left",
						].indexOf(body.beatGridOrigin);
					if (body.beatGridHue !== undefined)
						selectedEffect.parameters[4].value = body.beatGridHue;
					if (body.beatGridBrightness !== undefined)
						selectedEffect.parameters[5].value = body.beatGridBrightness;
					if (body.beatFormEnlargement !== undefined)
						selectedEffect.parameters[0].value = body.beatFormEnlargement;
					if (body.beatFormLifetime !== undefined)
						selectedEffect.parameters[1].value = body.beatFormLifetime;
					if (body.beatFormDensity !== undefined)
						selectedEffect.parameters[2].value = body.beatFormDensity;
					if (body.beatFormVariation !== undefined)
						selectedEffect.parameters[3].value = body.beatFormVariation;
					for (const [id, key] of [
						["tv-curvature", "tvCurvature"],
						["distortion", "effectDistortion"],
						["image-grain", "imageGrain"],
						["compression-damage", "compressionDamage"],
						["block-size", "blockSize"],
						["tile-displacement", "tileDisplacement"],
						["chroma-damage", "chromaDamage"],
						["glitching", "effectGlitching"],
					] as const) {
						const parameter = selectedEffect.parameters.find(
							(candidate) => candidate.id === id,
						);
						if (parameter && body[key] !== undefined)
							parameter.value = body[key];
					}
				}
				return jsonResponse(output);
			}
			const master = path.match(/^\/outputs\/([^/]+)\/master\/update$/u);
			if (master) {
				const body = JSON.parse(String(init?.body ?? "{}"));
				const output = server.outputs.find(
					(candidate) => candidate.id === master[1],
				);
				if (!output)
					return jsonResponse({ code: "unknown-output", message: "no" }, 404);
				for (const key of [
					"dimmer",
					"volume",
					"tintRed",
					"tintGreen",
					"tintBlue",
					"flipMirror",
				])
					if (body[key] !== undefined)
						(output.master as unknown as Record<string, unknown>)[key] =
							body[key];
				if (body.maskFolder !== undefined)
					output.master.mask.folder = body.maskFolder;
				if (body.maskFile !== undefined)
					output.master.mask.file = body.maskFile;
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
		if (body.intrinsicBpm !== undefined)
			located.item.intrinsicBpm = body.intrinsicBpm;
		if (body.folder !== undefined && body.file !== undefined) {
			const occupant = server.catalog.folders
				.find((candidate) => candidate.folder === body.folder)
				?.items.find((candidate) => candidate.file === body.file);
			const previousFolder = located.folder.folder;
			const previousFile = located.item.file;
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
			if (occupant && body.swap) {
				destination.items = destination.items.filter(
					(candidate) => candidate.id !== occupant.id,
				);
				let source = server.catalog.folders.find(
					(candidate) => candidate.folder === previousFolder,
				);
				if (!source) {
					source = { folder: previousFolder, name: null, items: [] };
					server.catalog.folders.push(source);
				}
				occupant.file = previousFile;
				source.items.push(occupant);
			}
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
		if (body.swapWith !== undefined) {
			const second = server.catalog.folders.find(
				(candidate) => candidate.folder === body.swapWith,
			);
			folder.folder = body.swapWith;
			if (second) second.folder = Number(folderUpdate[1]);
			server.catalog.folders.sort((left, right) => left.folder - right.folder);
		} else {
			folder.name = body.name.trim() || null;
		}
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
		playbackTakeover: false,
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
		effects: Array.from({ length: 4 }, (_, slot) => emptyEffect(slot)),
		drawing: true,
	};
}

function emptyEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		index,
		effectType: null,
		label: "None",
		enabled: false,
		mix: 0,
		supported: true,
		capabilityDetail: null,
		parameters: [],
	};
}

function analogTvEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		...emptyEffect(index),
		effectType: "analog-tv",
		label: "Analog TV",
		enabled: true,
		mix: 1,
		parameters: [
			["tv-curvature", "TV curvature", 0.3],
			["distortion", "Distortion", 0.18],
			["image-grain", "Image grain", 0.2],
			["glitching", "Glitching", 0.08],
		].map(([id, label, value]) => ({
			id: String(id),
			label: String(label),
			value: Number(value),
			defaultValue: Number(value),
		})),
	};
}

function opacityCycleEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		...emptyEffect(index),
		effectType: "opacity-cycle",
		label: "Layer opacity cycle",
		enabled: true,
		mix: 1,
		parameters: [
			{ id: "cycle-interval", label: "Interval", value: 0, defaultValue: 0 },
		],
	};
}

function blurEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		...emptyEffect(index),
		effectType: "blur",
		label: "Blur",
		enabled: true,
		mix: 1,
		parameters: [
			{
				id: "blur-amount",
				label: "Blur amount",
				value: 0.35,
				defaultValue: 0.35,
			},
		],
	};
}

function feedbackEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		...emptyEffect(index),
		effectType: "feedback",
		label: "Feedback",
		enabled: true,
		mix: 1,
		parameters: [
			{
				id: "feedback-amount",
				label: "Feedback amount",
				value: 0.82,
				defaultValue: 0.82,
			},
			{
				id: "feedback-motion",
				label: "Motion speed",
				value: 0.25,
				defaultValue: 0.25,
			},
			{
				id: "feedback-direction",
				label: "Motion direction",
				value: 0,
				defaultValue: 0,
			},
		],
	};
}

function beatMoveEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		...emptyEffect(index),
		effectType: "beat-move",
		label: "Beat Move",
		enabled: true,
		mix: 1,
		parameters: [
			{
				id: "beat-move-amount",
				label: "Movement amount",
				value: 0.15,
				defaultValue: 0.15,
			},
			{
				id: "beat-move-direction",
				label: "Direction",
				value: 0,
				defaultValue: 0,
			},
			{
				id: "beat-move-decay",
				label: "Return time",
				value: 0.35,
				defaultValue: 0.35,
			},
		],
	};
}

function kaleidoscopeEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		...emptyEffect(index),
		effectType: "kaleidoscope",
		label: "Kaleidoscope",
		enabled: true,
		mix: 1,
		parameters: [
			{
				id: "kaleidoscope-repetitions",
				label: "Mirror repetitions",
				value: 6,
				defaultValue: 6,
			},
			{
				id: "kaleidoscope-angle",
				label: "Angle",
				value: 0,
				defaultValue: 0,
			},
		],
	};
}

function rasterizeEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		...emptyEffect(index),
		effectType: "rasterize",
		label: "Rasterized Print",
		enabled: true,
		mix: 1,
		parameters: [
			{
				id: "rasterize-mode",
				label: "Print mode",
				value: 0,
				defaultValue: 0,
			},
			{
				id: "rasterize-dot-size",
				label: "Dot size",
				value: 8,
				defaultValue: 8,
			},
		],
	};
}

function beatScanEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		...emptyEffect(index),
		effectType: "beat-scan",
		label: "Beat Scan",
		enabled: true,
		mix: 1,
		parameters: [
			{
				id: "beat-scan-width",
				label: "Scan width",
				value: 0.06,
				defaultValue: 0.06,
			},
			{
				id: "beat-scan-edge",
				label: "Edge",
				value: 0,
				defaultValue: 0,
			},
			{
				id: "beat-scan-falloff",
				label: "Edge falloff",
				value: 0.45,
				defaultValue: 0.45,
			},
			{
				id: "beat-scan-duration",
				label: "Travel time",
				value: 1,
				defaultValue: 1,
			},
		],
	};
}

function beatScaleTurnEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		...emptyEffect(index),
		effectType: "beat-scale-turn",
		label: "Beat Scale and Turn",
		enabled: true,
		mix: 1,
		parameters: [
			{
				id: "beat-scale-amount",
				label: "Scale amount",
				value: 0.15,
				defaultValue: 0.15,
			},
			{ id: "beat-turn-enabled", label: "Turn", value: 0, defaultValue: 0 },
			{
				id: "beat-turn-rotation",
				label: "Rotation amount",
				value: 5,
				defaultValue: 5,
			},
			{
				id: "beat-scale-decay",
				label: "Return time",
				value: 0.35,
				defaultValue: 0.35,
			},
		],
	};
}

function beatGridWaveEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		...emptyEffect(index),
		effectType: "beat-grid-wave",
		label: "Beat Grid Wave",
		enabled: true,
		mix: 1,
		parameters: [
			{
				id: "beat-grid-density",
				label: "Grid density",
				value: 24,
				defaultValue: 24,
			},
			{
				id: "beat-grid-height",
				label: "Wave height",
				value: 0.5,
				defaultValue: 0.5,
			},
			{
				id: "beat-grid-duration",
				label: "Travel time",
				value: 1.2,
				defaultValue: 1.2,
			},
			{
				id: "beat-grid-origin",
				label: "Wave origin",
				value: 0,
				defaultValue: 0,
			},
			{ id: "beat-grid-hue", label: "Grid hue", value: 190, defaultValue: 190 },
			{
				id: "beat-grid-brightness",
				label: "Brightness",
				value: 1,
				defaultValue: 1,
			},
		],
	};
}

function beatFormFlashEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		...emptyEffect(index),
		effectType: "beat-form-flash",
		label: "Beat Form Flash",
		enabled: true,
		mix: 1,
		parameters: [
			{
				id: "beat-form-enlargement",
				label: "Start size",
				value: 1.6,
				defaultValue: 1.6,
			},
			{
				id: "beat-form-lifetime",
				label: "Lifetime",
				value: 0.9,
				defaultValue: 0.9,
			},
			{
				id: "beat-form-density",
				label: "Forms per beat",
				value: 1,
				defaultValue: 1,
			},
			{
				id: "beat-form-variation",
				label: "Variation",
				value: 0.35,
				defaultValue: 0.35,
			},
		],
	};
}

function digitalTvEffect(
	index: number,
): OutputView["layers"][number]["effects"][number] {
	return {
		...emptyEffect(index),
		effectType: "digital-tv",
		label: "Digital TV",
		enabled: true,
		mix: 1,
		parameters: [
			["compression-damage", "Compression damage", 0.35],
			["block-size", "Block size", 0.35],
			["tile-displacement", "Tile displacement", 0.25],
			["chroma-damage", "Chroma damage", 0.2],
			["glitching", "Glitching", 0.15],
		].map(([id, label, value]) => ({
			id: String(id),
			label: String(label),
			value: Number(value),
			defaultValue: Number(value),
		})),
	};
}

export function aVisualizer(
	overrides: Partial<VisualizerView> = {},
): VisualizerView {
	return {
		address: { folder: 250, file: 1, class: "generated-visualizer" },
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
		citpListen: "0.0.0.0:4809",
		httpListen: "127.0.0.1:8080",
		speedGroupEndpoint: null,
	};
	return {
		sameComputerPreset: false,
		stored,
		resolved: { ...stored },
		citpAdvertisedPort: 4809,
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
