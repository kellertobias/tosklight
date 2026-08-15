import { useEffect, useMemo, useState } from "react";
import { useMediaServers } from "../features/mediaServers/MediaServersContext";
import {
	useProgrammerValuesActions,
	useProgrammerValuesView,
} from "../features/programmerValues/ProgrammerValuesView";
import { useProgrammingSelectionActions } from "../features/programmingInteraction/ProgrammingInteractionView";
import { useApp } from "../state/AppContext";
import { buildMediaPaneModel } from "./media/buildMediaPaneModel";
import {
	type MediaBrowserMode,
	MediaPaneSurface,
} from "./media/MediaPaneSurface";
import { useMediaPaneActions } from "./media/useMediaPaneActions";
import { useMediaPaneData } from "./media/useMediaPaneData";
import { NativeMediaControls } from "./media/NativeMediaControls";
import { OPEN_MEDIA_PATCH_ACTION } from "./media/openMediaPatch";

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
	mainSectionId?: string;
	rightPaneVisible?: boolean;
}

export interface MediaPaneWindowProps extends WindowProps {
	mediaPaneState?: PersistedMediaPaneState;
	onMediaPaneStateChange?: (state: PersistedMediaPaneState) => void;
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
	const [mainSectionId, setMainSectionId] = useState(
		mediaPaneState?.mainSectionId ?? "content",
	);
	const [rightPaneVisible, setRightPaneVisible] = useState(
		mediaPaneState?.rightPaneVisible ?? false,
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
		if (!selectedServerId && eligibleServers[0]) {
			setSelectedServerId(eligibleServers[0].fixture_id);
		}
	}, [eligibleServers, selectedServerId]);

	const selectedLayer = selectedServer?.layers.find(
		(layer) => layer.fixture_id === selectedLayerId,
	);
	const liveProgrammer = programmerValues?.fixtureValues.filter(
		(value) => value.fixtureId === selectedLayerId,
	);

	const actions = useMediaPaneActions({
		selectedServer,
		selectedLayer,
		selectedLayerId,
		browserMode,
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
				mainSectionId,
				rightPaneVisible,
				draftFolderId,
				draftFileId,
				thumbnailUrls,
				previewUrls: media?.mediaPreviewUrls ?? {},
				liveProgrammer,
				nativeControls:
					selectedServer?.native_action && nativeMedia && updateNativeMediaText ? (
						<NativeMediaControls
							fixtureId={selectedServer.fixture_id}
							load={nativeMedia}
							updateText={updateNativeMediaText}
						/>
					) : undefined,
			}),
		[
			browserMode,
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
