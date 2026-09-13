import type {
	MediaServerInspection,
	NativeMediaEffectSlot,
} from "../../api/client/mediaOutput";
import type { MediaServerFixture } from "../../api/types";
import type { ProgrammerFixtureValue } from "../../features/programmerValues/contracts";
import { controlSections } from "./mediaControlSections";
import { libraryModel, selectionModel } from "./mediaPaneLibraryModel";
import type {
	MediaEffectLibrarySlot,
	MediaPaneModel,
	MediaSourceFilter,
} from "./mediaPaneModel";
import {
	layerModels,
	previewState,
	serverChoices,
} from "./mediaPaneServerModel";

export { mediaOfflineReason } from "./mediaPaneServerModel";

export interface BuildMediaPaneModelInput {
	inspection: MediaServerInspection;
	inspectionError: string | null;
	servers: MediaServerFixture[];
	selectedServer: MediaServerFixture | undefined;
	selectedServerId: string;
	selectedLayerId: string;
	browserMode: MediaPaneModel["browserMode"];
	sourceFilter?: MediaSourceFilter;
	selectedControlSectionId: string;
	mainSectionId: string;
	rightPaneVisible: boolean;
	draftFolderId: string;
	draftFileId: string | null;
	thumbnailUrls: Record<string, string>;
	previewUrls: Record<string, string>;
	liveProgrammer: readonly ProgrammerFixtureValue[] | undefined;
	nativeEffects?: NativeMediaEffectSlot[];
	nativeEffectsError?: string | null;
	/** Sparse native-library status; settings remain owned by Pixel's Effects library. */
	effectLibrarySlots?: readonly MediaEffectLibrarySlot[];
}

export function buildMediaPaneModel(
	input: BuildMediaPaneModelInput,
): MediaPaneModel {
	const selectedLayer = input.selectedServer?.layers.find(
		(layer) => layer.fixture_id === input.selectedLayerId,
	);
	const selectedCitpLayer = input.selectedServer?.layers.findIndex(
		(layer) => layer.fixture_id === input.selectedLayerId,
	);
	const selectedStatus = selectedLayer
		? input.inspection.layers.find((layer) => layer.layer === selectedCitpLayer)
		: undefined;
	const capabilities = selectedLayer
		? input.inspection.capabilities.layers.find(
				(candidate) => candidate.layer === selectedLayer.head_index,
			)
		: undefined;
	const liveFolder =
		normalizedAttribute(input.liveProgrammer, "media.folder") ??
		selectedStatus?.folder;
	const liveFile =
		normalizedAttribute(input.liveProgrammer, "media.file") ??
		selectedStatus?.file;
	const sections = controlSections(
		input,
		capabilities?.secondary_controls ?? [],
	);
	return {
		hasPatchedServer: input.servers.length > 0,
		hasCitpEndpoint: Boolean(input.selectedServer?.endpoint),
		servers: serverChoices(input),
		selectedServerId: input.selectedServerId,
		selectedLayerId: input.selectedLayerId,
		preview: previewState(input),
		layers: layerModels(input),
		browserMode: input.browserMode,
		sourceFilter: input.sourceFilter ?? "media",
		showSourceFilters: Boolean(input.selectedServer?.native_action),
		maskBrowser: "supported",
		...libraryModel(input),
		...selectionModel(input, liveFolder, liveFile),
		controlSections: sections,
		selectedControlSectionId: sections.some(
			(section) => section.id === input.selectedControlSectionId,
		)
			? input.selectedControlSectionId
			: (sections.find((section) => section.id !== "native")?.id ??
				sections[0]?.id ??
				""),
		mainSectionId: input.mainSectionId,
		rightPaneVisible: input.rightPaneVisible,
		nativeManagementUrl:
			input.selectedServer?.native_action && input.selectedServer.endpoint
				? `http://${input.selectedServer.endpoint.ip_address}:8080`
				: undefined,
	};
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
