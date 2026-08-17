import type { Dispatch, SetStateAction } from "react";
import { useCallback } from "react";
import type { MediaServerInspection } from "../../api/client/mediaOutput";
import type { MediaServerFixture } from "../../api/types";
import type { MediaServersState } from "../../features/mediaServers/MediaServersContext";
import {
	type ProgrammerValuesMutationQueueController,
	programmerValuesMutationKey,
} from "../../features/programmerValues/useProgrammerValuesMutationQueue";
import type { useProgrammingSelectionActions } from "../../features/programmingInteraction/ProgrammingInteractionView";
import type { PersistedMediaPaneState } from "../MediaPaneWindow";
import {
	audioLibraryMutations,
	mediaLibraryMutations,
} from "../MediaPaneWindow.helpers";
import {
	mediaCmyFromRgb,
	mediaControlNormalizedValue,
} from "./mediaControlValue";
import type {
	MediaBrowserMode,
	MediaLibraryItem,
	MediaSourceFilter,
} from "./mediaPaneModel";

interface MediaPaneActionsInput {
	servers: MediaServerFixture[];
	selectedServer: MediaServerFixture | undefined;
	selectedLayer: MediaServerFixture["layers"][number] | undefined;
	selectedFixtureId: string | undefined;
	selectedLayerId: string;
	browserMode: MediaBrowserMode;
	browserModeByLayer: Record<string, MediaBrowserMode>;
	sourceFilter: MediaSourceFilter;
	selectedControlSectionId: string;
	mainSectionId: string;
	rightPaneVisible: boolean;
	inspection: MediaServerInspection;
	draftFolder: number;
	applySelection: MediaServersState["applyMediaLibrarySelection"] | undefined;
	selectionActions: ReturnType<typeof useProgrammingSelectionActions>;
	valuesQueue: ProgrammerValuesMutationQueueController;
	setSelectedServerId: Dispatch<SetStateAction<string>>;
	setSelectedLayerId: Dispatch<SetStateAction<string>>;
	setBrowserMode: Dispatch<SetStateAction<MediaBrowserMode>>;
	setBrowserModeByLayer: Dispatch<
		SetStateAction<Record<string, MediaBrowserMode>>
	>;
	setSourceFilter: Dispatch<SetStateAction<MediaSourceFilter>>;
	setSelectedControlSectionId: Dispatch<SetStateAction<string>>;
	setMainSectionId: Dispatch<SetStateAction<string>>;
	setRightPaneVisible: Dispatch<SetStateAction<boolean>>;
	setDraftFolderId: Dispatch<SetStateAction<string>>;
	setDraftFileId: Dispatch<SetStateAction<string | null>>;
	setInspectionError: Dispatch<SetStateAction<string | null>>;
	initializeLayer(layerId: string): void;
	resetMediaData(): void;
	onPersist?: (state: PersistedMediaPaneState) => void;
}

function usesLegacyAudioAddressing(
	layer: MediaServerFixture["layers"][number] | undefined,
) {
	const attributes = layer?.attributes;
	if (!attributes?.length) return false;
	return (
		!attributes.includes("media.folder") && attributes.includes("audio.folder")
	);
}

/** Everything a library click does, kept out of the hook so the hook stays a wiring layer. */
function browseMediaItem(
	input: MediaPaneActionsInput,
	mode: MediaBrowserMode,
	item: MediaLibraryItem,
) {
	if (item.kind === "folder") {
		input.setDraftFolderId(item.id);
		input.setDraftFileId(null);
		return;
	}
	input.setDraftFileId(item.id);
	if (!input.selectedFixtureId || !Number.isFinite(input.draftFolder))
		return;
	const file = Number(item.id);
	if (usesLegacyAudioAddressing(input.selectedLayer)) {
		// An Audio Player patched before TL-367 keeps its own folder/file attributes and
		// advertises no CITP library to select from.
		if (!input.selectedLayer) return;
		const operation = input.valuesQueue.submitBarrier(
			audioLibraryMutations(
				input.selectedLayer.fixture_id,
				input.draftFolder,
				file,
			),
		);
		void operation?.catch((cause) =>
			input.setInspectionError(
				cause instanceof Error ? cause.message : String(cause),
			),
		);
		return;
	}
	if (input.selectedLayerId === "master") {
		if (mode !== "mask") return;
		const operation = input.valuesQueue.submitBarrier([
			{
				action: "set_fixture",
				fixtureId: input.selectedFixtureId,
				attribute: "media.mask.file",
				value: { kind: "normalized", value: file / 255 },
				timing: { fade: false, fadeMillis: null, delayMillis: null },
			},
		]);
		void operation.catch((cause) =>
			input.setInspectionError(
				cause instanceof Error ? cause.message : String(cause),
			),
		);
		return;
	}
	if (!input.selectedLayer) return;
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
			: input.valuesQueue.submitBarrier(
					mediaLibraryMutations(
						input.selectedLayer.fixture_id,
						mode,
						input.draftFolder,
						file,
					),
				);
	if (!operation) return;
	void operation.catch((cause) =>
		input.setInspectionError(
			cause instanceof Error ? cause.message : String(cause),
		),
	);
}

export function useMediaPaneActions(input: MediaPaneActionsInput) {
	const persist = useCallback(
		(next: Partial<PersistedMediaPaneState>) =>
			input.onPersist?.({
				serverId: input.selectedServer?.fixture_id,
				layerId: input.selectedLayerId,
				browserMode: input.browserMode,
				browserModeByLayer: input.browserModeByLayer,
				sourceFilter: input.sourceFilter,
				controlSectionId: input.selectedControlSectionId,
				mainSectionId: input.mainSectionId,
				rightPaneVisible: input.rightPaneVisible,
				...next,
			}),
		[input],
	);
	const onSelectLayer = useCallback(
		(layerId: string) => {
			const rememberedModes =
				input.selectedLayerId === "master"
					? input.browserModeByLayer
					: {
							...input.browserModeByLayer,
							[input.selectedLayerId]: input.browserMode,
						};
			const nextMode =
				layerId === "master" ? "mask" : (rememberedModes[layerId] ?? "media");
			const nextMainSection = nextMode === "mask" ? "mask" : "content";
			input.setSelectedLayerId(layerId);
			input.initializeLayer(layerId);
			input.setBrowserModeByLayer(rememberedModes);
			input.setBrowserMode(nextMode);
			input.setMainSectionId(nextMainSection);
			persist({
				layerId,
				browserMode: nextMode,
				browserModeByLayer: rememberedModes,
				mainSectionId: nextMainSection,
			});
			const fixtureId =
				layerId === "master" ? input.selectedServer?.fixture_id : layerId;
			if (fixtureId)
				void input.selectionActions?.replace({ resolvedFixtures: [fixtureId] });
		},
		[input, persist],
	);
	const onBrowseItem = useCallback(
		(mode: MediaBrowserMode, item: MediaLibraryItem) =>
			browseMediaItem(input, mode, item),
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
			const nextMode =
				layerId === "master"
					? "mask"
					: (input.browserModeByLayer[layerId] ?? "media");
			input.setSelectedServerId(serverId);
			input.setSelectedLayerId(layerId);
			input.setBrowserMode(nextMode);
			input.setMainSectionId(nextMode === "mask" ? "mask" : "content");
			input.resetMediaData();
			persist({
				serverId,
				layerId,
				browserMode: nextMode,
				mainSectionId: nextMode === "mask" ? "mask" : "content",
			});
		},
		onSelectBrowserMode: (mode: MediaBrowserMode) => {
			if (input.selectedLayerId === "master" && mode === "media") return;
			const rememberedModes =
				input.selectedLayerId === "master"
					? input.browserModeByLayer
					: { ...input.browserModeByLayer, [input.selectedLayerId]: mode };
			input.setBrowserMode(mode);
			input.setBrowserModeByLayer(rememberedModes);
			input.setMainSectionId(mode === "media" ? "content" : "mask");
			persist({
				browserMode: mode,
				browserModeByLayer: rememberedModes,
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
			input.setSelectedControlSectionId(sectionId);
			input.setMainSectionId(sectionId);
			persist({ controlSectionId: sectionId, mainSectionId: sectionId });
		},
		onChangeControl: (attribute: string, value: string | number) => {
			if (!input.selectedFixtureId) return;
			const fixtureId = input.selectedFixtureId;
			if (attribute === "color.tint" && typeof value === "string") {
				const rgb = mediaRgbFromHex(value);
				if (!rgb) return;
				const mutations = ["red", "green", "blue"].map((component, index) => ({
					action: "set_fixture" as const,
					fixtureId,
					attribute: `color.${component}`,
					value: { kind: "normalized" as const, value: rgb[index] ?? 0 },
					timing: { fade: false, fadeMillis: null, delayMillis: null },
				}));
				void input.valuesQueue.submitLatest(
					programmerValuesMutationKey(mutations),
					mutations,
				);
				return;
			}
			if (attribute === "media.layer.tint" && typeof value === "string") {
				const cmy = mediaCmyFromRgb(value);
				if (!cmy) return;
				const mutations = ["cyan", "magenta", "yellow"].map(
					(component, index) => ({
						action: "set_fixture" as const,
						fixtureId,
						attribute: `media.layer.${component}`,
						value: { kind: "normalized" as const, value: cmy[index] ?? 0 },
						timing: { fade: false, fadeMillis: null, delayMillis: null },
					}),
				);
				void input.valuesQueue.submitLatest(
					programmerValuesMutationKey(mutations),
					mutations,
				);
				return;
			}
			const raw = Number(value);
			if (!Number.isFinite(raw)) return;
			const mutations = [
				{
					action: "set_fixture" as const,
					fixtureId,
					attribute,
					value: {
						kind: "normalized" as const,
						value: mediaControlNormalizedValue(
							attribute,
							raw,
							input.selectedLayerId === "master",
						),
					},
					timing: { fade: false, fadeMillis: null, delayMillis: null },
				},
			];
			void input.valuesQueue.submitLatest(
				programmerValuesMutationKey(mutations),
				mutations,
			);
		},
		onResetControl: (attribute: string) => {
			if (!input.selectedFixtureId) return;
			const fixtureId = input.selectedFixtureId;
			if (attribute === "color.tint") {
				void input.valuesQueue.submitBarrier(
					["red", "green", "blue"].map((component) => ({
						action: "release_fixture" as const,
						fixtureId,
						attribute: `color.${component}`,
					})),
				);
				return;
			}
			if (attribute === "media.layer.tint") {
				void input.valuesQueue.submitBarrier(
					["cyan", "magenta", "yellow"].map((component) => ({
						action: "release_fixture" as const,
						fixtureId,
						attribute: `media.layer.${component}`,
					})),
				);
				return;
			}
			void input.valuesQueue.submitBarrier([
				{ action: "release_fixture", fixtureId, attribute },
			]);
		},
		onSetRightPaneVisible: (visible: boolean) => {
			input.setRightPaneVisible(visible);
			persist({ rightPaneVisible: visible });
		},
	};
}

function mediaRgbFromHex(value: string): [number, number, number] | null {
	const match = /^#([0-9a-f]{6})$/iu.exec(value);
	if (!match) return null;
	return [0, 2, 4].map(
		(offset) => Number.parseInt(match[1].slice(offset, offset + 2), 16) / 255,
	) as [number, number, number];
}
