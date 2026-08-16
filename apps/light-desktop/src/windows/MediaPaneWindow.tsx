import { useEffect, useMemo, useState } from "react";
import { useMediaServers } from "../features/mediaServers/MediaServersContext";
import {
	useProgrammerValuesActions,
	useProgrammerValuesView,
} from "../features/programmerValues/ProgrammerValuesView";
import { useProgrammingSelectionActions } from "../features/programmingInteraction/ProgrammingInteractionView";
import { buildMediaPaneModel } from "./media/buildMediaPaneModel";
import { useApp } from "../state/AppContext";
import {
	type MediaBrowserMode,
	MediaPaneSurface,
	type MediaSourceFilter,
} from "./media/MediaPaneSurface";
import { NativeMediaControls } from "./media/NativeMediaControls";
import { OPEN_MEDIA_PATCH_ACTION } from "./media/openMediaPatch";
import { useMediaPaneActions } from "./media/useMediaPaneActions";
import { useMediaPaneData } from "./media/useMediaPaneData";

export {
	mediaCapabilitiesForLayer,
	mediaDraftForLayer,
	mediaFileMutations,
	mediaLibraryMutations,
} from "./MediaPaneWindow.helpers";

import type { WindowProps } from "./windowTypes";

export interface PersistedMediaPaneState {
	serverId?: string;
	layerId?: string;
	browserMode?: MediaBrowserMode;
	sourceFilter?: MediaSourceFilter;
	mainSectionId?: string;
	rightPaneVisible?: boolean;
}

export interface MediaPaneWindowProps extends WindowProps {
	mediaPaneState?: PersistedMediaPaneState;
	onMediaPaneStateChange?: (state: PersistedMediaPaneState) => void;
}

export function mediaRightPaneIsVisible(
	state: PersistedMediaPaneState | undefined,
): boolean {
	return state?.rightPaneVisible ?? true;
}

export function reconcileMediaPaneSelection(
	servers: Array<{ fixture_id: string; layers: Array<{ fixture_id: string }> }>,
	selectedServerId: string,
) {
	const server =
		servers.find((candidate) => candidate.fixture_id === selectedServerId) ??
		servers[0];
	return server
		? {
				serverId: server.fixture_id,
				layerId: server.layers[0]?.fixture_id ?? "master",
			}
		: { serverId: "", layerId: "master" };
}

export function MediaPaneWindow({
	active = true,
	compact = false,
	mediaPaneState,
	onMediaPaneStateChange,
}: MediaPaneWindowProps) {
	const { dispatch } = useApp();
	const media = useMediaServers();
	const inspectMediaServer = media?.inspectMediaServer;
	const nativeMedia = media?.nativeMedia;
	const updateNativeMediaText = media?.updateNativeMediaText;
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
	const [sourceFilter, setSourceFilter] = useState<MediaSourceFilter>(
		mediaPaneState?.sourceFilter ?? "media",
	);
	const [mainSectionId, setMainSectionId] = useState(
		mediaPaneState?.mainSectionId ?? "content",
	);
	const [rightPaneVisible, setRightPaneVisible] = useState(
		mediaRightPaneIsVisible(mediaPaneState),
	);
	const {
		inspection,
		inspectionError,
		setInspectionError,
		draftFolder,
		draftFolderId,
		setDraftFolderId,
		draftFileId,
		setDraftFileId,
		thumbnailUrls,
		reset: resetMediaData,
		initializeLayer,
	} = useMediaPaneData({
		active,
		server: selectedServer,
		layerId: selectedLayerId,
		inspect: inspectMediaServer,
		refreshPreview: refreshMediaPreview,
		refreshThumbnails: refreshMediaThumbnails,
		loadThumbnail: loadMediaThumbnail,
	});

	useEffect(() => {
		const nextSelection = reconcileMediaPaneSelection(
			eligibleServers,
			selectedServerId,
		);
		if (!nextSelection.serverId) {
			if (selectedServerId) {
				setSelectedServerId("");
				setSelectedLayerId("master");
				resetMediaData();
			}
			return;
		}
		if (nextSelection.serverId === selectedServerId) return;
		setSelectedServerId(nextSelection.serverId);
		setSelectedLayerId(nextSelection.layerId);
		resetMediaData();
		onMediaPaneStateChange?.({
			...mediaPaneState,
			serverId: nextSelection.serverId,
			layerId: nextSelection.layerId,
		});
	}, [
		eligibleServers,
		mediaPaneState,
		onMediaPaneStateChange,
		resetMediaData,
		selectedServerId,
	]);

	const selectedLayer = selectedServer?.layers.find(
		(layer) => layer.fixture_id === selectedLayerId,
	);
	const liveProgrammer = programmerValues?.fixtureValues.filter(
		(value) => value.fixtureId === selectedLayerId,
	);

	const actions = useMediaPaneActions({
		servers: eligibleServers,
		selectedServer,
		selectedLayer,
		selectedLayerId,
		browserMode,
		sourceFilter,
		mainSectionId,
		rightPaneVisible,
		inspection,
		draftFolder,
		applySelection: applyMediaLibrarySelection,
		selectionActions,
		valuesActions,
		setSelectedServerId,
		setSelectedLayerId,
		setBrowserMode,
		setSourceFilter,
		setMainSectionId,
		setRightPaneVisible,
		setDraftFolderId,
		setDraftFileId,
		setInspectionError,
		initializeLayer,
		resetMediaData,
		onPersist: onMediaPaneStateChange,
	});

	const model = useMemo(
		() =>
			buildMediaPaneModel({
				inspection,
				inspectionError,
				servers: eligibleServers,
				selectedServer,
				selectedServerId,
				selectedLayerId,
				browserMode,
				sourceFilter,
				mainSectionId,
				rightPaneVisible,
				draftFolderId,
				draftFileId,
				thumbnailUrls,
				previewUrls: media?.mediaPreviewUrls ?? {},
				liveProgrammer,
				nativeControls:
					selectedServer?.native_action &&
					nativeMedia &&
					updateNativeMediaText ? (
						<NativeMediaControls
							fixtureId={selectedServer.fixture_id}
							load={nativeMedia}
							updateText={updateNativeMediaText}
						/>
					) : undefined,
			}),
		[
			browserMode,
			sourceFilter,
			draftFileId,
			draftFolderId,
			eligibleServers,
			inspection,
			inspectionError,
			liveProgrammer,
			nativeMedia,
			mainSectionId,
			media?.mediaPreviewUrls,
			rightPaneVisible,
			selectedLayerId,
			selectedServer,
			selectedServerId,
			thumbnailUrls,
			updateNativeMediaText,
		],
	);

	return (
		<MediaPaneSurface
			model={model}
			compact={compact}
			onOpenPatch={() => dispatch(OPEN_MEDIA_PATCH_ACTION)}
			{...actions}
		/>
	);
}
