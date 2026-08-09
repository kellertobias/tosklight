import type { Meta, StoryObj } from "@storybook/react-vite";
import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import { useEffect, useMemo, useState } from "react";
import { CommandSectionFixture } from "../../../ui-library/storybook/fixtures/controlSection";
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
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import { AppShellView } from "../components/shell/AppShell";
import { LeftDock } from "../components/shell/LeftDock";
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

function folderItems(folders: DummyMediaFolder[]): MediaLibraryItem[] {
	return folders.map((folder) => ({
		id: folder.id,
		kind: "folder" as const,
		name: folder.name,
		detail: `${folder.assets.length} mock files`,
	}));
}

function fileItems(
	folders: DummyMediaFolder[],
	draftFolderId: string,
): MediaLibraryItem[] {
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

interface MediaEncoderSelection {
	mediaFolderId: string;
	mediaFileId: string;
	maskFolderId: string;
	maskFileId: string;
}

function StatefulMediaStory({
	servers = dummyMediaServers,
	compact = false,
	embedded = false,
	showNotice = true,
	previewOverride,
	encoderSelection,
}: {
	servers?: DummyMediaServer[];
	compact?: boolean;
	embedded?: boolean;
	showNotice?: boolean;
	previewOverride?: MediaPreviewState;
	encoderSelection?: MediaEncoderSelection;
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
	const [draftFolderId, setDraftFolderId] = useState(
		server?.layers[0]?.liveFolderId ?? dummyMediaFolders[0]?.id ?? "",
	);
	const [draftFileId, setDraftFileId] = useState<string | null>(
		server?.layers[0]?.liveFileId ?? null,
	);
	const [selectedControlSectionId, setSelectedControlSectionId] =
		useState("playback");
	const [mainSectionId, setMainSectionId] = useState("content");
	const [rightPaneVisible, setRightPaneVisible] = useState(true);
	const [controlValues, setControlValues] = useState<
		Record<string, string | number>
	>({
		playMode: "loop",
		speed: 1,
		opacity: 82,
		volume: 68,
		positionX: 0,
		positionY: 0,
		positionZ: 0,
		rotationX: 0,
		rotationY: 0,
		rotationZ: 0,
		cropTop: 0,
		cropRight: 0,
		cropBottom: 0,
		cropLeft: 0,
		keystone: 0,
		tint: "#7fd9ff",
		grayscale: 0,
		effectMode: "none",
		effectAmount: 50,
		effectSpeed: 1,
	});
	const [lastInteraction, setLastInteraction] = useState("none");
	useEffect(() => {
		if (!encoderSelection) return;
		if (browserMode === "media") {
			setDraftFolderId(encoderSelection.mediaFolderId);
			setDraftFileId(encoderSelection.mediaFileId);
		} else {
			setDraftFolderId(encoderSelection.maskFolderId);
			setDraftFileId(encoderSelection.maskFileId);
		}
	}, [browserMode, encoderSelection]);
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
							{ value: "loop", label: "Play Loop" },
							{ value: "once", label: "Play Once" },
							{ value: "bounce", label: "Play Bounce" },
							{ value: "stop", label: "Stop" },
							{ value: "reverse-once", label: "Reverse Once" },
							{ value: "reverse", label: "Play Reverse" },
						],
					},
					{
						id: "opacity",
						kind: "value",
						label: "Opacity",
						value: Number(controlValues.opacity),
						minimum: 0,
						maximum: 100,
						display: `${controlValues.opacity}%`,
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
					{
						id: "volume",
						kind: "value",
						label: "Volume",
						value: Number(controlValues.volume),
						minimum: 0,
						maximum: 100,
						display: `${controlValues.volume}%`,
						disabled: !server?.supportsAudio,
					},
					{
						id: "grayscale",
						kind: "value",
						label: "Grayscale",
						value: Number(controlValues.grayscale),
						minimum: 0,
						maximum: 100,
						display: `${controlValues.grayscale}%`,
					},
					{
						id: "tint",
						kind: "color",
						label: "Color",
						group: "Color",
						value: String(controlValues.tint),
					},
				],
			},
			{
				id: "position",
				label: "Position",
				controls: [
					...["X", "Y", "Z"].map((axis) => ({
						id: `position${axis}`,
						kind: "value" as const,
						label: `${axis} position`,
						group: "Position",
						value: Number(controlValues[`position${axis}`]),
						minimum: -100,
						maximum: 100,
						display: Number(controlValues[`position${axis}`]).toFixed(2),
					})),
					...["X", "Y", "Z"].map((axis) => ({
						id: `rotation${axis}`,
						kind: "value" as const,
						label: `${axis} rotation`,
						group: "Rotation",
						value: Number(controlValues[`rotation${axis}`]),
						minimum: -180,
						maximum: 180,
						display: `${controlValues[`rotation${axis}`]}°`,
					})),
				],
			},
			{
				id: "frame",
				label: "Frame",
				controls: [
					{
						id: "keystone",
						kind: "value",
						label: "Keystone",
						value: Number(controlValues.keystone),
						minimum: -100,
						maximum: 100,
						display: `${controlValues.keystone}%`,
					},
					...["Top", "Right", "Bottom", "Left"].map((edge) => ({
						id: `crop${edge}`,
						kind: "value" as const,
						label: `Crop ${edge.toLowerCase()}`,
						value: Number(controlValues[`crop${edge}`]),
						minimum: 0,
						maximum: 100,
						display: `${controlValues[`crop${edge}`]}%`,
					})),
				],
			},
			{
				id: "effects",
				label: "Effects",
				capability: server?.supportsEffects ? "supported" : "unsupported",
				unsupportedDetail: server?.supportsEffects
					? undefined
					: "This media server does not advertise layer effects",
				controls: [
					{
						id: "effectMode",
						kind: "choice",
						label: "Effect",
						value: String(controlValues.effectMode),
						options: [
							{ value: "none", label: "None" },
							{ value: "glow", label: "Glow" },
							{ value: "blur", label: "Blur" },
							{ value: "pixelate", label: "Pixelate" },
						],
					},
					{
						id: "effectAmount",
						kind: "value",
						label: "Amount",
						value: Number(controlValues.effectAmount),
						minimum: 0,
						maximum: 100,
						display: `${controlValues.effectAmount}%`,
					},
					{
						id: "effectSpeed",
						kind: "value",
						label: "Effect speed",
						value: Number(controlValues.effectSpeed),
						minimum: -2,
						maximum: 2,
						step: 0.05,
						display: `${Number(controlValues.effectSpeed).toFixed(2)}×`,
					},
				],
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
	const displayLayers: DummyMediaLayer[] = [
		...server.layers.slice(0, 8),
		...Array.from(
			{ length: Math.max(0, 8 - server.layers.length) },
			(_, index) => ({
				id: `empty-layer-${server.id}-${server.layers.length + index + 1}`,
				name: `Layer ${server.layers.length + index + 1} · Empty`,
				status: "unsupported" as const,
				statusDetail: "No source assigned",
				preview: "",
				liveFolderId: "",
				liveFileId: "",
				maskFolderId: null,
				maskFileId: null,
			}),
		),
	];
	const model: MediaPaneModel = {
		servers: servers.map((candidate) => ({
			id: candidate.id,
			name: candidate.name,
			detail: candidate.address,
			statusLabel: candidate.statusDetail,
		})),
		selectedServerId: server.id,
		selectedLayerId,
		preview:
			previewOverride ?? previewState(server.status, server.programPreview),
		layers: displayLayers.map((candidate, index) => ({
			id: candidate.id,
			number: String(index + 1),
			name: candidate.name.replace(/^Layer \d+ · /, ""),
			status: layerStatus(candidate),
			statusLabel: candidate.statusDetail,
			thumbnailSrc: candidate.preview,
			liveSourceLabel: candidate.liveFolderId
				? `${folderName(dummyMediaFolders, candidate.liveFolderId)} / ${assetName(dummyMediaFolders, candidate.liveFileId) ?? "None"}`
				: "None / None",
			opacityPercent: [100, 72, 88, 64, 54, 92, 78, 46][index] ?? 100,
			maskLabel: candidate.maskFolderId
				? `${folderName(dummyMaskFolders, candidate.maskFolderId)} / ${assetName(dummyMaskFolders, candidate.maskFileId) ?? "None"}`
				: "None / None",
			colorValue: index === 0 ? "#3ca7ff" : "#ffffff",
			grayscalePercent: index === 0 ? 0 : 100,
			effectLabel: index === 1 ? "Glow" : "None",
		})),
		browserMode,
		maskBrowser: server.supportsMasks ? "supported" : "unsupported",
		libraryFolders: folderItems(folders),
		libraryFiles: fileItems(folders, draftFolderId),
		draftFolderId,
		draftFileId,
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
		mainSectionId,
		rightPaneVisible,
	};

	const selectServer = (id: string) => {
		const next = servers.find((candidate) => candidate.id === id);
		setSelectedServerId(id);
		setSelectedLayerId(next?.layers[0]?.id ?? "");
		setBrowserMode("media");
		setMainSectionId("content");
		setDraftFolderId(
			next?.layers[0]?.liveFolderId ?? dummyMediaFolders[0]?.id ?? "",
		);
		setDraftFileId(next?.layers[0]?.liveFileId ?? null);
		setLastInteraction(`Selected dummy server ${id}`);
	};
	const selectLayer = (id: string) => {
		if (id === "master") {
			setSelectedLayerId(id);
			setSelectedControlSectionId("frame");
			setMainSectionId("frame");
			setLastInteraction("Selected dummy master output");
			return;
		}
		const nextLayer =
			server.layers.find((candidate) => candidate.id === id) ??
			server.layers[0];
		setSelectedLayerId(id);
		setBrowserMode("media");
		setMainSectionId("content");
		setDraftFolderId(nextLayer?.liveFolderId ?? dummyMediaFolders[0]?.id ?? "");
		setDraftFileId(nextLayer?.liveFileId ?? null);
		setLastInteraction(`Selected dummy layer ${id}`);
	};
	const selectBrowserMode = (mode: MediaBrowserMode) => {
		const nextFolders = mode === "media" ? dummyMediaFolders : dummyMaskFolders;
		const nextFolderId =
			mode === "media" ? layer.liveFolderId : layer.maskFolderId;
		const nextFileId = mode === "media" ? layer.liveFileId : layer.maskFileId;
		setBrowserMode(mode);
		setMainSectionId(mode === "media" ? "content" : "mask");
		setDraftFolderId(nextFolderId ?? nextFolders[0]?.id ?? "");
		setDraftFileId(nextFileId ?? nextFolders[0]?.assets[0]?.id ?? null);
		setLastInteraction(`Browsing dummy ${mode}`);
	};
	const browseItem = (mode: MediaBrowserMode, item: MediaLibraryItem) => {
		// Production integration contract: cache server contents; a pool folder only
		// moves this browsing draft. Selecting a file must apply its folder + file
		// atomically (and the same rule applies to mask folder + mask file).
		if (item.kind === "folder") {
			setDraftFolderId(item.id);
			setDraftFileId(
				folders.find((folder) => folder.id === item.id)?.assets[0]?.id ?? null,
			);
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
				height: embedded ? "100%" : compact ? 580 : "100vh",
				minWidth: 0,
				overflow: "hidden",
			}}
		>
			{showNotice && <DummyDataNotice />}
			<MediaPaneSurface
				model={model}
				compact={compact}
				onSelectServer={selectServer}
				onSelectLayer={selectLayer}
				onSelectBrowserMode={selectBrowserMode}
				onBrowseItem={browseItem}
				onSelectControlSection={(id) => {
					setSelectedControlSectionId(id);
					setMainSectionId(id);
					setLastInteraction(`Opened dummy controls ${id}`);
				}}
				onChangeControl={(id, value) => {
					setControlValues((current) => ({ ...current, [id]: value }));
					setLastInteraction(`Adjusted dummy ${id} to ${value}`);
				}}
				onSetRightPaneVisible={(visible) => {
					setRightPaneVisible(visible);
					if (!visible) setMainSectionId("content");
					setLastInteraction(`Right pane ${visible ? "visible" : "hidden"}`);
				}}
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
					maximized
					showHeader={false}
					pane={{
						id: "media-pane",
						title: "Media",
						type: "media",
						x: 2,
						y: 2,
						width: 20,
						height: 15,
					}}
				>
					<StatefulMediaStory embedded showNotice={false} />
				</PaneView>
			</GridDesktop>
		</div>
	),
};

// Kept as a literal registration-contract story name used by the pane boundary test.
export const ConfigurablePane: Story = ConfigurableDesktopPane;

function selectByNormalized<T>(
	items: readonly T[],
	value: number,
): T | undefined {
	if (items.length === 0) return undefined;
	const index = Math.round(
		Math.max(0, Math.min(1, value)) * (items.length - 1),
	);
	return items[index];
}

function FullDeskMediaStory() {
	const initialMediaFolder = dummyMediaFolders[0];
	const initialMaskFolder = dummyMaskFolders[0];
	const [encoderSelection, setEncoderSelection] =
		useState<MediaEncoderSelection>({
			mediaFolderId: initialMediaFolder?.id ?? "",
			mediaFileId: initialMediaFolder?.assets[0]?.id ?? "",
			maskFolderId: initialMaskFolder?.id ?? "",
			maskFileId: initialMaskFolder?.assets[0]?.id ?? "",
		});
	const handleProgrammerValue = (attribute: string, value: number) => {
		setEncoderSelection((current) => {
			if (attribute === "media.folder") {
				const folder = selectByNormalized(dummyMediaFolders, value);
				return folder
					? {
							...current,
							mediaFolderId: folder.id,
							mediaFileId: folder.assets[0]?.id ?? "",
						}
					: current;
			}
			if (attribute === "media.file") {
				const folder = dummyMediaFolders.find(
					(candidate) => candidate.id === current.mediaFolderId,
				);
				const file = selectByNormalized(folder?.assets ?? [], value);
				return file ? { ...current, mediaFileId: file.id } : current;
			}
			if (attribute === "media.mask.folder") {
				const folder = selectByNormalized(dummyMaskFolders, value);
				return folder
					? {
							...current,
							maskFolderId: folder.id,
							maskFileId: folder.assets[0]?.id ?? "",
						}
					: current;
			}
			if (attribute === "media.mask.file") {
				const folder = dummyMaskFolders.find(
					(candidate) => candidate.id === current.maskFolderId,
				);
				const file = selectByNormalized(folder?.assets ?? [], value);
				return file ? { ...current, maskFileId: file.id } : current;
			}
			return current;
		});
	};

	return (
		<div style={{ height: "max(860px, calc(100vh - 48px))", minHeight: 860 }}>
			<ApplicationStateHarness>
				<AppShellView
					dock={
						<LeftDock
							presentation={{
								showIdentity: "Media UI Review",
								showIndicator: {
									label: "Offline mock",
									detail: "No Media Server is connected",
									className: "show-status-warning",
									connected: false,
								},
								clock: <span>12:00</span>,
							}}
						/>
					}
					workspace={
						<GridDesktop id="media-full-desk" name="Media Review">
							<PaneView
								maximized
								showHeader={false}
								pane={{
									id: "media-pane-full-desk",
									title: "Media",
									type: "media",
									x: 1,
									y: 1,
									width: 24,
									height: 18,
								}}
							>
								<StatefulMediaStory
									embedded
									showNotice={false}
									encoderSelection={encoderSelection}
								/>
							</PaneView>
						</GridDesktop>
					}
					control={
						<CommandSectionFixture
							inheritAppState
							initialMode="programmer"
							initialProgrammerFamily="Media"
							onProgrammerValue={handleProgrammerValue}
						/>
					}
				/>
			</ApplicationStateHarness>
		</div>
	);
}

export const FullDeskPreview: Story = {
	render: () => <FullDeskMediaStory />,
};

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
