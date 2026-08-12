import { useEffect, useMemo, useState } from "react";
import { SwitchField } from "@tosklight/ui/controls";
import {
	type MediaBrowserMode,
	type MediaLibraryItem,
	type MediaPaneModel,
	MediaPaneSurface,
} from "../../../../light-desktop/src/windows/media/MediaPaneSurface";
import { resolveAddress } from "../../entities/catalog";
import { sourceBadge } from "../../entities/output";
import { api } from "../../shared/api/client";
import { useCatalog } from "../../shared/api/queries";
import {
	useLayerControl,
	useOutputsForControl,
} from "../../shared/api/layerControl";

const CATALOG_POLL_MS = 15_000;

/**
 * The Media Server's Media screen is the production CITP Media Pane surface.
 * Only the data adapter differs: this product already owns the server and can
 * project its HTTP output/catalog state directly into the pane's view model.
 */
export function MediaPanePage() {
	const outputs = useOutputsForControl();
	const catalog = useCatalog(CATALOG_POLL_MS);
	const control = useLayerControl(outputs.data);
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

	useEffect(() => {
		const first = layers[0];
		if (!selectedLayerId && first) {
			setSelectedLayerId(layerId(first.output.id, first.layer.index));
			setDraftFolderId(String(first.layer.address.folder || 1));
			setDraftFileId(
				first.layer.address.file ? String(first.layer.address.file) : null,
			);
		}
	}, [layers, selectedLayerId]);

	const selected =
		layers.find(
			({ output, layer }) =>
				layerId(output.id, layer.index) === selectedLayerId,
		) ?? layers[0];
	const takeover = (outputs.data ?? []).some(
		(output) => output.playbackTakeover,
	);
	const draftFolder = Number(draftFolderId || 1);
	const folder = catalog.data?.folders.find(
		(candidate) => candidate.folder === draftFolder,
	);
	const selectedItem = folder?.items.find(
		(item) => String(item.file) === draftFileId,
	);

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
				effectLabel: layer.playMode,
			};
		}),
		browserMode,
		maskBrowser: "hidden",
		libraryFolders: Array.from({ length: 199 }, (_, index) => {
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
		libraryFiles: (folder?.items ?? []).map((item) => ({
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
			maskFolderId: null,
			maskFileId: null,
		},
		draftSelection: {
			folderId: String(draftFolder),
			fileId: draftFileId,
			maskFolderId: null,
			maskFileId: null,
		},
		liveSelectionLabel: selected
			? `${selected.layer.address.folder}/${selected.layer.address.file}`
			: "No layer",
		draftSelectionLabel: `${draftFolder}/${draftFileId ?? "Choose"}`,
		controlSections: selected
			? [
					{
						id: "playback",
						label: "Playback",
						controls: [
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
							{
								id: "play-mode",
								kind: "readout",
								label: "Play mode",
								value: selected.layer.playMode,
							},
						],
					},
					{
						id: "frame",
						label: "Frame",
						controls: [
							{
								id: "scale",
								kind: "readout",
								label: "Scale",
								value: `${selected.layer.scaleX.toFixed(2)} × ${selected.layer.scaleY.toFixed(2)}`,
							},
							{
								id: "position",
								kind: "readout",
								label: "Position",
								value: `${selected.layer.positionX.toFixed(2)}, ${selected.layer.positionY.toFixed(2)}`,
							},
							{
								id: "rotation",
								kind: "readout",
								label: "Rotation",
								value: `${selected.layer.rotation.toFixed(1)}°`,
							},
						],
					},
				]
			: [],
		selectedControlSectionId,
		mainSectionId,
		rightPaneVisible,
	};

	const browse = (_mode: MediaBrowserMode, item: MediaLibraryItem) => {
		if (item.kind === "folder") {
			setDraftFolderId(item.id);
			setDraftFileId(null);
			return;
		}
		setDraftFileId(item.id);
		if (selected && takeover)
			void control.update(selected.output, selected.layer.index, {
				folder: draftFolder,
				file: Number(item.id),
			});
	};

	return (
		<MediaPaneSurface
			model={model}
			title="Playback"
			headerAction={
				<SwitchField
					className="media-playback-takeover"
					label="Take over playback"
					aria-label="Take over playback"
					offLabel="Take over playback"
					onLabel="Release"
					checked={takeover}
					onChange={async (event) => {
						await Promise.all(
							(outputs.data ?? []).map((output) =>
								api.setPlaybackTakeover(output.id, event.target.checked),
							),
						);
						void outputs.reload();
					}}
				/>
			}
			onSelectServer={() => {}}
			onSelectLayer={(id) => {
				setSelectedLayerId(id);
				const next = layers.find(
					({ output, layer }) => layerId(output.id, layer.index) === id,
				);
				if (next) {
					setDraftFolderId(String(next.layer.address.folder || 1));
					setDraftFileId(
						next.layer.address.file ? String(next.layer.address.file) : null,
					);
				}
			}}
			onSelectBrowserMode={setBrowserMode}
			onBrowseItem={browse}
			onSelectControlSection={(id) => {
				setSelectedControlSectionId(id);
				setMainSectionId(id);
			}}
			onChangeControl={(id, value) => {
				if (id === "dimmer" && selected && takeover)
					void control.update(selected.output, selected.layer.index, {
						dimmer: Number(value) / 100,
					});
			}}
			onSetRightPaneVisible={setRightPaneVisible}
		/>
	);
}

function layerId(outputId: string, index: number) {
	return `${outputId}:${index}`;
}
