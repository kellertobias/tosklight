import { useEffect, useMemo, useState } from "react";
import {
	type MediaBrowserMode,
	type MediaLibraryItem,
	type MediaPaneModel,
	MediaPaneSurface,
	type MediaSecondaryControl,
	type MediaSourceFilter,
} from "../../../../light-desktop/src/windows/media/MediaPaneSurface";
import { useFailureToast } from "../../app/ToastContext";
import { resolveAddress } from "../../entities/catalog";
import { sourceBadge } from "../../entities/output";
import {
	PlaybackTakeoverBoundary,
	usePlaybackTakeover,
} from "../../operator/PlaybackTakeoverContext";
import { api } from "../../shared/api/client";
import type {
	OutputView,
	UpdateLayer,
	UpdateMaster,
	VisualizerParametersView,
	VisualizerView,
} from "../../shared/api/generated/media-wire";
import {
	useCatalog,
	useEffects,
	useModels,
	useRuntime,
	useText,
	useVisualizers,
} from "../../shared/api/queries";
import { textPreviewUrl } from "../text-sources/TextSourcesPage";
import { visualizerPreviewUrl } from "../visualizers/preview";
import { effectLayerChange } from "./effectLayerChange";
import {
	blendSection,
	effectBankSection,
	frameSection,
	layerDmxChange,
	playbackRangeControls,
	VISUALIZER_FLAGS,
	VISUALIZER_GROUP,
	VISUALIZER_NUMBERS,
	valueControl,
	visualizerParameterControls,
} from "./layerDmxSections";
import { useOutputFacts } from "./useOutputFacts";

const CATALOG_POLL_MS = 15_000;

/**
 * The Media Server's Media screen is the production CITP Media Pane surface.
 * Only the data adapter differs: this product already owns the server and can
 * project its HTTP output/catalog state directly into the pane's view model.
 */
export function MediaPanePage() {
	return (
		<PlaybackTakeoverBoundary>
			<MediaPanePageContent />
		</PlaybackTakeoverBoundary>
	);
}

function MediaPanePageContent() {
	const { outputs, control, selectedOutputId, selectOutput } =
		usePlaybackTakeover();
	const catalog = useCatalog(CATALOG_POLL_MS);
	const runtime = useRuntime();
	const text = useText();
	const visualizers = useVisualizers();
	const effects = useEffects();
	const models = useModels();
	const layers = useMemo(
		() =>
			(outputs.data ?? []).flatMap((output) =>
				output.layers.map((layer) => ({ output, layer })),
			),
		[outputs.data],
	);
	const [selectedLayerId, setSelectedLayerId] = useState("");
	const [browserMode, setBrowserMode] = useState<MediaBrowserMode>("media");
	const [sourceFilter, setSourceFilter] = useState<MediaSourceFilter>("media");
	const [draftFolderId, setDraftFolderId] = useState("");
	const [draftFileId, setDraftFileId] = useState<string | null>(null);
	const [mainSectionId, setMainSectionId] = useState("content");
	const [selectedControlSectionId, setSelectedControlSectionId] =
		useState("playback");
	const [rightPaneVisible, setRightPaneVisible] = useState(true);
	const [previewRevision, setPreviewRevision] = useState(0);
	useEffect(() => {
		const first = layers[0];
		if (!selectedLayerId && first) {
			setSelectedLayerId(layerId(first.output.id, first.layer.index));
			selectOutput(first.output.id);
			setDraftFolderId(String(first.layer.address.folder || 1));
			setDraftFileId(
				first.layer.address.file ? String(first.layer.address.file) : null,
			);
		}
	}, [layers, selectedLayerId, selectOutput]);

	const selected =
		selectedLayerId === "master"
			? undefined
			: (layers.find(
					({ output, layer }) =>
						layerId(output.id, layer.index) === selectedLayerId,
				) ?? (!selectedLayerId ? layers[0] : undefined));
	const selectedOutput =
		(outputs.data ?? []).find((output) => output.id === selectedOutputId) ??
		selected?.output ??
		outputs.data?.[0];
	const takeover = selectedOutput?.playbackTakeover ?? false;
	const selectedVisualizer = selected
		? visualizers.data?.find(
				(candidate) =>
					candidate.address.folder === selected.layer.address.folder &&
					candidate.address.file === selected.layer.address.file,
			)
		: undefined;
	const displayedVisualizer = selectedVisualizer
		? {
				...selectedVisualizer,
				parameters:
					selected?.layer.visualizerParameters ?? selectedVisualizer.parameters,
			}
		: undefined;
	const runningOutput = runtime.data?.outputs.find(
		(output) => output.id === selectedOutput?.id,
	);
	const titleInfo = {
		primary:
			runtime.data && runningOutput
				? `${runtime.data.administrationIp} · DMX U${runningOutput.universe} A${runningOutput.startAddress}`
				: "Running output unavailable",
		secondary: `${catalog.data?.itemCount ?? 0} library ${catalog.data?.itemCount === 1 ? "item" : "items"}`,
	};
	const sourceFailure = outputSourceFailures(outputs.data ?? []);
	useFailureToast(sourceFailure ? { message: sourceFailure } : undefined);
	const { previewSize } = useOutputFacts(selectedOutput?.id);
	useEffect(() => {
		const timer = window.setInterval(
			() => setPreviewRevision((revision) => revision + 1),
			500,
		);
		return () => window.clearInterval(timer);
	}, []);
	useEffect(() => {
		if (takeover || !selected) return;
		setDraftFolderId(String(selected.layer.address.folder || 1));
		setDraftFileId(
			selected.layer.address.file ? String(selected.layer.address.file) : null,
		);
	}, [takeover, selected]);
	const draftFolder = Number(draftFolderId || 1);
	const folder = catalog.data?.folders.find(
		(candidate) => candidate.folder === draftFolder,
	);
	const generatedFolders = new Map<
		number,
		{ name: string; files: MediaLibraryItem[] }
	>();
	for (const slot of text.data ?? []) {
		const entry = generatedFolders.get(slot.address.folder) ?? {
			name: "Text",
			files: [],
		};
		entry.files.push({
			id: String(slot.address.file),
			kind: "file",
			name: slot.name,
			detail: slot.kind,
			thumbnailSrc: textPreviewUrl(slot, previewAspectRatio(previewSize)),
		});
		generatedFolders.set(slot.address.folder, entry);
	}
	for (const visualizer of visualizers.data ?? []) {
		const entry = generatedFolders.get(visualizer.address.folder) ?? {
			name: "Visualizers",
			files: [],
		};
		entry.files.push({
			id: String(visualizer.address.file),
			kind: "file",
			name: visualizer.name,
			detail: visualizer.kind,
			thumbnailSrc: visualizerPreviewUrl(visualizer),
		});
		generatedFolders.set(visualizer.address.folder, entry);
	}
	const generated = generatedFolders.get(draftFolder);

	const model: MediaPaneModel = {
		hasPatchedServer: true,
		hasCitpEndpoint: true,
		showSourceFilters: true,
		servers: [
			{
				id: "this-media-server",
				name: "This Media Server",
				statusLabel: outputs.failure ? "Offline" : "Online",
			},
		],
		selectedServerId: "this-media-server",
		selectedLayerId,
		preview: selectedOutput
			? {
					kind: "ready",
					imageSrc: api.outputPreviewUrl(
						selectedOutput.id,
						previewRevision,
						previewSize,
					),
					outputSize: previewSize,
				}
			: {
					kind: "unsupported",
					capability: "preview",
					detail: "No configured program output is available.",
				},
		layers: layers.map(({ output, layer }) => {
			const item = resolveAddress(
				catalog.data,
				layer.address.folder,
				layer.address.file,
			).item;
			const badge = sourceBadge(layer.sourceStatus);
			return {
				id: layerId(output.id, layer.index),
				number: String(layer.index + 1),
				name: output.name,
				status:
					badge.tone === "bad"
						? "failed"
						: badge.tone === "busy"
							? "stale"
							: "online",
				statusLabel: badge.label,
				thumbnailSrc:
					layer.address.folder || layer.address.file
						? api.outputLayerPreviewUrl(
								output.id,
								layer.index,
								previewRevision,
								previewSize,
							)
						: undefined,
				errorDetail: layer.sourceStatus.failure ?? undefined,
				liveSourceLabel: item?.name ?? "Nothing selected",
				opacityPercent: Math.round(layer.dimmer * 100),
				maskLabel:
					layer.mask.address.class === "blank"
						? "None"
						: `${layer.mask.address.folder}/${layer.mask.address.file}`,
				grayscalePercent: Math.round((1 - layer.grayscale) * 100),
				effectLabel:
					layer.effects
						.filter((effect) => effect.enabled && effect.effectType)
						.map((effect) => effect.label)
						.join(" · ") || "None",
			};
		}),
		browserMode,
		sourceFilter,
		maskBrowser: "supported",
		libraryFolders:
			sourceFilter === "media"
				? Array.from({ length: 199 }, (_, index) => {
						const number = index + 1;
						const entry = catalog.data?.folders.find(
							(candidate) => candidate.folder === number,
						);
						return {
							id: String(number),
							kind: "folder" as const,
							name: entry?.name || `Folder ${String(number).padStart(3, "0")}`,
							detail: `${entry?.items.length ?? 0} files`,
							disabled: !takeover,
						};
					})
				: Array.from(
						{
							length: sourceFilter === "text" ? 50 : 6,
						},
						(_, index) => {
							const number = (sourceFilter === "text" ? 200 : 250) + index;
							const entry = generatedFolders.get(number);
							return {
								id: String(number),
								kind: "folder" as const,
								name:
									entry?.name ??
									`${sourceFilter === "text" ? "Text" : "Visualizers"} ${number}`,
								detail: `${entry?.files.length ?? 0} sources`,
								disabled: !takeover,
							};
						},
					),
		libraryFiles:
			generated?.files ??
			(folder?.items ?? []).map((item) => ({
				id: String(item.file),
				kind: "file" as const,
				name: item.name,
				detail: `${item.width}×${item.height}`,
				thumbnailSrc: api.thumbnailUrl(draftFolder, item.file),
				disabled: !takeover,
			})),
		draftFolderId: String(draftFolder),
		draftFileId,
		liveSelection: {
			folderId: selected ? String(selected.layer.address.folder) : null,
			fileId: selected ? String(selected.layer.address.file) : null,
			maskFolderId: selected
				? String(selected.layer.mask.address.folder)
				: selectedOutput
					? String(selectedOutput.master.mask.folder)
					: null,
			maskFileId: selected
				? String(selected.layer.mask.address.file)
				: selectedOutput
					? String(selectedOutput.master.mask.file)
					: null,
		},
		draftSelection: {
			folderId: String(draftFolder),
			fileId: draftFileId,
			maskFolderId: browserMode === "mask" ? String(draftFolder) : null,
			maskFileId: browserMode === "mask" ? draftFileId : null,
		},
		liveSelectionLabel: selected
			? `${selected.layer.address.folder}/${selected.layer.address.file}`
			: "No layer",
		draftSelectionLabel: `${draftFolder}/${draftFileId ?? "Choose"}`,
		controlSections:
			selectedLayerId === "master" && selectedOutput
				? masterSections(selectedOutput, takeover)
				: selected
					? [
							{
								id: "playback",
								label: "Playback",
								controls: [
									{
										id: "play-mode",
										kind: "choice",
										label: "Play mode",
										value: String(selected.layer.playModeDmx),
										options: PLAY_MODES.map(([value, label]) => ({
											value: String(value),
											label,
										})),
										quickActions: [
											{ value: "216", label: "Stop" },
											{ value: "60", label: "Play" },
											{ value: "0", label: "Play looped" },
										],
										disabled: !takeover,
									},
									{
										id: "dimmer",
										kind: "value",
										label: "Dimmer",
										value: Math.round(selected.layer.dimmer * 100),
										minimum: 0,
										maximum: 100,
										display: `${Math.round(selected.layer.dimmer * 100)}%`,
										disabled: !takeover,
									},
									valueControl(
										"volume",
										"Volume",
										selected.layer.volume * 100,
										0,
										100,
										!takeover,
										"%",
									),
									{
										id: "speed",
										kind: "value",
										label: "Speed",
										value: selected.layer.speedMultiplierDmx,
										minimum: 0,
										maximum: 255,
										step: 1,
										disabled: !takeover,
										display: selected.layer.speedMultiplier,
									},
									valueControl(
										"bpm",
										"Playback BPM",
										selected.layer.playbackBpm ?? 0,
										0,
										255,
										!takeover,
										"",
										1,
									),
									valueControl(
										"blur",
										"Blur",
										selected.layer.blur * 100,
										0,
										100,
										!takeover,
										"%",
									),
									...playbackRangeControls(
										selected,
										!takeover,
										catalog.data,
									),
								],
							},
							frameSection(selected.layer, models.data ?? [], !takeover),
							{
								id: "colour",
								label: "Colour",
								controls: [
									{
										id: "tint",
										kind: "color",
										label: "Tint",
										value: tintHex(
											selected.layer.tintRed,
											selected.layer.tintGreen,
											selected.layer.tintBlue,
										),
										disabled: !takeover,
									},
									valueControl(
										"grayscale",
										"Grayscale",
										selected.layer.grayscale * 100,
										0,
										100,
										!takeover,
										"%",
									),
								],
							},
							{
								id: "mask-controls",
								label: "Mask",
								controls: [
									valueControl(
										"mask-position-x",
										"Mask position X",
										selected.layer.mask.positionX,
										-2,
										2,
										!takeover,
									),
									valueControl(
										"mask-position-y",
										"Mask position Y",
										selected.layer.mask.positionY,
										-2,
										2,
										!takeover,
									),
									valueControl(
										"mask-scale-x",
										"Mask scale X",
										selected.layer.mask.scaleX,
										0,
										2,
										!takeover,
									),
									valueControl(
										"mask-scale-y",
										"Mask scale Y",
										selected.layer.mask.scaleY,
										0,
										2,
										!takeover,
									),
									{
										id: "mask-invert",
										kind: "choice",
										label: "Invert",
										value: String(selected.layer.mask.invert),
										options: [
											{ value: "false", label: "Normal" },
											{ value: "true", label: "Invert" },
										],
										disabled: !takeover,
									},
									valueControl(
										"mask-opacity",
										"Mask opacity",
										selected.layer.mask.opacity * 100,
										0,
										100,
										!takeover,
										"%",
									),
								],
							},
							layerEffectsSection(
								effectBankSection(
									selected.layer.effectBanks,
									effects.data ?? [],
									!takeover,
								),
								displayedVisualizer
									? [
											...visualizerControls(displayedVisualizer, !takeover),
											...visualizerParameterControls(selected.layer, !takeover),
										]
									: [],
							),
							blendSection(selected.layer, !takeover),
						]
					: [],
		selectedControlSectionId,
		mainSectionId,
		rightPaneVisible,
	};

	const browse = (mode: MediaBrowserMode, item: MediaLibraryItem) => {
		if (!takeover) return;
		if (selectedLayerId === "master" && mode !== "mask") return;
		if (item.kind === "folder") {
			setDraftFolderId(item.id);
			setDraftFileId(null);
			return;
		}
		setDraftFileId(item.id);
		if (
			selectedLayerId === "master" &&
			selectedOutput &&
			takeover &&
			mode === "mask"
		)
			void control.updateMaster(selectedOutput, {
				maskFolder: draftFolder,
				maskFile: Number(item.id),
			});
		else if (selected && takeover)
			void control.update(selected.output, selected.layer.index, {
				...(mode === "mask"
					? { maskFolder: draftFolder, maskFile: Number(item.id) }
					: { folder: draftFolder, file: Number(item.id) }),
			});
	};

	return (
		<MediaPaneSurface
			model={model}
			title="Playback"
			info={titleInfo}
			onSelectServer={() => {}}
			onSelectLayer={(id) => {
				setSelectedLayerId(id);
				if (id === "master") {
					setSelectedControlSectionId("output");
					setMainSectionId("output");
				} else {
					setSelectedControlSectionId("playback");
					setMainSectionId("playback");
				}
				const next = layers.find(
					({ output, layer }) => layerId(output.id, layer.index) === id,
				);
				if (next) {
					selectOutput(next.output.id);
					setSourceFilter(sourceFilterForFolder(next.layer.address.folder));
					setDraftFolderId(String(next.layer.address.folder || 1));
					setDraftFileId(
						next.layer.address.file ? String(next.layer.address.file) : null,
					);
				}
			}}
			onSelectBrowserMode={(mode) => {
				setBrowserMode(mode);
				setMainSectionId(mode === "mask" ? "mask" : "content");
			}}
			onSelectSourceFilter={(filter) => {
				setSourceFilter(filter);
				setDraftFolderId(
					String(filter === "media" ? 1 : filter === "text" ? 200 : 250),
				);
				setDraftFileId(null);
			}}
			onBrowseItem={browse}
			onSelectControlSection={(id) => {
				setSelectedControlSectionId(id);
				setMainSectionId(id);
			}}
			onChangeControl={(id, value) => {
				if (!takeover || !selectedOutput) return;
				if (displayedVisualizer && selected && id.startsWith("visualizer-")) {
					void control.updateContinuous(
						selected.output,
						selected.layer.index,
						visualizerChange(id, value, displayedVisualizer.parameters),
					);
					return;
				}
				if (selectedLayerId === "master") {
					void control.updateMasterContinuous(
						selectedOutput,
						masterChange(id, value),
					);
					return;
				}
				if (selected)
					void control.updateContinuous(
						selected.output,
						selected.layer.index,
						layerChange(id, value),
					);
			}}
			onSetRightPaneVisible={setRightPaneVisible}
		/>
	);
}

function layerId(outputId: string, index: number) {
	return `${outputId}:${index}`;
}

function outputSourceFailures(outputs: OutputView[]) {
	return outputs
		.flatMap((output) =>
			output.layers.flatMap((layer) =>
				layer.sourceStatus.failure
					? [
							`${output.name} layer ${layer.index + 1}: ${layer.sourceStatus.failure}`,
						]
					: [],
			),
		)
		.join("\n");
}

function sourceFilterForFolder(folder: number): MediaSourceFilter {
	if (folder >= 250) return "visualizers";
	if (folder >= 200) return "text";
	return "media";
}

function previewAspectRatio(size?: { width: number; height: number }) {
	return size && size.height > 0 ? size.width / size.height : 16 / 9;
}

const PLAY_MODES: Array<[number, string]> = [
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
];

function tintHex(red: number, green: number, blue: number) {
	return `#${[red, green, blue]
		.map((value) =>
			Math.round(value * 255)
				.toString(16)
				.padStart(2, "0"),
		)
		.join("")}`;
}

function tintChange(value: string) {
	const raw = value.replace("#", "");
	return {
		tintRed: Number.parseInt(raw.slice(0, 2), 16) / 255,
		tintGreen: Number.parseInt(raw.slice(2, 4), 16) / 255,
		tintBlue: Number.parseInt(raw.slice(4, 6), 16) / 255,
	};
}

function layerChange(id: string, value: string | number): UpdateLayer {
	const number = Number(value);
	const dmx = layerDmxChange(id, value);
	if (dmx) return dmx;
	const effect = effectLayerChange(id, value);
	if (effect) return effect;
	switch (id) {
		case "play-mode":
			return { playModeDmx: number };
		case "dimmer":
			return { dimmer: number / 100 };
		case "volume":
			return { volume: number / 100 };
		case "speed":
			return { speedMultiplierDmx: number };
		case "bpm":
			return { playbackBpm: number };
		case "scale-x":
			return { scaleX: number };
		case "scale-y":
			return { scaleY: number };
		case "scaling-mode":
			return { scalingMode: String(value) };
		case "position-x":
			return { positionX: number };
		case "position-y":
			return { positionY: number };
		case "rotation":
			return { rotation: number };
		case "tint":
			return tintChange(String(value));
		case "grayscale":
			return { grayscale: number / 100 };
		case "blur":
			return { blur: number / 100 };
		case "mask-scale-x":
			return { maskScaleX: number };
		case "mask-scale-y":
			return { maskScaleY: number };
		case "mask-position-x":
			return { maskPositionX: number };
		case "mask-position-y":
			return { maskPositionY: number };
		case "mask-invert":
			return { maskInvert: value === "true" };
		case "mask-opacity":
			return { maskOpacity: number / 100 };
		default:
			return {};
	}
}

/**
 * A layer's Effects tab: a shown Visualizer's own configuration comes first, as its own
 * Visualizer group, followed by the two DMX effect banks. Other sources show only the banks.
 */
function layerEffectsSection(
	banks: MediaPaneModel["controlSections"][number],
	visualizer: MediaSecondaryControl[],
): MediaPaneModel["controlSections"][number] {
	return { ...banks, controls: [...visualizer, ...banks.controls] };
}

/** A shown Visualizer's live configuration, grouped under Visualizer on the Effects tab. */
function visualizerControls(
	visualizer: VisualizerView,
	disabled: boolean,
): MediaSecondaryControl[] {
	const controls: MediaSecondaryControl[] = [
		{
			id: "visualizer-reset",
			kind: "choice",
			label: visualizer.name,
			value: "current",
			options: [
				{ value: "current", label: "Current" },
				{ value: "reset", label: "Reset parameters" },
			],
			disabled,
		},
	];
	for (const parameter of visualizer.uses) {
		const number = VISUALIZER_NUMBERS[parameter];
		if (number) {
			const label =
				visualizer.typeId === 0 && parameter === "amount"
					? "Bloom"
					: number.label;
			const minimum =
				visualizer.typeId === 1 && parameter === "size"
					? 0.005
					: number.minimum;
			const maximum =
				visualizer.typeId === 1 && parameter === "size" ? 0.1 : number.maximum;
			controls.push(
				valueControl(
					`visualizer-${parameter}`,
					label,
					Number(visualizer.parameters[number.field]),
					minimum,
					maximum,
					disabled,
					"",
					number.step,
				),
			);
			continue;
		}
		const flag = VISUALIZER_FLAGS[parameter];
		if (flag) {
			controls.push({
				id: `visualizer-${parameter}`,
				kind: "choice" as const,
				label: flag.label,
				value: String(visualizer.parameters[flag.field]),
				options: [
					{ value: "true", label: "On" },
					{ value: "false", label: "Off" },
				],
				disabled,
			});
			continue;
		}
		if (parameter === "primary" || parameter === "secondary") {
			const prefix = parameter === "primary" ? "primary" : "secondary";
			controls.push({
				id: `visualizer-${parameter}`,
				kind: "color" as const,
				label: parameter === "primary" ? "Colour" : "Second colour",
				value: tintHex(
					visualizer.parameters[`${prefix}Red`],
					visualizer.parameters[`${prefix}Green`],
					visualizer.parameters[`${prefix}Blue`],
				),
				disabled,
			});
		}
	}
	return controls.map((control) => ({ ...control, group: VISUALIZER_GROUP }));
}

/** A layer's own visualizer tuning edit. It never addresses an effect slot. */
function visualizerChange(
	id: string,
	value: string | number,
	parameters: VisualizerParametersView,
): UpdateLayer {
	if (id === "visualizer-reset")
		return value === "reset" ? { resetVisualizerParameters: true } : {};
	return {
		visualizerParameters: changeVisualizerParameter(
			parameters,
			id.slice("visualizer-".length),
			value,
		),
	};
}

function changeVisualizerParameter(
	parameters: VisualizerParametersView,
	parameter: string,
	value: string | number,
): VisualizerParametersView {
	const number = VISUALIZER_NUMBERS[parameter];
	if (number) return { ...parameters, [number.field]: Number(value) };
	const flag = VISUALIZER_FLAGS[parameter];
	if (flag) return { ...parameters, [flag.field]: value === "true" };
	if (parameter === "primary" || parameter === "secondary") {
		const channels = tintChange(String(value));
		return {
			...parameters,
			[`${parameter}Red`]: channels.tintRed,
			[`${parameter}Green`]: channels.tintGreen,
			[`${parameter}Blue`]: channels.tintBlue,
		};
	}
	return parameters;
}

function masterChange(id: string, value: string | number): UpdateMaster {
	const number = Number(value);
	switch (id) {
		case "master-dimmer":
			return { dimmer: number / 100 };
		case "master-volume":
			return { volume: number / 100 };
		case "master-tint":
			return tintChange(String(value));
		case "media.master.effect.opacity_cycle":
			return { opacityCycleDmx: number };
		case "master-scale-x":
			return { scaleX: number };
		case "master-scale-y":
			return { scaleY: number };
		case "master-scaling-mode":
			return { scalingMode: String(value) };
		case "master-position-x":
			return { positionX: number };
		case "master-position-y":
			return { positionY: number };
		case "master-rotation":
			return { rotation: number };
		case "master-mask-position-x":
			return { maskPositionX: number };
		case "master-mask-position-y":
			return { maskPositionY: number };
		case "shaper-left":
			return { shaperLeft: number / 100 };
		case "shaper-right":
			return { shaperRight: number / 100 };
		case "shaper-top":
			return { shaperTop: number / 100 };
		case "shaper-bottom":
			return { shaperBottom: number / 100 };
		case "shaper-left-rotation":
			return { shaperLeftRotation: number };
		case "shaper-right-rotation":
			return { shaperRightRotation: number };
		case "shaper-top-rotation":
			return { shaperTopRotation: number };
		case "shaper-bottom-rotation":
			return { shaperBottomRotation: number };
		case "shaper-rotation":
			return { shaperRotation: number };
		default:
			return {};
	}
}

function masterSections(
	output: OutputView,
	takeover: boolean,
): MediaPaneModel["controlSections"] {
	return [
		masterOutputSection(output, takeover),
		masterEffectsSection(output, takeover),
		masterGeometrySection(output, takeover),
		masterMaskSection(output, takeover),
		masterShapersSection(output, takeover),
		// The master mirrors through negative scale; there is no Flip / mirror channel.
		masterColourSection(output, takeover),
	];
}

type MasterSection = MediaPaneModel["controlSections"][number];
function masterOutputSection(
	output: OutputView,
	takeover: boolean,
): MasterSection {
	return {
		id: "output",
		label: "Output",
		controls: [
			valueControl(
				"master-dimmer",
				"Dimmer",
				output.master.dimmer * 100,
				0,
				100,
				!takeover,
				"%",
			),
			valueControl(
				"master-volume",
				"Volume",
				output.master.volume * 100,
				0,
				100,
				!takeover,
				"%",
			),
		],
	};
}
function masterEffectsSection(
	output: OutputView,
	takeover: boolean,
): MasterSection {
	return {
		id: "effects",
		label: "Effects",
		controls: [
			{
				id: "media.master.effect.opacity_cycle",
				kind: "choice",
				label: "Multiplier / Divider",
				group: "Layer Opacity Cycle",
				value: String(output.master.opacityCycleDmx),
				options: [
					{ value: "0", label: "Off" },
					{ value: "1", label: "/16" },
					{ value: "32", label: "/8" },
					{ value: "64", label: "/4" },
					{ value: "96", label: "/2" },
					{ value: "128", label: "1x" },
					{ value: "160", label: "2x" },
					{ value: "192", label: "4x" },
					{ value: "224", label: "8x" },
					{ value: "240", label: "16x" },
				],
				disabled: !takeover,
			},
		],
	};
}
function masterGeometrySection(
	output: OutputView,
	takeover: boolean,
): MasterSection {
	return {
		id: "geometry",
		label: "Geometry",
		controls: [
			valueControl(
				"master-position-x",
				"Position X",
				output.master.positionX,
				-2,
				2,
				!takeover,
			),
			valueControl(
				"master-position-y",
				"Position Y",
				output.master.positionY,
				-2,
				2,
				!takeover,
			),
			valueControl(
				"master-scale-x",
				"Scale X",
				output.master.scaleX,
				-4,
				4,
				!takeover,
			),
			valueControl(
				"master-scale-y",
				"Scale Y",
				output.master.scaleY,
				-4,
				4,
				!takeover,
			),
			valueControl(
				"master-rotation",
				"Rotation",
				output.master.rotation,
				-180,
				180,
				!takeover,
				"°",
				1,
			),
			{
				id: "master-scaling-mode",
				kind: "choice",
				label: "Scale mode",
				value: output.master.scalingMode,
				options: [
					{ value: "fit", label: "Fit" },
					{ value: "fill", label: "Fill" },
					{ value: "original", label: "Native" },
					{ value: "stretch", label: "Stretch" },
				],
				disabled: !takeover,
			},
		],
	};
}
function masterMaskSection(
	output: OutputView,
	takeover: boolean,
): MasterSection {
	return {
		id: "mask-controls",
		label: "Mask position",
		controls: [
			valueControl(
				"master-mask-position-x",
				"Mask position X",
				output.master.maskPositionX,
				-2,
				2,
				!takeover,
			),
			valueControl(
				"master-mask-position-y",
				"Mask position Y",
				output.master.maskPositionY,
				-2,
				2,
				!takeover,
			),
		],
	};
}
function masterShapersSection(
	output: OutputView,
	takeover: boolean,
): MasterSection {
	return {
		id: "shapers",
		label: "Shapers",
		controls: [
			valueControl(
				"shaper-left",
				"Left",
				output.master.shaperLeft * 100,
				0,
				100,
				!takeover,
				"%",
			),
			valueControl(
				"shaper-right",
				"Right",
				output.master.shaperRight * 100,
				0,
				100,
				!takeover,
				"%",
			),
			valueControl(
				"shaper-top",
				"Top",
				output.master.shaperTop * 100,
				0,
				100,
				!takeover,
				"%",
			),
			valueControl(
				"shaper-bottom",
				"Bottom",
				output.master.shaperBottom * 100,
				0,
				100,
				!takeover,
				"%",
			),
			valueControl(
				"shaper-left-rotation",
				"Left rotation",
				output.master.shaperLeftRotation,
				-45,
				45,
				!takeover,
				"°",
				1,
			),
			valueControl(
				"shaper-right-rotation",
				"Right rotation",
				output.master.shaperRightRotation,
				-45,
				45,
				!takeover,
				"°",
				1,
			),
			valueControl(
				"shaper-top-rotation",
				"Top rotation",
				output.master.shaperTopRotation,
				-45,
				45,
				!takeover,
				"°",
				1,
			),
			valueControl(
				"shaper-bottom-rotation",
				"Bottom rotation",
				output.master.shaperBottomRotation,
				-45,
				45,
				!takeover,
				"°",
				1,
			),
			valueControl(
				"shaper-rotation",
				"Module rotation",
				output.master.shaperRotation,
				-180,
				180,
				!takeover,
				"°",
				1,
			),
		],
	};
}
function masterColourSection(
	output: OutputView,
	takeover: boolean,
): MasterSection {
	return {
		id: "colour",
		label: "Colour",
		controls: [
			{
				id: "master-tint",
				kind: "color",
				label: "Tint",
				value: tintHex(
					output.master.tintRed,
					output.master.tintGreen,
					output.master.tintBlue,
				),
				disabled: !takeover,
			},
		],
	};
}
