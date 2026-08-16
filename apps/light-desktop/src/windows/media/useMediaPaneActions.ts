import type { Dispatch, SetStateAction } from "react";
import { useCallback } from "react";
import type { MediaServerInspection } from "../../api/client/mediaOutput";
import type { MediaServerFixture } from "../../api/types";
import type { MediaServersState } from "../../features/mediaServers/MediaServersContext";
import type { useProgrammerValuesActions } from "../../features/programmerValues/ProgrammerValuesView";
import type { useProgrammingSelectionActions } from "../../features/programmingInteraction/ProgrammingInteractionView";
import type { PersistedMediaPaneState } from "../MediaPaneWindow";
import { mediaLibraryMutations } from "../MediaPaneWindow.helpers";
import type {
	MediaBrowserMode,
	MediaLibraryItem,
	MediaSourceFilter,
} from "./mediaPaneModel";

interface MediaPaneActionsInput {
	servers: MediaServerFixture[];
	selectedServer: MediaServerFixture | undefined;
	selectedLayer: MediaServerFixture["layers"][number] | undefined;
	selectedLayerId: string;
	browserMode: MediaBrowserMode;
	sourceFilter: MediaSourceFilter;
	mainSectionId: string;
	rightPaneVisible: boolean;
	inspection: MediaServerInspection;
	draftFolder: number;
	applySelection: MediaServersState["applyMediaLibrarySelection"] | undefined;
	selectionActions: ReturnType<typeof useProgrammingSelectionActions>;
	valuesActions: ReturnType<typeof useProgrammerValuesActions>;
	setSelectedServerId: Dispatch<SetStateAction<string>>;
	setSelectedLayerId: Dispatch<SetStateAction<string>>;
	setBrowserMode: Dispatch<SetStateAction<MediaBrowserMode>>;
	setSourceFilter: Dispatch<SetStateAction<MediaSourceFilter>>;
	setMainSectionId: Dispatch<SetStateAction<string>>;
	setRightPaneVisible: Dispatch<SetStateAction<boolean>>;
	setDraftFolderId: Dispatch<SetStateAction<string>>;
	setDraftFileId: Dispatch<SetStateAction<string | null>>;
	setInspectionError: Dispatch<SetStateAction<string | null>>;
	initializeLayer(layerId: string): void;
	resetMediaData(): void;
	onPersist?: (state: PersistedMediaPaneState) => void;
}

export function useMediaPaneActions(input: MediaPaneActionsInput) {
	const persist = useCallback(
		(next: Partial<PersistedMediaPaneState>) =>
			input.onPersist?.({
				serverId: input.selectedServer?.fixture_id,
				layerId: input.selectedLayerId,
				browserMode: input.browserMode,
				sourceFilter: input.sourceFilter,
				mainSectionId: input.mainSectionId,
				rightPaneVisible: input.rightPaneVisible,
				...next,
			}),
		[input],
	);
	const onSelectLayer = useCallback(
		(layerId: string) => {
			input.setSelectedLayerId(layerId);
			input.initializeLayer(layerId);
			persist({ layerId });
			const fixtureId =
				layerId === "master" ? input.selectedServer?.fixture_id : layerId;
			if (fixtureId)
				void input.selectionActions?.replace({ resolvedFixtures: [fixtureId] });
		},
		[input, persist],
	);
	const onBrowseItem = useCallback(
		(mode: MediaBrowserMode, item: MediaLibraryItem) => {
			if (item.kind === "folder") {
				input.setDraftFolderId(item.id);
				input.setDraftFileId(null);
				return;
			}
			input.setDraftFileId(item.id);
			if (!input.selectedLayer || !Number.isFinite(input.draftFolder)) return;
			const file = Number(item.id);
			const advertised = input.inspection.files.some(
				(candidate) =>
					candidate.folder_id === input.draftFolder && candidate.id === file,
			);
			const capabilities = input.inspection.capabilities.layers.find(
				(candidate) => candidate.layer === input.selectedLayer?.head_index,
			);
			const libraryAdvertised =
				mode === "mask"
					? capabilities?.mask_library
					: capabilities?.content_library;
			const operation =
				input.applySelection &&
				input.selectedServer &&
				input.inspection.library_revision &&
				advertised &&
				libraryAdvertised
					? input.applySelection(input.selectedServer.fixture_id, {
							expected_library_revision: input.inspection.library_revision,
							layer_fixture_id: input.selectedLayer.fixture_id,
							kind: mode === "mask" ? "mask" : "content",
							folder: input.draftFolder,
							file,
						})
					: input.valuesActions?.batch({
							requestId: crypto.randomUUID(),
							mutations: mediaLibraryMutations(
								input.selectedLayer.fixture_id,
								mode,
								input.draftFolder,
								file,
							),
						});
			if (!operation) return;
			void operation.catch((cause) =>
				input.setInspectionError(
					cause instanceof Error ? cause.message : String(cause),
				),
			);
		},
		[input],
	);
	return {
		onSelectLayer,
		onBrowseItem,
		onSelectServer: (serverId: string) => {
			const server = input.servers.find(
				(candidate) => candidate.fixture_id === serverId,
			);
			const layerId = server?.layers[0]?.fixture_id ?? "master";
			input.setSelectedServerId(serverId);
			input.setSelectedLayerId(layerId);
			input.resetMediaData();
			persist({ serverId, layerId });
		},
		onSelectBrowserMode: (mode: MediaBrowserMode) => {
			input.setBrowserMode(mode);
			input.setMainSectionId(mode === "media" ? "content" : "mask");
			persist({
				browserMode: mode,
				mainSectionId: mode === "media" ? "content" : "mask",
			});
		},
		onSelectSourceFilter: (filter: MediaSourceFilter) => {
			input.setSourceFilter(filter);
			input.setDraftFolderId(
				String(filter === "media" ? 1 : filter === "text" ? 200 : 250),
			);
			input.setDraftFileId(null);
			persist({ sourceFilter: filter });
		},
		onSelectControlSection: (sectionId: string) => {
			input.setMainSectionId(sectionId);
			persist({ mainSectionId: sectionId });
		},
		onChangeControl: (attribute: string, value: string | number) => {
			if (!input.selectedLayer || typeof value !== "number") return;
			void input.valuesActions?.setFixtureValue({
				requestId: crypto.randomUUID(),
				fixtureId: input.selectedLayer.fixture_id,
				attribute,
				value: {
					kind: "normalized",
					value: Math.max(0, Math.min(255, value)) / 255,
				},
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			});
		},
		onSetRightPaneVisible: (visible: boolean) => {
			input.setRightPaneVisible(visible);
			persist({ rightPaneVisible: visible });
		},
	};
}
