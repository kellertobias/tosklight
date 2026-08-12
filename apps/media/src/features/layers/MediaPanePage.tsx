import { SwitchField } from "@tosklight/ui/controls";
import { useEffect, useMemo, useState } from "react";
import {
	type MediaBrowserMode,
	type MediaLibraryItem,
	type MediaPaneModel,
	MediaPaneSurface,
} from "../../../../light-desktop/src/windows/media/MediaPaneSurface";
import { resolveAddress } from "../../entities/catalog";
import { sourceBadge } from "../../entities/output";
import { api } from "../../shared/api/client";
import type {
	OutputView,
	UpdateLayer,
	UpdateMaster,
} from "../../shared/api/generated/media-wire";
import {
	useLayerControl,
	useOutputsForControl,
} from "../../shared/api/layerControl";
import { useCatalog, useText, useVisualizers } from "../../shared/api/queries";

const CATALOG_POLL_MS = 15_000;

/**
 * The Media Server's Media screen is the production CITP Media Pane surface.
 * Only the data adapter differs: this product already owns the server and can
 * project its HTTP output/catalog state directly into the pane's view model.
 */
export function MediaPanePage() {
	const outputs = useOutputsForControl();
	const catalog = useCatalog(CATALOG_POLL_MS);
	const text = useText();
	const visualizers = useVisualizers();
	const control = useLayerControl();
	const layers = useMemo(
		() =>
			(outputs.data ?? []).flatMap((output) =>
				output.layers.map((layer) => ({ output, layer })),
			),
		[outputs.data],
	);
	const [selectedLayerId, setSelectedLayerId] = useState("");
	const [browserMode, setBrowserMode] = useState<MediaBrowserMode>("media");
	const [draftFolderId, setDraftFolderId] = useState("");
	const [draftFileId, setDraftFileId] = useState<string | null>(null);
	const [mainSectionId, setMainSectionId] = useState("content");
	const [selectedControlSectionId, setSelectedControlSectionId] =
		useState("playback");
	const [rightPaneVisible, setRightPaneVisible] = useState(false);
	const [selectedOutputId, setSelectedOutputId] = useState("");
	const [takeoverError, setTakeoverError] = useState("");
	const [takeoverBusy, setTakeoverBusy] = useState(false);

	useEffect(() => {
		const first = layers[0];
		if (!selectedLayerId && first) {
			setSelectedLayerId(layerId(first.output.id, first.layer.index));
			setSelectedOutputId(first.output.id);
			setDraftFolderId(String(first.layer.address.folder || 1));
			setDraftFileId(
				first.layer.address.file ? String(first.layer.address.file) : null,
			);
		}
	}, [layers, selectedLayerId]);

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
	const draftFolder = Number(draftFolderId || 1);
	const folder = catalog.data?.folders.find(
		(candidate) => candidate.folder === draftFolder,
	);
	const selectedItem = folder?.items.find(
		(item) => String(item.file) === draftFileId,
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
		});
		generatedFolders.set(visualizer.address.folder, entry);
	}
	const generated = generatedFolders.get(draftFolder);

	const model: MediaPaneModel = {
		servers: [
			{
				id: "this-media-server",
				name: "This Media Server",
				statusLabel: outputs.failure ? "Offline" : "Online",
			},
		],
		selectedServerId: "this-media-server",
		selectedLayerId,
		preview: selectedItem
			? {
					kind: "ready",
					imageSrc: api.thumbnailUrl(draftFolder, selectedItem.file),
				}
			: {
					kind: "unsupported",
					capability: "preview",
					detail: "Choose media to preview it.",
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
				thumbnailSrc: item
					? api.thumbnailUrl(layer.address.folder, layer.address.file)
					: undefined,
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
		maskBrowser: "supported",
		libraryFolders: [
			...Array.from({ length: 199 }, (_, index) => {
				const number = index + 1;
				const entry = catalog.data?.folders.find(
					(candidate) => candidate.folder === number,
				);
				return {
					id: String(number),
					kind: "folder" as const,
					name: entry?.name || `Folder ${String(number).padStart(3, "0")}`,
					detail: `${entry?.items.length ?? 0} files`,
				};
			}),
			...Array.from(generatedFolders, ([number, entry]) => ({
				id: String(number),
				kind: "folder" as const,
				name: entry.name,
				detail: `${entry.files.length} sources`,
			})),
		],
		libraryFiles:
			generated?.files ??
			(folder?.items ?? []).map((item) => ({
				id: String(item.file),
				kind: "file" as const,
				name: item.name,
				detail: `${item.width}×${item.height}`,
				thumbnailSrc: api.thumbnailUrl(draftFolder, item.file),
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
									),
								],
							},
							{
								id: "frame",
								label: "Frame",
								controls: [
									valueControl(
										"scale-x",
										"Scale X",
										selected.layer.scaleX,
										0,
										10,
										!takeover,
									),
									valueControl(
										"scale-y",
										"Scale Y",
										selected.layer.scaleY,
										0,
										10,
										!takeover,
									),
									{
										id: "scaling-mode",
										kind: "choice",
										label: "Scaling mode",
										value: selected.layer.scalingMode,
										options: ["fit", "fill", "original", "stretch"].map(
											(value) => ({ value, label: value }),
										),
										disabled: !takeover,
									},
									valueControl(
										"position-x",
										"Position X",
										selected.layer.positionX,
										-2,
										2,
										!takeover,
									),
									valueControl(
										"position-y",
										"Position Y",
										selected.layer.positionY,
										-2,
										2,
										!takeover,
									),
									valueControl(
										"rotation",
										"Rotation",
										selected.layer.rotation,
										-360,
										360,
										!takeover,
										"°",
									),
								],
							},
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
							{
								...effectSection(selected.layer.effects, !takeover),
							},
						]
					: [],
		selectedControlSectionId,
		mainSectionId,
		rightPaneVisible,
	};

	const browse = (mode: MediaBrowserMode, item: MediaLibraryItem) => {
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
			headerAction={
				<>
					<SwitchField
						className="media-playback-takeover"
						label="Take over playback"
						aria-label="Take over playback"
						offLabel="Release"
						onLabel="Take over playback"
						checked={takeover}
						disabled={!selectedOutput || takeoverBusy}
						onChange={async (event) => {
							if (!selectedOutput) return;
							setTakeoverBusy(true);
							setTakeoverError("");
							try {
								await control.setTakeover(selectedOutput, event.target.checked);
							} catch (error) {
								setTakeoverError(
									error instanceof Error ? error.message : String(error),
								);
							} finally {
								setTakeoverBusy(false);
							}
						}}
					/>
					{takeoverError ? <span role="alert">{takeoverError}</span> : null}
					{control.refusal ? (
						<span role="alert">{control.refusal.message}</span>
					) : null}
				</>
			}
			onSelectServer={() => {}}
			onSelectLayer={(id) => {
				setSelectedLayerId(id);
				if (id === "master") {
					setSelectedControlSectionId("output");
					setMainSectionId("output");
				}
				const next = layers.find(
					({ output, layer }) => layerId(output.id, layer.index) === id,
				);
				if (next) {
					setSelectedOutputId(next.output.id);
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
			onBrowseItem={browse}
			onSelectControlSection={(id) => {
				setSelectedControlSectionId(id);
				setMainSectionId(id);
			}}
			onChangeControl={(id, value) => {
				if (!takeover || !selectedOutput) return;
				if (selectedLayerId === "master") {
					void control.updateMaster(selectedOutput, masterChange(id, value));
					return;
				}
				if (selected)
					void control.update(
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

function valueControl(
	id: string,
	label: string,
	value: number,
	minimum: number,
	maximum: number,
	disabled: boolean,
	suffix = "",
) {
	return {
		id,
		kind: "value" as const,
		label,
		value,
		minimum,
		maximum,
		disabled,
		display: `${Number(value.toFixed(2))}${suffix}`,
	};
}

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
	const effect = /^effect-(\d+)-(type|enabled|mix|tv-curvature|distortion|image-grain|glitching)$/.exec(
		id,
	);
	if (effect) {
		const effectSlot = Number(effect[1]);
		switch (effect[2]) {
			case "type":
				return { effectSlot, effectType: String(value) };
			case "enabled":
				return { effectSlot, effectEnabled: value === "true" };
			case "mix":
				return { effectSlot, effectMix: number / 100 };
			case "tv-curvature":
				return { effectSlot, tvCurvature: number / 100 };
			case "distortion":
				return { effectSlot, effectDistortion: number / 100 };
			case "image-grain":
				return { effectSlot, imageGrain: number / 100 };
			case "glitching":
				return { effectSlot, effectGlitching: number / 100 };
		}
	}
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
		case "mask-scale-x":
			return { maskScaleX: number };
		case "mask-scale-y":
			return { maskScaleY: number };
		case "mask-invert":
			return { maskInvert: value === "true" };
		case "mask-opacity":
			return { maskOpacity: number / 100 };
		default:
			return {};
	}
}

function effectControls(
	effects: OutputView["layers"][number]["effects"],
	disabled: boolean,
) {
	return effects.flatMap((effect) => {
		const prefix = `effect-${effect.index}`;
		const slot = `Slot ${effect.index + 1}`;
		const controls = [
			{
				id: `${prefix}-type`,
				kind: "choice" as const,
				label: `${slot} effect`,
				value: effect.effectType ?? "none",
				options: [
					{ value: "none", label: "None" },
					{ value: "analog-tv", label: "Analog TV" },
				],
				disabled,
			},
		];
		if (effect.effectType !== "analog-tv") return controls;
		return [
			...controls,
			{
				id: `${prefix}-enabled`,
				kind: "choice" as const,
				label: `${slot} state`,
				value: String(effect.enabled),
				options: [
					{ value: "true", label: "Enabled" },
					{ value: "false", label: "Bypassed" },
				],
				disabled,
			},
			valueControl(
				`${prefix}-mix`,
				`${slot} mix`,
				effect.mix * 100,
				0,
				100,
				disabled,
				"%",
			),
			...effect.parameters.map((parameter) =>
				valueControl(
					`${prefix}-${parameter.id}`,
					`${slot} · ${parameter.label}`,
					parameter.value * 100,
					0,
					100,
					disabled,
					"%",
				),
			),
		];
	});
}

function effectSection(
	effects: OutputView["layers"][number]["effects"],
	disabled: boolean,
): MediaPaneModel["controlSections"][number] {
	const unsupported = effects.find((effect) => !effect.supported);
	return {
		id: "effects",
		label: "Effects",
		capability: unsupported ? "unsupported" : undefined,
		unsupportedDetail: unsupported?.capabilityDetail ?? undefined,
		controls: unsupported ? [] : effectControls(effects, disabled),
	};
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
		case "flip-mirror":
			return { flipMirror: String(value) };
		default:
			return {};
	}
}

function masterSections(
	output: OutputView,
	takeover: boolean,
): MediaPaneModel["controlSections"] {
	return [
		{
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
		},
		{
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
				{
					id: "flip-mirror",
					kind: "choice",
					label: "Flip / mirror",
					value: output.master.flipMirror,
					options: ["none", "horizontal", "vertical", "both"].map((value) => ({
						value,
						label: value,
					})),
					disabled: !takeover,
				},
			],
		},
	];
}
