import type { Meta, StoryObj } from "@storybook/react-vite";
import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import { useMemo, useState } from "react";
import {
	type DummyMediaFolder,
	type DummyMediaLayer,
	type DummyMediaPreviewState,
	type DummyMediaServer,
	dummyMaskFolders,
	dummyMediaCapabilityStates,
	dummyMediaFolders,
	dummyMediaServers,
} from "../../../ui-library/storybook/fixtures/media";
import {
	type MediaBrowserMode,
	type MediaControlSection,
	type MediaLibraryItem,
	type MediaPaneModel,
	MediaPaneSurface,
	type MediaPreviewState,
} from "./media/MediaPaneSurface";

const meta = {
	title: "ToskLight/Windows/Media Pane",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function DummyDataNotice() {
	return (
		<div
			className="media-pane-dummy-notice"
			data-media-source="deterministic-dummy"
			role="note"
		>
			<strong>Dummy data</strong> · Storybook-only preview. No Media Server,
			Programmer, or output is connected.
		</div>
	);
}

function previewState(
	state: DummyMediaPreviewState,
	imageSrc: string,
): MediaPreviewState {
	switch (state) {
		case "ready":
			return { kind: "ready", imageSrc, capturedAt: "21:16:32" };
		case "stale":
			return {
				kind: "stale",
				imageSrc,
				capturedAt: "21:16:14",
				detail: "Mock preview is 18 seconds old",
			};
		case "offline":
			return {
				kind: "offline",
				imageSrc,
				capturedAt: "21:15:48",
				detail: "No network connection is attempted by this story",
			};
		case "failed":
			return {
				kind: "failed_source",
				imageSrc,
				source: "Dummy source 3",
				detail: "Last mock frame retained",
			};
		case "missing-patch":
			return {
				kind: "missing_patch",
				detail: "Configured dummy master is no longer patched",
			};
		case "unsupported":
			return {
				kind: "unsupported",
				capability: "Program preview",
				detail: "The dummy provider does not advertise this capability",
			};
	}
}

function layerStatus(layer: DummyMediaLayer) {
	switch (layer.status) {
		case "ready":
			return "online" as const;
		case "stale":
			return "stale" as const;
		case "failed":
		case "offline":
		case "missing-patch":
			return "failed" as const;
		case "unsupported":
			return "unsupported" as const;
	}
}

function libraryItems(
	folders: DummyMediaFolder[],
	draftFolderId: string | null,
): MediaLibraryItem[] {
	if (!draftFolderId)
		return folders.map((folder) => ({
			id: folder.id,
			kind: "folder" as const,
			name: folder.name,
			detail: `${folder.assets.length} mock files`,
		}));
	const folder = folders.find((candidate) => candidate.id === draftFolderId);
	return (folder?.assets ?? []).map((asset) => ({
		id: asset.id,
		kind: "file" as const,
		name: asset.name,
		detail: asset.detail,
		thumbnailSrc: asset.thumbnail,
	}));
}

function folderName(folders: DummyMediaFolder[], id: string | null) {
	return folders.find((folder) => folder.id === id)?.name ?? "Library";
}

function assetName(folders: DummyMediaFolder[], id: string | null) {
	return folders
		.flatMap((folder) => folder.assets)
		.find((asset) => asset.id === id)?.name;
}

function StatefulMediaStory({
	servers = dummyMediaServers,
	compact = false,
	previewOverride,
}: {
	servers?: DummyMediaServer[];
	compact?: boolean;
	previewOverride?: MediaPreviewState;
}) {
	const [selectedServerId, setSelectedServerId] = useState(
		servers[0]?.id ?? "",
	);
	const server =
		servers.find((candidate) => candidate.id === selectedServerId) ??
		servers[0];
	const [selectedLayerId, setSelectedLayerId] = useState(
		server?.layers[0]?.id ?? "",
	);
	const layer =
		server?.layers.find((candidate) => candidate.id === selectedLayerId) ??
		server?.layers[0];
	const [browserMode, setBrowserMode] = useState<MediaBrowserMode>("media");
	const [draftFolderId, setDraftFolderId] = useState<string | null>(null);
	const [draftFileId, setDraftFileId] = useState<string | null>(null);
	const [selectedControlSectionId, setSelectedControlSectionId] =
		useState("playback");
	const [controlValues, setControlValues] = useState<
		Record<string, string | number>
	>({
		playMode: "loop",
		speed: 1,
		opacity: 82,
		rotation: 0,
		tint: "#7fd9ff",
	});
	const [lastInteraction, setLastInteraction] = useState("none");
	const folders =
		browserMode === "media" ? dummyMediaFolders : dummyMaskFolders;
	const liveFolderId =
		browserMode === "media" ? layer?.liveFolderId : layer?.maskFolderId;
	const liveFileId =
		browserMode === "media" ? layer?.liveFileId : layer?.maskFileId;
	const controls = useMemo<MediaControlSection[]>(
		() => [
			{
				id: "playback",
				label: "Playback",
				controls: [
					{
						id: "playMode",
						kind: "choice",
						label: "Play mode",
						value: String(controlValues.playMode),
						options: [
							{ value: "loop", label: "Loop" },
							{ value: "once", label: "Once" },
							{ value: "pause", label: "Pause" },
						],
					},
					{
						id: "speed",
						kind: "value",
						label: "Speed",
						value: Number(controlValues.speed),
						minimum: -2,
						maximum: 2,
						step: 0.05,
						display: `${Number(controlValues.speed).toFixed(2)}×`,
					},
				],
			},
			{
				id: "geometry",
				label: "Geometry",
				controls: [
					{
						id: "rotation",
						kind: "value",
						label: "Rotation",
						value: Number(controlValues.rotation),
						minimum: -180,
						maximum: 180,
						display: `${controlValues.rotation}°`,
					},
					{
						id: "opacity",
						kind: "value",
						label: "Opacity / Dimmer",
						value: Number(controlValues.opacity),
						minimum: 0,
						maximum: 100,
						display: `${controlValues.opacity}%`,
					},
					{
						id: "tint",
						kind: "color",
						label: "Tint",
						value: String(controlValues.tint),
					},
				],
			},
			{
				id: "audio",
				label: "Audio",
				capability: server?.supportsAudio ? "supported" : "unsupported",
				unsupportedDetail: "The selected dummy server has no audio output",
				controls: [
					{
						id: "audioLevel",
						kind: "value",
						label: "Audio level",
						value: 0,
						minimum: 0,
						maximum: 100,
						disabled: !server?.supportsAudio,
					},
				],
			},
			{
				id: "shapers",
				label: "Shapers",
				controls: [
					{
						id: "shaperReadout",
						kind: "readout",
						label: "Frame",
						value: "Top 0 · Right 0 · Bottom 0 · Left 0",
					},
				],
			},
			{
				id: "effects",
				label: "Effects",
				capability: server?.supportsEffects ? "supported" : "unsupported",
				unsupportedDetail: "No named effects advertised",
				controls: [],
			},
		],
		[controlValues, server?.supportsAudio, server?.supportsEffects],
	);

	if (!server || !layer)
		return <p>No deterministic Media fixture available.</p>;

	const draftFolderName = folderName(folders, draftFolderId);
	const draftAssetName = assetName(folders, draftFileId);
	const liveFolderName = folderName(folders, liveFolderId ?? null);
	const liveAssetName = assetName(folders, liveFileId ?? null) ?? "None";
	const model: MediaPaneModel = {
		servers: servers.map((candidate) => ({
			id: candidate.id,
			name: candidate.name,
			detail: candidate.address,
			statusLabel: candidate.statusDetail,
		})),
		selectedServerId: server.id,
		selectedLayerId: layer.id,
		preview:
			previewOverride ?? previewState(server.status, server.programPreview),
		layers: server.layers.map((candidate, index) => ({
			id: candidate.id,
			number: String(index + 1),
			name: candidate.name.replace(/^Layer \d+ · /, ""),
			status: layerStatus(candidate),
			statusLabel: candidate.statusDetail,
			thumbnailSrc: candidate.preview,
			liveSourceLabel: `${folderName(dummyMediaFolders, candidate.liveFolderId)} / ${assetName(dummyMediaFolders, candidate.liveFileId) ?? "None"}`,
		})),
		browserMode,
		maskBrowser: server.supportsMasks ? "supported" : "unsupported",
		libraryPath: [browserMode === "media" ? "Media" : "Mask", draftFolderName],
		libraryItems: libraryItems(folders, draftFolderId),
		liveSelection: {
			folderId: layer.liveFolderId,
			fileId: layer.liveFileId,
			maskFolderId: layer.maskFolderId,
			maskFileId: layer.maskFileId,
		},
		draftSelection: {
			folderId: browserMode === "media" ? draftFolderId : layer.liveFolderId,
			fileId: browserMode === "media" ? draftFileId : layer.liveFileId,
			maskFolderId: browserMode === "mask" ? draftFolderId : layer.maskFolderId,
			maskFileId: browserMode === "mask" ? draftFileId : layer.maskFileId,
		},
		liveSelectionLabel: `${liveFolderName} / ${liveAssetName}`,
		draftSelectionLabel: `${draftFolderName}${draftAssetName ? ` / ${draftAssetName}` : ""}`,
		controlSections: controls,
		selectedControlSectionId,
	};

	const selectServer = (id: string) => {
		const next = servers.find((candidate) => candidate.id === id);
		setSelectedServerId(id);
		setSelectedLayerId(next?.layers[0]?.id ?? "");
		setBrowserMode("media");
		setDraftFolderId(null);
		setDraftFileId(null);
		setLastInteraction(`Selected dummy server ${id}`);
	};
	const selectLayer = (id: string) => {
		setSelectedLayerId(id);
		setBrowserMode("media");
		setDraftFolderId(null);
		setDraftFileId(null);
		setLastInteraction(`Selected dummy layer ${id}`);
	};
	const selectBrowserMode = (mode: MediaBrowserMode) => {
		setBrowserMode(mode);
		setDraftFolderId(null);
		setDraftFileId(null);
		setLastInteraction(`Browsing dummy ${mode}`);
	};
	const browseItem = (mode: MediaBrowserMode, item: MediaLibraryItem) => {
		if (item.kind === "folder") {
			setDraftFolderId(item.id);
			setDraftFileId(null);
		} else {
			setDraftFileId(item.id);
		}
		setLastInteraction(`Browsed dummy ${mode} ${item.kind} ${item.id}`);
	};

	return (
		<div
			className="media-pane-story"
			data-media-story-boundary="local-state-only"
			style={{
				display: "flex",
				flexDirection: "column",
				height: compact ? 580 : 761,
				minWidth: 0,
				overflow: "hidden",
			}}
		>
			<DummyDataNotice />
			<MediaPaneSurface
				model={model}
				dummyDataBadge="DUMMY · LOCAL ONLY"
				compact={compact}
				onSelectServer={selectServer}
				onSelectLayer={selectLayer}
				onSelectBrowserMode={selectBrowserMode}
				onBrowseItem={browseItem}
				onSelectControlSection={(id) => {
					setSelectedControlSectionId(id);
					setLastInteraction(`Opened dummy controls ${id}`);
				}}
				onChangeControl={(id, value) => {
					setControlValues((current) => ({ ...current, [id]: value }));
					setLastInteraction(`Adjusted dummy ${id} to ${value}`);
				}}
				onOpenSettings={() =>
					setLastInteraction("Opened dummy Media pane settings")
				}
			/>
			<output aria-label="Dummy Media interaction" hidden>
				{lastInteraction}
			</output>
		</div>
	);
}

export const FullBuiltIn: Story = {
	render: () => (
		<section
			aria-label="Media built-in"
			data-light-surface="built-in"
			data-pane-type="media"
		>
			<StatefulMediaStory />
		</section>
	),
};

export const ConfigurableDesktopPane: Story = {
	render: () => (
		<div style={{ height: 761, padding: 12 }}>
			<GridDesktop id="media-review" name="Media Review">
				<PaneView
					pane={{
						id: "media-pane",
						title: "Media",
						type: "media",
						x: 2,
						y: 2,
						width: 20,
						height: 15,
					}}
					info={{
						primary: "Aurora Media · Layer 1",
						secondary: "Deterministic dummy data",
					}}
					settings
				>
					<StatefulMediaStory compact />
				</PaneView>
			</GridDesktop>
		</div>
	),
};

// Kept as a literal registration-contract story name used by the pane boundary test.
export const ConfigurablePane: Story = ConfigurableDesktopPane;

const previewReviewServer: DummyMediaServer = {
	...dummyMediaServers[0],
	id: "server-state-review",
	name: "Preview State Review",
	layers: dummyMediaCapabilityStates,
};

export const AllPreviewAndCapabilityStates: Story = {
	render: () => (
		<div
			style={{
				height: "100vh",
				display: "grid",
				gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
				gap: 12,
				overflow: "auto",
				padding: 12,
			}}
		>
			{(
				[
					"ready",
					"stale",
					"failed",
					"offline",
					"missing-patch",
					"unsupported",
				] as const
			).map((state) => (
				<StatefulMediaStory
					key={state}
					servers={[previewReviewServer]}
					compact
					previewOverride={previewState(
						state,
						previewReviewServer.programPreview,
					)}
				/>
			))}
		</div>
	),
};
