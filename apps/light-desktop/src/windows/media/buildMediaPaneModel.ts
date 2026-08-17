import type {
	MediaServerInspection,
	NativeMediaEffectParameter,
	NativeMediaEffectSlot,
} from "../../api/client/mediaOutput";
import type { MediaServerFixture } from "../../api/types";
import type { ProgrammerFixtureValue } from "../../features/programmerValues/contracts";
import {
	isMediaPercentAttribute,
	mediaControlDefaultNormalized,
	mediaControlOperatorValue,
} from "./mediaControlValue";
import type {
	MediaControlSection,
	MediaPaneLayer,
	MediaPaneModel,
	MediaPreviewState,
	MediaSecondaryControl,
	MediaSourceFilter,
} from "./mediaPaneModel";

export interface BuildMediaPaneModelInput {
	inspection: MediaServerInspection;
	inspectionError: string | null;
	servers: MediaServerFixture[];
	selectedServer: MediaServerFixture | undefined;
	selectedServerId: string;
	selectedLayerId: string;
	browserMode: MediaPaneModel["browserMode"];
	sourceFilter?: MediaSourceFilter;
	selectedControlSectionId: string;
	mainSectionId: string;
	rightPaneVisible: boolean;
	draftFolderId: string;
	draftFileId: string | null;
	thumbnailUrls: Record<string, string>;
	previewUrls: Record<string, string>;
	liveProgrammer: readonly ProgrammerFixtureValue[] | undefined;
	nativeEffects?: NativeMediaEffectSlot[];
	nativeEffectsError?: string | null;
}

export function buildMediaPaneModel(
	input: BuildMediaPaneModelInput,
): MediaPaneModel {
	const selectedLayer = input.selectedServer?.layers.find(
		(layer) => layer.fixture_id === input.selectedLayerId,
	);
	const selectedCitpLayer = input.selectedServer?.layers.findIndex(
		(layer) => layer.fixture_id === input.selectedLayerId,
	);
	const selectedStatus = selectedLayer
		? input.inspection.layers.find((layer) => layer.layer === selectedCitpLayer)
		: undefined;
	const capabilities = selectedLayer
		? input.inspection.capabilities.layers.find(
				(candidate) => candidate.layer === selectedLayer.head_index,
			)
		: undefined;
	const liveFolder =
		normalizedAttribute(input.liveProgrammer, "media.folder") ??
		selectedStatus?.folder;
	const liveFile =
		normalizedAttribute(input.liveProgrammer, "media.file") ??
		selectedStatus?.file;
	const sections = controlSections(
		input,
		capabilities?.secondary_controls ?? [],
	);
	return {
		hasPatchedServer: input.servers.length > 0,
		hasCitpEndpoint: Boolean(input.selectedServer?.endpoint),
		servers: serverChoices(input),
		selectedServerId: input.selectedServerId,
		selectedLayerId: input.selectedLayerId,
		preview: previewState(input),
		layers: layerModels(input),
		browserMode: input.browserMode,
		sourceFilter: input.sourceFilter ?? "media",
		showSourceFilters: Boolean(input.selectedServer?.native_action),
		maskBrowser: "supported",
		...libraryModel(input),
		...selectionModel(input, liveFolder, liveFile),
		controlSections: sections,
		selectedControlSectionId: sections.some(
			(section) => section.id === input.selectedControlSectionId,
		)
			? input.selectedControlSectionId
			: (sections.find((section) => section.id !== "native")?.id ??
				sections[0]?.id ??
				""),
		mainSectionId: input.mainSectionId,
		rightPaneVisible: input.rightPaneVisible,
		nativeManagementUrl:
			input.selectedServer?.native_action && input.selectedServer.endpoint
				? `http://${input.selectedServer.endpoint.ip_address}:8080`
				: undefined,
	};
}

function serverChoices(input: BuildMediaPaneModelInput) {
	if (input.servers.length === 0 && !input.selectedServerId)
		return [
			{
				id: "",
				name: "No media server is patched",
				statusLabel: "Missing patch",
				disabled: true,
			},
		];
	return [
		...(input.selectedServerId && !input.selectedServer
			? [
					{
						id: input.selectedServerId,
						name: "Configured server unavailable",
						statusLabel: "Missing patch",
						disabled: true,
					},
				]
			: []),
		...input.servers.map((server) => ({
			id: server.fixture_id,
			name: server.name,
			fixtureLabel:
				server.fixture_number == null
					? server.fixture_id
					: String(server.fixture_number),
			statusLabel: !server.endpoint
				? "Not configured"
				: server.status.online
					? "Online"
					: "Offline",
		})),
	];
}

function previewState(input: BuildMediaPaneModelInput): MediaPreviewState {
	if (!input.selectedServer)
		return {
			kind: "missing_patch",
			detail: "No media server is patched.",
		};
	if (!input.selectedServer.endpoint)
		return {
			kind: "offline",
			detail:
				"No CITP Media Server is available. Configure one in Show Patch > Media Servers.",
		};
	if (input.inspectionError || !input.selectedServer.status.online)
		return {
			kind: "offline",
			detail:
				input.inspectionError ??
				input.selectedServer.status.last_error ??
				"Server offline",
		};
	const source = input.inspection.preview_sources.find(
		(candidate) => candidate.layer == null,
	);
	return source
		? {
				kind: "ready",
				outputSize: { width: source.width, height: source.height },
				imageSrc:
					input.previewUrls[`${input.selectedServer.fixture_id}:${source.id}`],
			}
		: {
				kind: "unsupported",
				capability: "preview",
				detail: "No composite preview source is advertised.",
			};
}

function layerModels(input: BuildMediaPaneModelInput): MediaPaneLayer[] {
	return (input.selectedServer?.layers ?? []).map((head, citpLayer) => {
		const status = input.inspection.layers.find(
			(layer) => layer.layer === citpLayer,
		);
		const source = input.inspection.preview_sources.find(
			(candidate) => candidate.layer === citpLayer,
		);
		return {
			id: head.fixture_id,
			number: String(citpLayer + 1),
			name: status?.name || `Layer ${citpLayer + 1}`,
			status:
				status?.flags && status.flags & 0x8
					? "failed"
					: status
						? "online"
						: "unsupported",
			statusLabel: status
				? status.flags & 0x4
					? "Loading"
					: status.flags & 0x8
						? "Failed"
						: "Online"
				: "No advertised mapping",
			errorDetail:
				status?.flags && status.flags & 0x8
					? "The Media Server could not render this layer. Check its Media Server logs."
					: undefined,
			thumbnailSrc:
				source && status && (status.folder !== 0 || status.file !== 0)
					? input.previewUrls[
							`${input.selectedServer?.fixture_id}:${source.id}`
						]
					: undefined,
			liveSourceLabel: status
				? `Folder ${status.folder} · File ${status.file}`
				: undefined,
		};
	});
}

function libraryModel(input: BuildMediaPaneModelInput) {
	const sourceFilter = input.sourceFilter ?? "media";
	const draftFolder = Number(input.draftFolderId);
	const advertisedFolders = new Map(
		input.inspection.folders.map((folder) => [folder.id, folder]),
	);
	const advertisedFiles = new Map(
		input.inspection.files
			.filter((file) => file.folder_id === draftFolder)
			.map((file) => [file.id, file]),
	);
	const [firstFolder, lastFolder] =
		input.selectedServer && input.selectedLayerId === "master"
			? [1, 1]
			: sourceFilter === "media"
				? [1, 199]
				: sourceFilter === "text"
					? [200, 249]
					: [250, 255];
	return {
		libraryFolders: Array.from(
			{ length: lastFolder - firstFolder + 1 },
			(_, index) => {
				const id = firstFolder + index;
				const folder = advertisedFolders.get(id);
				return {
					id: String(id),
					kind: "folder" as const,
					name: folder?.name || `Folder ${id}`,
					detail: folder
						? `${folder.element_count} files`
						: "Configurable slot · not advertised",
				};
			},
		),
		libraryFiles: Array.from({ length: 254 }, (_, index) => {
			const id = index + 1;
			const file = advertisedFiles.get(id);
			const slotType =
				input.sourceFilter === "visualizers"
					? "Visualizer"
					: input.sourceFilter === "text"
						? "Text"
						: "Media";
			return {
				id: String(id),
				kind: "file" as const,
				name: file?.name || "Empty",
				detail: file
					? `${file.width}×${file.height}`
					: `${slotType} slot · not advertised`,
				thumbnailSrc: input.thumbnailUrls[`${draftFolder}:${id}`],
				empty: !file,
			};
		}),
	};
}

function selectionModel(
	input: BuildMediaPaneModelInput,
	liveFolder: number | undefined,
	liveFile: number | undefined,
) {
	return {
		draftFolderId: input.draftFolderId,
		draftFileId: input.draftFileId,
		liveSelection: {
			folderId: liveFolder == null ? null : String(liveFolder),
			fileId: liveFile == null ? null : String(liveFile),
			maskFolderId: null,
			maskFileId: null,
		},
		draftSelection: {
			folderId: input.draftFolderId || null,
			fileId: input.draftFileId,
			maskFolderId: null,
			maskFileId: null,
		},
		liveSelectionLabel:
			liveFolder == null
				? "No live media"
				: `Folder ${liveFolder} / File ${liveFile ?? "None"}`,
		draftSelectionLabel: input.draftFolderId
			? `Folder ${input.draftFolderId} / File ${input.draftFileId ?? "Choose"}`
			: "Choose a folder",
	};
}

function controlSections(
	input: BuildMediaPaneModelInput,
	controls: Array<{ attribute: string }>,
): MediaControlSection[] {
	const selectedMaster = input.selectedLayerId === "master";
	const selectedLayer = input.selectedServer?.layers.some(
		(layer) => layer.fixture_id === input.selectedLayerId,
	);
	if (!selectedLayer && !selectedMaster) return [];
	const groups = selectedMaster ? MASTER_CONTROL_GROUPS : MEDIA_CONTROL_GROUPS;
	const standardAttributes = new Set<string>(
		groups.flatMap((group) => [...group.attributes]),
	);
	for (const component of ["red", "green", "blue", "cyan", "magenta", "yellow"])
		standardAttributes.add(`color.${component}`);
	const remaining = new Map(
		(selectedMaster ? [] : controls)
			.filter((control) => !standardAttributes.has(control.attribute))
			.map((control) => [control.attribute, control]),
	);
	const sections: MediaControlSection[] = groups.map((group) => ({
		id: group.id,
		label: group.label,
		capability: "supported" as const,
		controls: group.attributes.map((attribute) =>
			advertisedControl(input, attribute),
		),
	}));
	if (remaining.size) {
		sections.push({
			id: "other",
			label: "Other",
			capability: "supported",
			controls: [...remaining.values()].map((control) =>
				advertisedControl(input, control.attribute),
			),
		});
	}
	const effects = sections.find((section) => section.id === "effects");
	if (effects && input.nativeEffects?.length)
		effects.controls.push(...nativeEffectControls(input.nativeEffects));
	if (effects && input.nativeEffectsError)
		effects.controls.push({
			id: "native-effects-error",
			label: "Native effect controls unavailable",
			kind: "readout",
			value: input.nativeEffectsError,
		});
	return sections;
}

const NATIVE_EFFECT_TYPES = [
	["none", "None"],
	["analog-tv", "Analog TV"],
	["digital-tv", "Digital TV"],
	["opacity-cycle", "Layer opacity cycle"],
	["blur", "Blur"],
	["feedback", "Feedback"],
	["beat-move", "Beat Move"],
	["kaleidoscope", "Kaleidoscope"],
	["rasterize", "Rasterized Print"],
	["beat-scan", "Beat Scan"],
	["beat-scale-turn", "Beat Scale and Turn"],
	["beat-grid-wave", "Beat Grid Wave"],
	["beat-form-flash", "Beat Form Flash"],
	["drawn-image", "Drawn Image"],
] as const;

function nativeEffectControls(slots: NativeMediaEffectSlot[]) {
	return slots.flatMap((slot): MediaSecondaryControl[] => {
		const prefix = `effect-${slot.index}`;
		const controls: MediaSecondaryControl[] = [
			{
				id: `${prefix}-type`,
				kind: "choice",
				label: "Effect",
				value: slot.effectType ?? "none",
				options: NATIVE_EFFECT_TYPES.map(([value, label]) => ({
					value,
					label,
				})),
				disabled: !slot.supported,
			},
		];
		if (!slot.effectType) return controls;
		controls.push(
			{
				id: `${prefix}-enabled`,
				kind: "choice",
				label: "State",
				value: String(slot.enabled),
				options: [
					{ value: "true", label: "Enabled" },
					{ value: "false", label: "Bypassed" },
				],
			},
			...slot.parameters.map((parameter) =>
				nativeEffectParameterControl(prefix, parameter),
			),
		);
		return controls;
	});
}

const NATIVE_PARAMETER_CHOICES: Record<
	string,
	Array<{ value: string; label: string }>
> = {
	"cycle-interval": [
		{ value: "every-beat", label: "Every beat" },
		{ value: "every-half-beat", label: "Every half beat" },
		{ value: "every-second", label: "Every second" },
	],
	"feedback-direction": [
		"top",
		"bottom",
		"left",
		"right",
		"rotate-left",
		"rotate-right",
	].map((value) => ({ value, label: value.replaceAll("-", " ") })),
	"beat-move-direction": ["up", "down", "left", "right"].map((value) => ({
		value,
		label: value,
	})),
	"rasterize-mode": [
		{ value: "black-and-white", label: "Black and White" },
		{ value: "cmyk", label: "CMYK" },
	],
	"beat-scan-edge": [
		{ value: "sharp", label: "Sharp" },
		{ value: "soft", label: "Soft" },
	],
	"beat-turn-enabled": [
		{ value: "false", label: "Off" },
		{ value: "true", label: "On" },
	],
	"beat-grid-origin": ["centre", "top", "right", "bottom", "left"].map(
		(value) => ({ value, label: value }),
	),
};

function nativeEffectParameterControl(
	prefix: string,
	parameter: NativeMediaEffectParameter,
): MediaSecondaryControl {
	const choices = NATIVE_PARAMETER_CHOICES[parameter.id];
	if (choices) {
		const index = Math.max(
			0,
			Math.min(choices.length - 1, Math.round(parameter.value)),
		);
		return {
			id: `${prefix}-${parameter.id}`,
			kind: "choice",
			label: parameter.label,
			value: choices[index]?.value ?? choices[0]?.value ?? "",
			options: choices,
		};
	}
	const range = nativeEffectParameterRange(parameter.id);
	const { percent, ...bounds } = range;
	return {
		id: `${prefix}-${parameter.id}`,
		kind: "value",
		label: parameter.label,
		value: parameter.value,
		...bounds,
		display: percent
			? `${Math.round(parameter.value * 100)}%`
			: String(Number(parameter.value.toFixed(2))),
	};
}

function nativeEffectParameterRange(id: string) {
	if (id === "kaleidoscope-repetitions")
		return { minimum: 1, maximum: 16, step: 1, percent: false };
	if (id.includes("angle") || id.includes("rotation"))
		return { minimum: -180, maximum: 360, step: 1, percent: false };
	if (
		id.includes("duration") ||
		id.includes("decay") ||
		id.includes("lifetime")
	)
		return { minimum: 0.05, maximum: 5, step: 0.05, percent: false };
	if (id.includes("density"))
		return { minimum: 1, maximum: 64, step: 1, percent: false };
	if (id === "rasterize-dot-size")
		return { minimum: 2, maximum: 32, step: 1, percent: false };
	return { minimum: 0, maximum: 1, step: 0.01, percent: true };
}

const MEDIA_CONTROL_GROUPS = [
	{
		id: "playback",
		label: "Playback",
		attributes: [
			"media.play_mode",
			"intensity",
			"volume",
			"media.playback_speed",
			"media.playback_bpm",
			"media.playback.blur",
		],
	},
	{
		id: "frame",
		label: "Frame",
		attributes: [
			"media.scale.x",
			"media.scale.y",
			"media.scaling_mode",
			"media.position.x",
			"media.position.y",
			"position.rotation",
		],
	},
	{
		id: "colour",
		label: "Colour",
		attributes: ["color.tint", "media.grayscale"],
	},
	{
		id: "mask-controls",
		label: "Mask",
		attributes: [
			"media.mask.position.x",
			"media.mask.position.y",
			"media.mask.scale.x",
			"media.mask.scale.y",
			"media.mask.invert",
			"media.mask.opacity",
		],
	},
	{
		id: "effects",
		label: "Effects",
		attributes: [
			"media.effect.1",
			"media.effect.2",
			"media.effect.3",
			"media.effect.4",
		],
	},
] as const;

const MASTER_CONTROL_GROUPS = [
	{
		id: "playback",
		label: "Output",
		attributes: ["intensity", "volume"],
	},
	{
		id: "frame",
		label: "Frame",
		attributes: ["media.flip_mirror"],
	},
	{
		id: "colour",
		label: "Colour",
		attributes: ["color.tint"],
	},
	{
		id: "mask-controls",
		label: "Mask position",
		attributes: ["media.mask.position.x", "media.mask.position.y"],
	},
] as const;

function advertisedControl(
	input: BuildMediaPaneModelInput,
	attribute: string,
): MediaControlSection["controls"][number] {
	const normalized =
		normalizedValue(input.liveProgrammer, attribute) ??
		(attribute === "intensity"
			? input.selectedLayerId === "master"
				? 1
				: 0
			: mediaControlDefaultNormalized(attribute));
	const rawValue = Math.round(normalized * 255);
	if (attribute === "color.tint")
		return {
			id: attribute,
			label: "Colour",
			kind: "color",
			value: mediaRgbFromComponents(
				normalizedValue(input.liveProgrammer, "color.red") ?? 1,
				normalizedValue(input.liveProgrammer, "color.green") ?? 1,
				normalizedValue(input.liveProgrammer, "color.blue") ?? 1,
			),
		};
	if (attribute === "media.play_mode")
		return {
			id: attribute,
			label: "Play mode",
			kind: "choice",
			value: String(rawValue),
			options: PLAY_MODE_OPTIONS,
			quickActions: [
				{ value: "216", label: "Stop" },
				{ value: "60", label: "Play" },
				{ value: "0", label: "Play looped" },
			],
		};
	if (attribute === "media.scaling_mode")
		return {
			id: attribute,
			label: "Scaling mode",
			kind: "choice",
			value: String(rawValue),
			options: [
				{ value: "0", label: "Fit" },
				{ value: "64", label: "Fill" },
				{ value: "128", label: "Original" },
				{ value: "192", label: "Stretch" },
			],
		};
	if (attribute === "media.mask.invert")
		return {
			id: attribute,
			label: "Invert",
			kind: "choice",
			value: rawValue < 128 ? "0" : "255",
			options: [
				{ value: "0", label: "Normal" },
				{ value: "255", label: "Invert" },
			],
		};
	if (attribute === "media.playback_speed")
		return {
			id: attribute,
			label: "Speed",
			kind: "choice",
			value: String(rawValue),
			options: SPEED_OPTIONS,
		};
	if (attribute === "media.playback_bpm") {
		return {
			id: attribute,
			label: "Playback BPM",
			kind: "value",
			value: rawValue,
			minimum: 0,
			maximum: 255,
			step: 1,
			display: rawValue === 0 ? "Off" : `${rawValue} BPM`,
		};
	}
	if (attribute === "media.playback.blur")
		return {
			id: attribute,
			label: "Blur",
			kind: "value",
			value: rawValue,
			minimum: 0,
			maximum: 255,
			step: 1,
			display: `${Math.round((rawValue / 255) * 100)}%`,
		};
	if (attribute === "media.flip_mirror")
		return {
			id: attribute,
			label: "Flip / Mirror",
			kind: "choice",
			value: String(rawValue % 4),
			options: [
				{ value: "0", label: "None" },
				{ value: "1", label: "Horizontal" },
				{ value: "2", label: "Vertical" },
				{ value: "3", label: "Both" },
			],
		};
	const value = mediaControlOperatorValue(attribute, normalized);
	if (isMediaPercentAttribute(attribute)) {
		const percent = Math.round(value);
		return {
			id: attribute,
			label: MEDIA_CONTROL_LABELS[attribute] ?? readableAttribute(attribute),
			kind: "value",
			value: percent,
			minimum: 0,
			maximum: 100,
			step: 1,
			display: `${percent}%`,
		};
	}
	if (attribute === "media.scale.x" || attribute === "media.scale.y")
		return valueControl(attribute, value, 0, 10, 0.01, `${value.toFixed(2)}×`);
	if (attribute === "media.mask.scale.x" || attribute === "media.mask.scale.y")
		return valueControl(attribute, value, 0, 2, 0.01, `${value.toFixed(2)}×`);
	if (
		attribute === "media.position.x" ||
		attribute === "media.position.y" ||
		attribute === "media.mask.position.x" ||
		attribute === "media.mask.position.y"
	)
		return valueControl(attribute, value, -2, 2, 0.01, value.toFixed(2));
	if (attribute === "position.rotation")
		return valueControl(
			attribute,
			value,
			-360,
			360,
			1,
			`${Math.round(value)}°`,
		);
	return {
		id: attribute,
		label: MEDIA_CONTROL_LABELS[attribute] ?? readableAttribute(attribute),
		kind: "value",
		value,
		minimum: 0,
		maximum: 255,
		step: 1,
	};
}

function valueControl(
	attribute: string,
	value: number,
	minimum: number,
	maximum: number,
	step: number,
	display: string,
): MediaControlSection["controls"][number] {
	return {
		id: attribute,
		label: MEDIA_CONTROL_LABELS[attribute] ?? readableAttribute(attribute),
		kind: "value",
		value,
		minimum,
		maximum,
		step,
		display,
	};
}

const SPEED_OPTIONS = [
	...Array.from({ length: 15 }, (_, index) => ({
		value: String(index * 8),
		label: `/${16 - index}`,
	})),
	{ value: "127", label: "1×" },
	...Array.from({ length: 15 }, (_, index) => {
		const multiplier = index + 2;
		const raw = Math.ceil(135 + (index * 121) / 15);
		return { value: String(raw), label: `${multiplier}×` };
	}),
];

const PLAY_MODE_OPTIONS = [
	[0, "Loop"],
	[20, "Reverse"],
	[40, "Bounce"],
	[60, "Once — Hold"],
	[68, "Once — Black"],
	[76, "Once — Transparent"],
	[84, "Reverse Once — Hold"],
	[92, "Reverse Once — Black"],
	[100, "Reverse Once — Transparent"],
	[108, "Loop Synced"],
	[128, "Reverse Synced"],
	[148, "Bounce Synced"],
	[168, "Once Synced — Hold"],
	[176, "Once Synced — Black"],
	[184, "Once Synced — Transparent"],
	[192, "Reverse Once Synced — Hold"],
	[200, "Reverse Once Synced — Black"],
	[208, "Reverse Once Synced — Transparent"],
	[216, "Stop"],
	[236, "Pause"],
].map(([value, label]) => ({ value: String(value), label: String(label) }));

const MEDIA_CONTROL_LABELS: Record<string, string> = {
	intensity: "Dimmer",
	volume: "Volume",
	"media.play_mode": "Play mode",
	"media.playback_speed": "Speed",
	"media.playback_bpm": "Playback BPM",
	"media.playback.blur": "Blur",
	"media.scale.x": "Scale X",
	"media.scale.y": "Scale Y",
	"media.scaling_mode": "Scaling mode",
	"media.position.x": "Position X",
	"media.position.y": "Position Y",
	"position.rotation": "Rotation",
	"color.tint": "Colour",
	"media.grayscale": "Grayscale",
	"media.mask.scale.x": "Mask scale X",
	"media.mask.scale.y": "Mask scale Y",
	"media.mask.position.x": "Mask position X",
	"media.mask.position.y": "Mask position Y",
	"media.mask.invert": "Invert",
	"media.mask.opacity": "Mask opacity",
	"media.effect.1": "Effect 1",
	"media.effect.2": "Effect 2",
	"media.effect.3": "Effect 3",
	"media.effect.4": "Effect 4",
	"media.flip_mirror": "Flip / Mirror",
	"media.layer.play.mode": "Play mode",
	"media.layer.dimmer": "Dimmer",
	"media.layer.volume": "Volume",
	"media.layer.speed.multiplier": "Speed",
	"media.layer.playback.bpm": "Playback BPM",
	"media.layer.scale.x": "Scale X",
	"media.layer.scale.y": "Scale Y",
	"media.layer.scaling.mode": "Scaling mode",
	"media.layer.position.x": "Position X",
	"media.layer.position.y": "Position Y",
	"media.layer.rotation": "Rotation",
	"media.layer.tint": "Colour",
	"media.layer.grayscale": "Grayscale",
	"media.layer.mask.scale.x": "Mask scale X",
	"media.layer.mask.scale.y": "Mask scale Y",
	"media.layer.mask.position.x": "Mask position X",
	"media.layer.mask.position.y": "Mask position Y",
	"media.layer.mask.invert": "Invert",
	"media.layer.mask.opacity": "Mask opacity",
	"media.layer.effect.1": "Effect 1",
	"media.layer.effect.2": "Effect 2",
	"media.layer.effect.3": "Effect 3",
	"media.layer.effect.4": "Effect 4",
};

function mediaRgbFromComponents(red: number, green: number, blue: number) {
	return `#${[red, green, blue]
		.map((component) =>
			Math.round(Math.max(0, Math.min(1, component)) * 255)
				.toString(16)
				.padStart(2, "0"),
		)
		.join("")}`;
}

function readableAttribute(attribute: string) {
	const words = attribute.split(".").slice(2);
	return words
		.map((word, index) =>
			index === 0 ? `${word.charAt(0).toUpperCase()}${word.slice(1)}` : word,
		)
		.join(" ");
}

function normalizedAttribute(
	values: readonly ProgrammerFixtureValue[] | undefined,
	attribute: string,
) {
	const value = values?.find(
		(candidate) => candidate.attribute === attribute,
	)?.value;
	return value?.kind === "normalized" && typeof value.value === "number"
		? Math.round(value.value * 255)
		: undefined;
}

function normalizedValue(
	values: readonly ProgrammerFixtureValue[] | undefined,
	attribute: string,
) {
	const value = values?.find(
		(candidate) => candidate.attribute === attribute,
	)?.value;
	return value?.kind === "normalized" && typeof value.value === "number"
		? value.value
		: undefined;
}
