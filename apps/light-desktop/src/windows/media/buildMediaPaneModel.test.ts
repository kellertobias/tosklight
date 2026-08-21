import { describe, expect, it } from "vitest";
import {
	type BuildMediaPaneModelInput,
	buildMediaPaneModel,
} from "./buildMediaPaneModel";
import { EMPTY_MEDIA_INSPECTION } from "./useMediaPaneData";

function input(
	patch: Partial<BuildMediaPaneModelInput> = {},
): BuildMediaPaneModelInput {
	return {
		inspection: EMPTY_MEDIA_INSPECTION,
		inspectionError: null,
		servers: [],
		selectedServer: undefined,
		selectedServerId: "",
		selectedLayerId: "master",
		browserMode: "media",
		selectedControlSectionId: "playback",
		mainSectionId: "content",
		rightPaneVisible: false,
		draftFolderId: "0",
		draftFileId: null,
		thumbnailUrls: {},
		previewUrls: {},
		liveProgrammer: undefined,
		...patch,
	};
}

describe("Media pane disconnected configuration", () => {
	it("projects a truthful no-patch state without inventing a CITP problem", () => {
		const model = buildMediaPaneModel(input());
		expect(model.servers).toEqual([
			expect.objectContaining({ name: "No media server is patched" }),
		]);
		expect(model.hasPatchedServer).toBe(false);
		expect(model.hasCitpEndpoint).toBe(false);
		expect(model.showSourceFilters).toBe(false);
		expect(model.preview).toMatchObject({
			kind: "missing_patch",
			detail: "No media server is patched.",
		});
		expect(model.maskBrowser).toBe("supported");
		expect(model.libraryFolders).toHaveLength(199);
		expect(model.libraryFiles).toHaveLength(254);
		expect(model.libraryFiles[0]).toMatchObject({
			id: "1",
			name: "Empty",
			empty: true,
		});
		expect(model.libraryFiles.at(-1)).toMatchObject({
			id: "254",
			name: "Empty",
			empty: true,
		});
		expect(model.libraryFiles.some((file) => file.id === "0")).toBe(false);
		expect(model.libraryFolders[0]).toMatchObject({
			id: "1",
			name: "Folder 1",
		});
		expect(model.libraryFolders[198]).toMatchObject({
			id: "199",
			name: "Folder 199",
		});
	});

	it("scopes thumbnails and empty slot labels to the selected source folder", () => {
		const visualizers = buildMediaPaneModel(
			input({
				inspection: {
					...EMPTY_MEDIA_INSPECTION,
					files: [
						{
							id: 1,
							folder_id: 250,
							name: "Bars",
							width: 128,
							height: 72,
							length_frames: 1,
							fps: 25,
						},
					],
				},
				sourceFilter: "visualizers",
				draftFolderId: "250",
				thumbnailUrls: {
					"1:1": "blob:media-one",
					"250:1": "blob:visualizer-one",
				},
			}),
		);
		expect(visualizers.libraryFiles[0]).toMatchObject({
			name: "Bars",
			empty: false,
			thumbnailSrc: "blob:visualizer-one",
		});

		const text = buildMediaPaneModel(
			input({
				sourceFilter: "text",
				draftFolderId: "200",
				thumbnailUrls: { "250:1": "blob:visualizer-one" },
			}),
		);
		expect(text.libraryFiles[0]).toMatchObject({
			name: "Empty",
			detail: "Text slot · not advertised",
			empty: true,
		});
		expect(text.libraryFiles[0]?.thumbnailSrc).toBeUndefined();
	});

	it("keeps an unconfigured patched server and its logical layers selectable", () => {
		const server = {
			fixture_id: "server-1",
			fixture_number: 1001,
			name: "Media master",
			endpoint: null,
			layers: [{ fixture_id: "layer-1", head_index: 0 }],
			status: { online: false, last_success: null, last_error: null },
		};
		const model = buildMediaPaneModel(
			input({
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
			}),
		);
		expect(model.servers[0]).toMatchObject({
			id: "server-1",
			fixtureLabel: "1001",
			statusLabel: "Not configured",
		});
		expect(model.hasPatchedServer).toBe(true);
		expect(model.hasCitpEndpoint).toBe(false);
		expect(model.showSourceFilters).toBe(false);
		expect(model.layers).toHaveLength(1);
		expect(model.preview).toMatchObject({
			kind: "offline",
			detail: expect.stringContaining("No CITP Media Server is available"),
		});
	});

	it("reconciles advertised names without removing unadvertised slots", () => {
		const inspection = {
			...EMPTY_MEDIA_INSPECTION,
			library_revision: "live-7",
			folders: [{ id: 7, name: "Tour package", element_count: 1 }],
			files: [
				{
					folder_id: 7,
					id: 19,
					name: "Opening loop",
					width: 1920,
					height: 1080,
					length_frames: 250,
					fps: 25,
				},
			],
		};
		const model = buildMediaPaneModel(
			input({ inspection, draftFolderId: "7" }),
		);
		expect(model.libraryFolders).toHaveLength(199);
		expect(model.libraryFolders[6]).toMatchObject({
			id: "7",
			name: "Tour package",
		});
		expect(model.libraryFolders[7]).toMatchObject({
			id: "8",
			name: "Folder 8",
		});
		expect(model.libraryFiles[18]).toMatchObject({
			id: "19",
			name: "Opening loop",
		});
		expect(model.libraryFiles[19]).toMatchObject({
			id: "20",
			name: "Empty",
			empty: true,
		});
	});

	it("projects stable visualizer and text address ranges independently", () => {
		const visualizers = buildMediaPaneModel(
			input({ sourceFilter: "visualizers" }),
		);
		expect(visualizers.libraryFolders).toHaveLength(6);
		expect(visualizers.libraryFolders.map((folder) => folder.id)).toEqual([
			"250",
			"251",
			"252",
			"253",
			"254",
			"255",
		]);
		const text = buildMediaPaneModel(input({ sourceFilter: "text" }));
		expect(text.libraryFolders).toHaveLength(50);
		expect(text.libraryFolders[0].id).toBe("200");
		expect(text.libraryFolders[49].id).toBe("249");
	});

	it("shows native source filters only for a native ToskLight server", () => {
		const baseServer = {
			fixture_id: "server-1",
			name: "Media master",
			endpoint: null,
			layers: [],
			status: { online: false, last_success: null, last_error: null },
		};
		expect(
			buildMediaPaneModel(
				input({
					servers: [baseServer],
					selectedServer: baseServer,
					selectedServerId: baseServer.fixture_id,
				}),
			).showSourceFilters,
		).toBe(false);

		const nativeServer = {
			...baseServer,
			native_action: "tosklight_media_v2",
		};
		expect(
			buildMediaPaneModel(
				input({
					servers: [nativeServer],
					selectedServer: nativeServer,
					selectedServerId: nativeServer.fixture_id,
				}),
			).showSourceFilters,
		).toBe(true);
	});

	it("lists every patched Internal Audio Player and previews it as audio", () => {
		const player = (suffix: string, folder: number, file: number) => ({
			fixture_id: `player-${suffix}`,
			fixture_number: Number(suffix),
			name: "ToskLight Audio Player",
			kind: "audio_player" as const,
			endpoint: null,
			layers: [{ fixture_id: `player-${suffix}-head`, head_index: 0 }],
			status: { online: true, last_success: null, last_error: null },
			audio: {
				folder,
				file,
				volume_percent: 42,
				transport: "play" as const,
				repeat: false,
				source: `${String(folder).padStart(3, "0")}/${String(file).padStart(3, "0")}.wav`,
			},
		});
		const first = player("1", 3, 12);
		const second = player("2", 1, 1);
		const model = buildMediaPaneModel(
			input({
				servers: [first, second],
				selectedServer: first,
				selectedServerId: first.fixture_id,
			}),
		);
		expect(model.servers.map((server) => server.id)).toEqual([
			"player-1",
			"player-2",
		]);
		expect(model.servers[0]).toMatchObject({ statusLabel: "Internal" });
		expect(model.preview).toEqual({
			kind: "audio",
			detail: "003/012.wav",
		});
		expect(model.layers).toEqual([
			expect.objectContaining({
				id: "player-1-head",
				status: "online",
				statusLabel: "Playing",
				audio: { volumeLabel: "42%", sourceLabel: "003 / 012" },
			}),
		]);
	});

	it("greys out controls the addressed head does not carry", () => {
		const player = {
			fixture_id: "player-1",
			name: "ToskLight Audio Player",
			kind: "audio_player" as const,
			endpoint: null,
			layers: [
				{
					fixture_id: "player-1-head",
					head_index: 0,
					attributes: [
						"media.file",
						"media.folder",
						"media.play_mode",
						"volume",
					],
				},
			],
			master_attributes: ["media.file", "media.folder"],
			status: { online: true, last_success: null, last_error: null },
			audio: {
				folder: 1,
				file: 1,
				volume_percent: 100,
				transport: "play" as const,
				repeat: true,
				source: "001/001.wav",
			},
		};
		const model = buildMediaPaneModel(
			input({
				servers: [player],
				selectedServer: player,
				selectedServerId: player.fixture_id,
				selectedLayerId: "player-1-head",
			}),
		);
		const playback = model.controlSections.find(
			(section) => section.id === "playback",
		);
		expect(playback?.controls.map((control) => control.id)).toEqual([
			"media.play_mode",
			"intensity",
			"volume",
			"media.playback_speed",
			"media.playback_bpm",
			"media.playback.blur",
		]);
		expect(
			playback?.controls.map((control) => Boolean(control.disabled)),
		).toEqual([false, true, false, true, true, true]);
		expect(
			model.controlSections
				.find((section) => section.id === "frame")
				?.controls.every((control) => control.disabled),
		).toBe(true);
	});

	it("keeps every control enabled when a server reports no attribute set", () => {
		const server = {
			fixture_id: "server-1",
			name: "Media master",
			endpoint: { protocol: "citp" as const, ip_address: "127.0.0.1", port: 4809 },
			layers: [{ fixture_id: "layer-1", head_index: 0 }],
			status: { online: true, last_success: null, last_error: null },
		};
		const model = buildMediaPaneModel(
			input({
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
			}),
		);
		expect(
			model.controlSections.every((section) =>
				section.controls.every((control) => !control.disabled),
			),
		).toBe(true);
	});

	it("keeps an unavailable Internal Audio Player truthful", () => {
		const player = {
			fixture_id: "player-1",
			name: "ToskLight Audio Player",
			kind: "audio_player" as const,
			endpoint: null,
			layers: [{ fixture_id: "player-1-head", head_index: 0 }],
			status: {
				online: false,
				last_success: null,
				last_error: "Audio Player output binding default is not mapped",
			},
			audio: {
				folder: 0,
				file: 0,
				volume_percent: 0,
				transport: "stop" as const,
				repeat: false,
				source: null,
			},
		};
		const model = buildMediaPaneModel(
			input({
				servers: [player],
				selectedServer: player,
				selectedServerId: player.fixture_id,
			}),
		);
		expect(model.servers[0]).toMatchObject({ statusLabel: "Unavailable" });
		expect(model.preview).toEqual({
			kind: "audio",
			detail: "Audio Player output binding default is not mapped",
		});
		expect(model.layers[0]).toMatchObject({
			status: "failed",
			statusLabel: "Failed",
			audio: { volumeLabel: "0%", sourceLabel: "000 / 000" },
			liveSourceLabel: "No audio source selected",
		});
	});

	it("projects the advertised composite dimensions into the master output", () => {
		const server = {
			fixture_id: "server-1",
			name: "Media master",
			endpoint: {
				protocol: "citp" as const,
				ip_address: "127.0.0.1",
				port: 4809,
			},
			layers: [],
			status: { online: true, last_success: null, last_error: null },
		};
		const inspection = {
			...EMPTY_MEDIA_INSPECTION,
			preview_sources: [
				{
					id: 3,
					name: "Program",
					physical_output: 0,
					layer: null,
					width: 1024,
					height: 768,
				},
			],
		};
		const model = buildMediaPaneModel(
			input({
				inspection,
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
			}),
		);

		expect(model.preview).toMatchObject({
			kind: "ready",
			outputSize: { width: 1024, height: 768 },
		});
		expect(model.hasPatchedServer).toBe(true);
		expect(model.hasCitpEndpoint).toBe(true);
	});

	it("maps advertised isolated layer frames and failure state into the layer tile", () => {
		const server = {
			fixture_id: "server-1",
			name: "Media master",
			endpoint: {
				protocol: "citp" as const,
				ip_address: "127.0.0.1",
				port: 4809,
			},
			layers: [
				{ fixture_id: "layer-1", head_index: 1 },
				{ fixture_id: "layer-2", head_index: 2 },
			],
			status: { online: true, last_success: null, last_error: null },
		};
		const inspection = {
			...EMPTY_MEDIA_INSPECTION,
			layers: [
				{
					layer: 0,
					physical_output: 0,
					folder: 1,
					file: 2,
					name: "Damaged",
					position_frames: 0,
					length_frames: 1,
					fps: 25,
					flags: 0x8,
				},
			],
			preview_sources: [
				{
					id: 4,
					name: "Layer 1",
					physical_output: 0,
					layer: 0,
					width: 320,
					height: 180,
				},
			],
		};
		const model = buildMediaPaneModel(
			input({
				inspection,
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
				previewUrls: { "server-1:4": "blob:live-layer" },
			}),
		);

		expect(model.layers[0]).toMatchObject({
			number: "1",
			thumbnailSrc: "blob:live-layer",
			status: "failed",
			errorDetail: expect.stringContaining("could not render"),
		});
		expect(model.layers[1]).toMatchObject({
			number: "2",
			status: "unsupported",
		});
	});

	it("preserves the Media Server control grouping instead of flattening the layer", () => {
		const server = {
			fixture_id: "server-1",
			name: "ToskLight Media Server",
			endpoint: {
				protocol: "citp" as const,
				ip_address: "127.0.0.1",
				port: 4809,
			},
			layers: [{ fixture_id: "layer-1", head_index: 0 }],
			status: { online: true, last_success: null, last_error: null },
		};
		const attributes = [
			"media.play_mode",
			"intensity",
			"media.scale.x",
			"media.position.y",
			"color.cyan",
			"media.mask.opacity",
			"media.effect.1",
		];
		const inspection = {
			...EMPTY_MEDIA_INSPECTION,
			capabilities: {
				...EMPTY_MEDIA_INSPECTION.capabilities,
				layers: [
					{
						layer: 0,
						content_library: true,
						mask_library: true,
						secondary_controls: attributes.map((attribute) => ({ attribute })),
					},
				],
			},
		};
		const model = buildMediaPaneModel(
			input({
				inspection,
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
				selectedControlSectionId: "frame",
			}),
		);

		expect(
			model.controlSections.map(({ id, label }) => ({ id, label })),
		).toEqual([
			{ id: "playback", label: "Playback" },
			{ id: "frame", label: "Frame" },
			{ id: "colour", label: "Colour" },
			{ id: "mask-controls", label: "Mask" },
			{ id: "effects", label: "Effects" },
		]);
		expect(model.selectedControlSectionId).toBe("frame");
		expect(model.controlSections[0]?.controls).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					id: "media.play_mode",
					label: "Play mode",
				}),
				expect.objectContaining({
					id: "intensity",
					label: "Dimmer",
				}),
			]),
		);
		expect(model.controlSections[1]?.controls).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					id: "media.scale.x",
					label: "Scale X",
					value: 1,
				}),
				expect.objectContaining({
					id: "media.scale.y",
					label: "Scale Y",
					value: 1,
				}),
			]),
		);
		expect(model.controlSections[2]?.controls[0]).toMatchObject({
			id: "color.tint",
			label: "Colour",
			kind: "color",
			value: "#ffffff",
		});
	});

	it("shows regular media fixture attributes without a CITP endpoint", () => {
		const server = {
			fixture_id: "server-1",
			name: "Patched Media Server",
			endpoint: null,
			layers: [{ fixture_id: "layer-1", head_index: 1 }],
			status: { online: false, last_success: null, last_error: null },
		};
		const model = buildMediaPaneModel(
			input({
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
			}),
		);

		expect(model.controlSections.map((section) => section.id)).toEqual([
			"playback",
			"frame",
			"colour",
			"mask-controls",
			"effects",
		]);
		expect(
			model.controlSections.every((section) => section.controls.length > 0),
		).toBe(true);
	});

	it("keeps Master fixture controls visible with its live-output defaults", () => {
		const server = {
			fixture_id: "server-1",
			name: "Patched Media Server",
			endpoint: null,
			layers: [{ fixture_id: "layer-1", head_index: 0 }],
			status: { online: false, last_success: null, last_error: null },
		};
		const model = buildMediaPaneModel(
			input({
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "master",
				browserMode: "mask",
				mainSectionId: "mask",
				draftFolderId: "1",
			}),
		);

		expect(model.controlSections.map((section) => section.id)).toEqual([
			"playback",
			"frame",
			"mask-controls",
			"shapers",
			"colour",
		]);
		expect(model.controlSections[0]?.controls).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					id: "intensity",
					value: 100,
					display: "100%",
				}),
				expect.objectContaining({
					id: "volume",
					value: 100,
					display: "100%",
				}),
			]),
		);
		expect(model.controlSections[4]?.controls[0]).toMatchObject({
			id: "color.tint",
			kind: "color",
			value: "#ffffff",
		});
		expect(model.controlSections[1]?.controls).toEqual(
			expect.arrayContaining([
				expect.objectContaining({ id: "media.scale.x", maximum: 4 }),
				expect.objectContaining({ id: "media.position.x", value: 0 }),
				expect.objectContaining({ id: "position.rotation", maximum: 180 }),
			]),
		);
		expect(model.controlSections[3]?.controls).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					id: "shaper.blade.1.position",
					label: "Left",
				}),
				expect.objectContaining({
					id: "shaper.blade.4.angle",
					label: "Bottom rotation",
				}),
				expect.objectContaining({
					id: "shaper.rotation",
					label: "Module rotation",
				}),
			]),
		);
		expect(model.libraryFolders.map((folder) => folder.id)).toEqual(["1"]);
	});

	it("starts every media layer with zero intensity", () => {
		const server = {
			fixture_id: "server-1",
			name: "Patched Media Server",
			endpoint: null,
			layers: [{ fixture_id: "layer-1", head_index: 0 }],
			status: { online: false, last_success: null, last_error: null },
		};
		const model = buildMediaPaneModel(
			input({
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
			}),
		);

		expect(
			model.controlSections[0]?.controls.find(
				(control) => control.id === "intensity",
			),
		).toMatchObject({ value: 0, display: "0%" });
	});

	it("maps centered frame attributes onto their physical operator ranges", () => {
		const server = {
			fixture_id: "server-1",
			name: "Patched Media Server",
			endpoint: null,
			layers: [{ fixture_id: "layer-1", head_index: 1 }],
			status: { online: false, last_success: null, last_error: null },
		};
		const values = [
			["media.scale.x", 1],
			["media.position.x", 0.5],
			["media.mask.position.x", 0.5],
			["position.rotation", 0.5],
		] as const;
		const model = buildMediaPaneModel(
			input({
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
				liveProgrammer: values.map(([attribute, value], programmerOrder) => ({
					fixtureId: "layer-1",
					attribute,
					value: { kind: "normalized", value },
					programmerOrder,
					fade: false,
					fadeMillis: null,
					delayMillis: null,
				})),
			}),
		);
		const controls = model.controlSections[1]?.controls ?? [];

		expect(
			controls.find((control) => control.id === "media.scale.x"),
		).toMatchObject({ value: 10, minimum: 0, maximum: 10, display: "10.00×" });
		expect(
			controls.find((control) => control.id === "media.position.x"),
		).toMatchObject({ value: 0, minimum: -2, maximum: 2, display: "0.00" });
		const maskControls = model.controlSections.find(
			(section) => section.id === "mask-controls",
		)?.controls;
		expect(
			maskControls?.find((control) => control.id === "media.mask.position.x"),
		).toMatchObject({ value: 0, minimum: -2, maximum: 2, display: "0.00" });
		expect(
			controls.find((control) => control.id === "position.rotation"),
		).toMatchObject({ value: 0, minimum: -360, maximum: 360, display: "0°" });
	});

	it("projects subtractive fixture values through one RGB colour picker", () => {
		const server = {
			fixture_id: "server-1",
			name: "Patched Media Server",
			endpoint: null,
			layers: [{ fixture_id: "layer-1", head_index: 1 }],
			status: { online: false, last_success: null, last_error: null },
		};
		const model = buildMediaPaneModel(
			input({
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
				liveProgrammer: [
					["color.red", 1],
					["color.green", 0.5],
					["color.blue", 0],
				].map(([attribute, value], programmerOrder) => ({
					fixtureId: "layer-1",
					attribute: String(attribute),
					value: { kind: "normalized" as const, value: Number(value) },
					programmerOrder,
					fade: false,
					fadeMillis: null,
					delayMillis: null,
				})),
			}),
		);

		expect(model.controlSections[2]?.controls[0]).toMatchObject({
			id: "color.tint",
			kind: "color",
			value: "#ff8000",
		});
	});

	it("places native server effect controls inside their Effect slot", () => {
		const server = {
			fixture_id: "server-1",
			name: "ToskLight Media Server",
			endpoint: null,
			native_action: "tosklight_media_v2",
			layers: [{ fixture_id: "layer-1", head_index: 1 }],
			status: { online: false, last_success: null, last_error: null },
		};
		const model = buildMediaPaneModel(
			input({
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
				nativeEffects: [
					{
						index: 0,
						effectType: "blur",
						label: "Blur",
						enabled: true,
						mix: 0.5,
						supported: true,
						capabilityDetail: null,
						parameters: [
							{
								id: "blur-amount",
								label: "Blur amount",
								value: 0.8,
								defaultValue: 0.5,
								minimum: 0,
								maximum: 1,
								step: 0.01,
							},
						],
					},
				],
			}),
		);

		expect(model.controlSections.map((section) => section.id)).not.toContain(
			"native",
		);
		expect(model.controlSections.at(-1)).toMatchObject({
			id: "effects",
			controls: expect.arrayContaining([
				expect.objectContaining({ id: "effect-0-type", kind: "choice" }),
				expect.objectContaining({ id: "effect-0-blur-amount", kind: "value" }),
			]),
		});
	});

	it("offers exactly the range the Media Server advertises for a parameter", () => {
		const server = {
			fixture_id: "server-1",
			name: "ToskLight Media Server",
			endpoint: null,
			native_action: "tosklight_media_v2",
			layers: [{ fixture_id: "layer-1", head_index: 1 }],
			status: { online: true, last_success: null, last_error: null },
		};
		const model = buildMediaPaneModel(
			input({
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
				nativeEffects: [
					{
						index: 0,
						effectType: "kaleidoscope",
						label: "Kaleidoscope",
						enabled: true,
						mix: 1,
						supported: true,
						capabilityDetail: null,
						parameters: [
							{
								id: "kaleidoscope-repetitions",
								label: "Mirror repetitions",
								value: 6,
								defaultValue: 6,
								minimum: 1,
								maximum: 16,
								step: 1,
							},
							{
								id: "kaleidoscope-angle",
								label: "Angle",
								value: 0,
								defaultValue: 0,
								minimum: -180,
								maximum: 180,
								step: 1,
							},
							{
								id: "future-amount",
								label: "Future amount",
								value: 0.4,
								defaultValue: 0.5,
								minimum: null,
								maximum: null,
								step: null,
							},
						],
					},
				],
			}),
		);

		const controls = model.controlSections.at(-1)?.controls ?? [];
		expect(
			controls.find((control) => control.id === "effect-0-kaleidoscope-repetitions"),
		).toMatchObject({
			kind: "value",
			minimum: 1,
			maximum: 16,
			step: 1,
			displayFormat: "integer",
		});
		// The desk used to offer 360 here, and every value above 180 was refused.
		expect(
			controls.find((control) => control.id === "effect-0-kaleidoscope-angle"),
		).toMatchObject({ minimum: -180, maximum: 180 });
		// A Media Server too old to advertise still gets the conservative normalized amount.
		expect(
			controls.find((control) => control.id === "effect-0-future-amount"),
		).toMatchObject({ minimum: 0, maximum: 1, displayFormat: "percent" });
	});

	it("shows percentage controls as percentages instead of raw DMX bytes", () => {
		const server = {
			fixture_id: "server-1",
			name: "ToskLight Media Server",
			endpoint: {
				protocol: "citp" as const,
				ip_address: "127.0.0.1",
				port: 4809,
			},
			layers: [{ fixture_id: "layer-1", head_index: 1 }],
			status: { online: true, last_success: null, last_error: null },
		};
		const inspection = {
			...EMPTY_MEDIA_INSPECTION,
			capabilities: {
				...EMPTY_MEDIA_INSPECTION.capabilities,
				layers: [
					{
						layer: 1,
						content_library: true,
						mask_library: true,
						secondary_controls: [{ attribute: "intensity" }],
					},
				],
			},
		};
		const model = buildMediaPaneModel(
			input({
				inspection,
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
				liveProgrammer: [
					{
						fixtureId: "layer-1",
						attribute: "intensity",
						value: { kind: "normalized", value: 1 },
						programmerOrder: 0,
						fade: false,
						fadeMillis: null,
						delayMillis: null,
					},
				],
			}),
		);

		expect(
			model.controlSections[0]?.controls.find(
				(control) => control.id === "intensity",
			),
		).toMatchObject({
			value: 100,
			maximum: 100,
			display: "100%",
		});
	});
});
