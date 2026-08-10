import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { MediaServerInspection } from "../api/client/mediaOutput";
import { useMediaServers } from "../features/mediaServers/MediaServersContext";
import type { ProgrammerFixtureValue } from "../features/programmerValues/contracts";
import {
	useProgrammerValuesActions,
	useProgrammerValuesView,
} from "../features/programmerValues/ProgrammerValuesView";
import { useProgrammingSelectionActions } from "../features/programmingInteraction/ProgrammingInteractionView";
import {
	type MediaBrowserMode,
	type MediaLibraryItem,
	type MediaPaneModel,
	MediaPaneSurface,
} from "./media/MediaPaneSurface";
import type { WindowProps } from "./windowTypes";

export interface PersistedMediaPaneState {
	serverId?: string;
	layerId?: string;
	browserMode?: MediaBrowserMode;
	mainSectionId?: string;
	rightPaneVisible?: boolean;
}

export interface MediaPaneWindowProps extends WindowProps {
	mediaPaneState?: PersistedMediaPaneState;
	onMediaPaneStateChange?: (state: PersistedMediaPaneState) => void;
}

const EMPTY_INSPECTION: MediaServerInspection = {
	library_revision: "",
	server: { name: "", layer_count: 0 },
	folders: [],
	files: [],
	preview_sources: [],
	layers: [],
	capabilities: { provider: "citp_msex", native_action: null, layers: [] },
};

export function MediaPaneWindow({
	active = true,
	compact = false,
	mediaPaneState,
	onMediaPaneStateChange,
}: MediaPaneWindowProps) {
	const media = useMediaServers();
	const inspectMediaServer = media?.inspectMediaServer;
	const applyMediaLibrarySelection = media?.applyMediaLibrarySelection;
	const refreshMediaPreview = media?.refreshMediaPreview;
	const refreshMediaThumbnails = media?.refreshMediaThumbnails;
	const loadMediaThumbnail = media?.mediaThumbnail;
	const selectionActions = useProgrammingSelectionActions(active);
	const valuesActions = useProgrammerValuesActions();
	const programmerValues = useProgrammerValuesView(active);
	const eligibleServers = media?.mediaServers ?? [];
	const [selectedServerId, setSelectedServerId] = useState(
		mediaPaneState?.serverId ?? eligibleServers[0]?.fixture_id ?? "",
	);
	const selectedServer = eligibleServers.find(
		(server) => server.fixture_id === selectedServerId,
	);
	const [selectedLayerId, setSelectedLayerId] = useState<string>(
		mediaPaneState?.layerId ??
			selectedServer?.layers[0]?.fixture_id ??
			"master",
	);
	const [browserMode, setBrowserMode] = useState<MediaBrowserMode>(
		mediaPaneState?.browserMode ?? "media",
	);
	const [mainSectionId, setMainSectionId] = useState(
		mediaPaneState?.mainSectionId ?? "content",
	);
	const [rightPaneVisible, setRightPaneVisible] = useState(
		mediaPaneState?.rightPaneVisible ?? false,
	);
	const [inspection, setInspection] =
		useState<MediaServerInspection>(EMPTY_INSPECTION);
	const [inspectionError, setInspectionError] = useState<string | null>(null);
	const [draftFolderId, setDraftFolderId] = useState("");
	const [draftFileId, setDraftFileId] = useState<string | null>(null);
	const initializedDraftScope = useRef<string | null>(null);
	const [thumbnailUrls, setThumbnailUrls] = useState<Record<string, string>>(
		{},
	);
	const thumbnailUrlsRef = useRef(thumbnailUrls);
	thumbnailUrlsRef.current = thumbnailUrls;

	const persist = useCallback(
		(next: Partial<PersistedMediaPaneState>) =>
			onMediaPaneStateChange?.({
				serverId: selectedServer?.fixture_id,
				layerId: selectedLayerId,
				browserMode,
				mainSectionId,
				rightPaneVisible,
				...next,
			}),
		[
			browserMode,
			mainSectionId,
			onMediaPaneStateChange,
			rightPaneVisible,
			selectedLayerId,
			selectedServer?.fixture_id,
		],
	);

	useEffect(() => {
		if (!selectedServerId && eligibleServers[0]) {
			setSelectedServerId(eligibleServers[0].fixture_id);
		}
	}, [eligibleServers, selectedServerId]);

	useEffect(() => {
		if (!active || !inspectMediaServer || !selectedServer) {
			setInspection(EMPTY_INSPECTION);
			setDraftFolderId("");
			setDraftFileId(null);
			initializedDraftScope.current = null;
			return;
		}
		let disposed = false;
		let running = false;
		const refresh = async () => {
			if (running) return;
			running = true;
			try {
				const next = await inspectMediaServer(selectedServer.fixture_id);
				if (!disposed) {
					setInspection(next);
					setInspectionError(null);
					const scope = `${selectedServer.fixture_id}:${selectedLayerId}`;
					if (initializedDraftScope.current !== scope) {
						const draft = mediaDraftForLayer(
							next,
							selectedServer.layers,
							selectedLayerId,
						);
						setDraftFolderId(draft?.folderId ?? "");
						setDraftFileId(draft?.fileId ?? null);
						initializedDraftScope.current = scope;
					}
				}
			} catch (cause) {
				if (!disposed) {
					setInspectionError(
						cause instanceof Error ? cause.message : String(cause),
					);
					setInspection(EMPTY_INSPECTION);
					setDraftFolderId("");
					setDraftFileId(null);
					initializedDraftScope.current = null;
				}
			} finally {
				running = false;
			}
		};
		void refresh();
		const timer = window.setInterval(() => void refresh(), 1_000);
		return () => {
			disposed = true;
			window.clearInterval(timer);
		};
	}, [active, inspectMediaServer, selectedLayerId, selectedServer]);

	const previewSources = inspection.preview_sources;
	useEffect(() => {
		if (
			!active ||
			!refreshMediaPreview ||
			!selectedServer ||
			previewSources.length === 0
		)
			return;
		let disposed = false;
		const refresh = () => {
			if (disposed) return;
			for (const source of previewSources) {
				void refreshMediaPreview(selectedServer.fixture_id, source.id);
			}
		};
		refresh();
		const timer = window.setInterval(refresh, 1_000);
		return () => {
			disposed = true;
			window.clearInterval(timer);
		};
	}, [active, previewSources, refreshMediaPreview, selectedServer]);

	const draftFolder = Number(draftFolderId);
	const visibleFiles = useMemo(
		() => inspection.files.filter((file) => file.folder_id === draftFolder),
		[draftFolder, inspection.files],
	);
	useEffect(() => {
		if (
			!active ||
			!refreshMediaThumbnails ||
			!loadMediaThumbnail ||
			!selectedServer ||
			visibleFiles.length === 0
		)
			return;
		let disposed = false;
		void (async () => {
			await refreshMediaThumbnails(
				selectedServer.fixture_id,
				draftFolder,
				visibleFiles.map((file) => file.id),
			);
			const entries = await Promise.all(
				visibleFiles.map(async (file) => {
					const blob = await loadMediaThumbnail(
						selectedServer.fixture_id,
						file.folder_id,
						file.id,
					);
					return [String(file.id), URL.createObjectURL(blob)] as const;
				}),
			);
			if (disposed) {
				for (const [, url] of entries) URL.revokeObjectURL(url);
				return;
			}
			setThumbnailUrls((current) => {
				for (const url of Object.values(current)) URL.revokeObjectURL(url);
				return Object.fromEntries(entries);
			});
		})().catch(() => undefined);
		return () => {
			disposed = true;
		};
	}, [
		active,
		draftFolder,
		loadMediaThumbnail,
		refreshMediaThumbnails,
		selectedServer,
		visibleFiles,
	]);

	useEffect(
		() => () => {
			for (const url of Object.values(thumbnailUrlsRef.current))
				URL.revokeObjectURL(url);
		},
		[],
	);

	const selectedLayer = selectedServer?.layers.find(
		(layer) => layer.fixture_id === selectedLayerId,
	);
	const selectedStatus = selectedLayer
		? inspection.layers.find(
				(layer) => layer.layer === selectedLayer.head_index,
			)
		: undefined;
	const selectedCapabilities = mediaCapabilitiesForLayer(
		inspection,
		selectedLayer?.head_index,
	);
	const liveProgrammer = programmerValues?.fixtureValues.filter(
		(value) => value.fixtureId === selectedLayerId,
	);
	const liveFolder =
		normalizedAttribute(liveProgrammer, "media.folder") ??
		selectedStatus?.folder;
	const liveFile =
		normalizedAttribute(liveProgrammer, "media.file") ?? selectedStatus?.file;
	const compositeSource = inspection.preview_sources.find(
		(source) => source.layer == null,
	);

	const chooseLayer = useCallback(
		(layerId: string) => {
			setSelectedLayerId(layerId);
			const scope = selectedServer
				? `${selectedServer.fixture_id}:${layerId}`
				: null;
			const draft = mediaDraftForLayer(
				inspection,
				selectedServer?.layers ?? [],
				layerId,
			);
			setDraftFolderId(draft?.folderId ?? "");
			setDraftFileId(draft?.fileId ?? null);
			initializedDraftScope.current = draft ? scope : null;
			persist({ layerId });
			const fixtureId =
				layerId === "master" ? selectedServer?.fixture_id : layerId;
			if (fixtureId)
				void selectionActions?.replace({ resolvedFixtures: [fixtureId] });
		},
		[inspection, persist, selectedServer, selectionActions],
	);

	const browse = useCallback(
		(mode: MediaBrowserMode, item: MediaLibraryItem) => {
			if (item.kind === "folder") {
				setDraftFolderId(item.id);
				setDraftFileId(null);
				return;
			}
			setDraftFileId(item.id);
			if (
				!applyMediaLibrarySelection ||
				!selectedServer ||
				!selectedLayer ||
				!inspection.library_revision ||
				!Number.isFinite(draftFolder)
			)
				return;
			void applyMediaLibrarySelection(selectedServer.fixture_id, {
				expected_library_revision: inspection.library_revision,
				layer_fixture_id: selectedLayer.fixture_id,
				kind: mode === "mask" ? "mask" : "content",
				folder: draftFolder,
				file: Number(item.id),
			}).catch((cause) =>
				setInspectionError(
					cause instanceof Error ? cause.message : String(cause),
				),
			);
		},
		[
			applyMediaLibrarySelection,
			draftFolder,
			inspection.library_revision,
			selectedLayer,
			selectedServer,
		],
	);

	const model = useMemo<MediaPaneModel>(() => {
		const offlineDetail =
			inspectionError ?? selectedServer?.status.last_error ?? "Server offline";
		return {
			servers: [
				...(selectedServerId && !selectedServer
					? [
							{
								id: selectedServerId,
								name: "Configured server unavailable",
								statusLabel: "Missing patch",
								disabled: true,
							},
						]
					: []),
				...eligibleServers.map((server) => ({
					id: server.fixture_id,
					name: server.name,
					statusLabel: server.status.online ? "Online" : "Offline",
				})),
			],
			selectedServerId,
			selectedLayerId,
			preview: !selectedServer
				? {
						kind: "missing_patch",
						detail: "Patch a CITP media master with logical layers.",
					}
				: inspectionError || !selectedServer.status.online
					? { kind: "offline", detail: offlineDetail }
					: compositeSource
						? {
								kind: "ready",
								imageSrc:
									media?.mediaPreviewUrls[
										`${selectedServer.fixture_id}:${compositeSource.id}`
									],
							}
						: {
								kind: "unsupported",
								capability: "preview",
								detail: "No composite preview source is advertised.",
							},
			layers: (selectedServer?.layers ?? []).map((head) => {
				const status = inspection.layers.find(
					(layer) => layer.layer === head.head_index,
				);
				const source = inspection.preview_sources.find(
					(candidate) => candidate.layer === status?.layer,
				);
				return {
					id: head.fixture_id,
					number: String(status?.layer ?? head.head_index),
					name: status?.name || `Layer ${head.head_index + 1}`,
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
					thumbnailSrc: source
						? media?.mediaPreviewUrls[
								`${selectedServer?.fixture_id}:${source.id}`
							]
						: undefined,
					liveSourceLabel: status
						? `Folder ${status.folder} · File ${status.file}`
						: undefined,
				};
			}),
			browserMode,
			maskBrowser: selectedCapabilities?.mask_library ? "supported" : "hidden",
			libraryFolders: inspection.folders.map((folder) => ({
				id: String(folder.id),
				kind: "folder",
				name: folder.name,
				detail: `${folder.element_count} files`,
			})),
			libraryFiles: visibleFiles.map((file) => ({
				id: String(file.id),
				kind: "file",
				name: file.name,
				detail: `${file.width}×${file.height}`,
				thumbnailSrc: thumbnailUrls[String(file.id)],
			})),
			draftFolderId,
			draftFileId,
			liveSelection: {
				folderId: liveFolder == null ? null : String(liveFolder),
				fileId: liveFile == null ? null : String(liveFile),
				maskFolderId: null,
				maskFileId: null,
			},
			draftSelection: {
				folderId: draftFolderId || null,
				fileId: draftFileId,
				maskFolderId: null,
				maskFileId: null,
			},
			liveSelectionLabel:
				liveFolder == null
					? "No live media"
					: `Folder ${liveFolder} / File ${liveFile ?? "None"}`,
			draftSelectionLabel: draftFolderId
				? `Folder ${draftFolderId} / File ${draftFileId ?? "Choose"}`
				: "Choose a folder",
			controlSections:
				selectedCapabilities?.secondary_controls.length && selectedLayer
					? [
							{
								id: "advertised",
								label: "Layer controls",
								capability: "supported",
								controls: selectedCapabilities.secondary_controls.map(
									(control) => ({
										id: control.attribute,
										label: control.attribute,
										kind: "value" as const,
										value:
											normalizedAttribute(liveProgrammer, control.attribute) ??
											0,
										minimum: 0,
										maximum: 255,
										step: 1,
									}),
								),
							},
						]
					: [],
			selectedControlSectionId: selectedCapabilities?.secondary_controls.length
				? "advertised"
				: "",
			mainSectionId,
			rightPaneVisible,
		};
	}, [
		browserMode,
		compositeSource,
		draftFileId,
		draftFolderId,
		eligibleServers,
		inspection,
		inspectionError,
		liveFile,
		liveFolder,
		liveProgrammer,
		mainSectionId,
		media?.mediaPreviewUrls,
		rightPaneVisible,
		selectedCapabilities,
		selectedLayer,
		selectedLayerId,
		selectedServer,
		selectedServerId,
		thumbnailUrls,
		visibleFiles,
	]);

	return (
		<MediaPaneSurface
			model={model}
			compact={compact}
			onSelectServer={(serverId) => {
				setSelectedServerId(serverId);
				setSelectedLayerId("master");
				setInspection(EMPTY_INSPECTION);
				setInspectionError(null);
				setDraftFolderId("");
				setDraftFileId(null);
				initializedDraftScope.current = null;
				persist({ serverId, layerId: "master" });
			}}
			onSelectLayer={chooseLayer}
			onSelectBrowserMode={(mode) => {
				setBrowserMode(mode);
				setMainSectionId(mode === "media" ? "content" : "mask");
				persist({
					browserMode: mode,
					mainSectionId: mode === "media" ? "content" : "mask",
				});
			}}
			onBrowseItem={browse}
			onSelectControlSection={(sectionId) => {
				setMainSectionId(sectionId);
				persist({ mainSectionId: sectionId });
			}}
			onChangeControl={(attribute, value) => {
				if (!selectedLayer || typeof value !== "number") return;
				void valuesActions?.setFixtureValue({
					requestId: crypto.randomUUID(),
					fixtureId: selectedLayer.fixture_id,
					attribute,
					value: {
						kind: "normalized",
						value: Math.max(0, Math.min(255, value)) / 255,
					},
					fade: false,
					fadeMillis: null,
					delayMillis: null,
				});
			}}
			onSetRightPaneVisible={(visible) => {
				setRightPaneVisible(visible);
				persist({ rightPaneVisible: visible });
			}}
		/>
	);
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

export function mediaFileMutations(
	fixtureId: string,
	folder: number,
	file: number,
) {
	return [
		mediaMutation(fixtureId, "media.folder", folder),
		mediaMutation(fixtureId, "media.file", file),
	];
}

export function mediaDraftForLayer(
	inspection: MediaServerInspection,
	layers: ReadonlyArray<{ fixture_id: string; head_index: number }>,
	layerId: string,
): { folderId: string; fileId: string } | null {
	const head = layers.find((candidate) => candidate.fixture_id === layerId);
	if (!head) return null;
	const status = inspection.layers.find(
		(candidate) => candidate.layer === head.head_index,
	);
	return status
		? { folderId: String(status.folder), fileId: String(status.file) }
		: null;
}

export function mediaCapabilitiesForLayer(
	inspection: MediaServerInspection,
	headIndex: number | undefined,
) {
	return headIndex == null
		? undefined
		: inspection.capabilities.layers.find(
				(capability) => capability.layer === headIndex,
			);
}

function mediaMutation(fixtureId: string, attribute: string, value: number) {
	return {
		action: "set_fixture" as const,
		fixtureId,
		attribute,
		value: {
			kind: "normalized" as const,
			value: Math.max(0, Math.min(255, value)) / 255,
		},
		timing: { fade: false, fadeMillis: null, delayMillis: null },
	};
}
