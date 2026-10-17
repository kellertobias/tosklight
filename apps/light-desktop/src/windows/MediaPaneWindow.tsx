import { useEffect, useState } from "react";
import type { MediaServerInspection } from "../api/client/mediaOutput";
import type { MediaServerFixture } from "../api/types";
import { useMediaServers } from "../features/mediaServers/MediaServersContext";
import { useProgrammerValuesMutationQueue } from "../features/programmerValues/useProgrammerValuesMutationQueue";
import { useProgrammingSelectionActions } from "../features/programmingInteraction/ProgrammingInteractionView";
import { useVisualizationRuntimeView } from "../features/visualizationRuntime/VisualizationRuntimeView";
import { useApp } from "../state/AppContext";
import { buildMediaPaneModel } from "./media/buildMediaPaneModel";
import {
	type MediaBrowserMode,
	MediaPaneSurface,
	type MediaSourceFilter,
} from "./media/MediaPaneSurface";
import { useNativeMediaEffects } from "./media/NativeMediaControls";
import { OPEN_MEDIA_PATCH_ACTION } from "./media/openMediaPatch";
import { useMediaPaneActions } from "./media/useMediaPaneActions";
import { useMediaPaneData } from "./media/useMediaPaneData";
import { useMediaProgrammerValues } from "./media/useMediaProgrammerValues";

export {
	audioLibraryMutations,
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
	browserModeByLayer?: Record<string, MediaBrowserMode>;
	sourceFilter?: MediaSourceFilter;
	controlSectionId?: string;
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
	const updateNativeMediaEffect = media?.updateNativeMediaEffect;
	const applyMediaLibrarySelection = media?.applyMediaLibrarySelection;
	const refreshMediaPreview = media?.refreshMediaPreview;
	const refreshMediaThumbnails = media?.refreshMediaThumbnails;
	const selectionActions = useProgrammingSelectionActions(active);
	const valuesQueue = useProgrammerValuesMutationQueue(active);
	const eligibleServers = media?.mediaServers ?? [];
	const [selectedServerId, setSelectedServerId] = useState(
		mediaPaneState?.serverId ?? eligibleServers[0]?.fixture_id ?? "",
	);
	const liveOutput = useMediaLiveOutput(active && eligibleServers.length > 0);
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
	const [browserModeByLayer, setBrowserModeByLayer] = useState<
		Record<string, MediaBrowserMode>
	>(mediaPaneState?.browserModeByLayer ?? {});
	const [sourceFilter, setSourceFilter] = useState<MediaSourceFilter>(
		mediaPaneState?.sourceFilter ?? "media",
	);
	const [selectedControlSectionId, setSelectedControlSectionId] = useState(
		mediaPaneState?.controlSectionId ?? "playback",
	);
	const [mainSectionId, setMainSectionId] = useState(
		mediaPaneState?.mainSectionId ?? "content",
	);
	const [rightPaneVisible, setRightPaneVisible] = useState(
		mediaRightPaneIsVisible(mediaPaneState),
	);
	const {
		endpointKey,
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
		loadThumbnail: media?.mediaThumbnail,
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

	const { selectedLayer, nativeLayer, selectedFixtureId } = mediaLayerSelection(
		selectedServer,
		selectedLayerId,
	);
	const nativeEffects = useNativeMediaEffects({
		active: Boolean(
			active &&
				selectedServer?.native_action &&
				nativeMedia &&
				updateNativeMediaEffect &&
				nativeLayer != null &&
				nativeLayer >= 0,
		),
		fixtureId: selectedServer?.fixture_id,
		layer: nativeLayer != null && nativeLayer >= 0 ? nativeLayer : undefined,
		sourceKey: liveSourceKey(inspection, nativeLayer),
		endpointKey,
		load: nativeMedia ?? unavailableNativeMedia,
		update: updateNativeMediaEffect ?? unavailableNativeMediaEffect,
	});
	const liveProgrammer = useMediaProgrammerValues(active, valuesQueue.route, {
		fixtureId: selectedFixtureId,
		parentFixtureId: selectedServerId,
		sourceKey: `${selectedServerId}:${endpointKey}`,
		output: liveOutput,
	});

	const actions = useMediaPaneActions({
		servers: eligibleServers,
		selectedServer,
		selectedLayer,
		selectedFixtureId,
		selectedLayerId,
		browserMode,
		browserModeByLayer,
		sourceFilter,
		selectedControlSectionId,
		mainSectionId,
		rightPaneVisible,
		inspection,
		draftFolder,
		applySelection: applyMediaLibrarySelection,
		selectionActions,
		valuesQueue,
		liveProgrammer,
		setSelectedServerId,
		setSelectedLayerId,
		setBrowserMode,
		setBrowserModeByLayer,
		setSourceFilter,
		setSelectedControlSectionId,
		setMainSectionId,
		setRightPaneVisible,
		setDraftFolderId,
		setDraftFileId,
		setInspectionError,
		initializeLayer,
		resetMediaData,
		onPersist: onMediaPaneStateChange,
	});

	const model = buildMediaPaneModel({
		inspection,
		inspectionError,
		servers: eligibleServers,
		selectedServer,
		selectedServerId,
		selectedLayerId,
		browserMode,
		sourceFilter,
		selectedControlSectionId,
		mainSectionId,
		rightPaneVisible,
		draftFolderId,
		draftFileId,
		thumbnailUrls,
		previewUrls: media?.mediaPreviewUrls ?? {},
		liveProgrammer,
		liveOutput,
		controlValues: liveProgrammer ?? [],
		...nativeEffects.modelInput,
	});

	return (
		<MediaPaneSurface
			model={model}
			compact={compact}
			onOpenPatch={() => dispatch(OPEN_MEDIA_PATCH_ACTION)}
			{...actions}
			onChangeControl={(id, value) => {
				if (id.startsWith("effect-")) nativeEffects.change(id, value);
				else actions.onChangeControl(id, value);
			}}
		/>
	);
}

/** Resolve the pane's logical fixture and corresponding native layer together. */
function mediaLayerSelection(
	server: MediaServerFixture | undefined,
	layerId: string,
) {
	const nativeLayer = server?.layers.findIndex(
		(layer) => layer.fixture_id === layerId,
	);
	const selectedLayer =
		nativeLayer != null && nativeLayer >= 0
			? server?.layers[nativeLayer]
			: undefined;
	return {
		selectedLayer,
		nativeLayer,
		selectedFixtureId:
			layerId === "master" ? server?.fixture_id : selectedLayer?.fixture_id,
	};
}

/** What a layer shows, as the polled server status reports it. */
function liveSourceKey(
	inspection: MediaServerInspection,
	layer: number | undefined,
) {
	const status = inspection.layers.find(
		(candidate) => candidate.layer === layer,
	);
	return status ? `${status.folder}:${status.file}` : "";
}

function unavailableNativeMedia(): Promise<never> {
	return Promise.reject(new Error("Native Media controls are unavailable"));
}

function unavailableNativeMediaEffect(): Promise<never> {
	return Promise.reject(
		new Error("Native Media effect controls are unavailable"),
	);
}

/** Normal resolved desk output only; an errored retained snapshot is not a live readout. */
function useMediaLiveOutput(enabled: boolean) {
	const output = useVisualizationRuntimeView({
		lane: "normal",
		enabled,
		intervalMillis: 250,
		deliveryIntervalMillis: 250,
		consumerId: "media-layer-readouts",
	});
	return enabled && output.ready && !output.error ? output.snapshot : null;
}
