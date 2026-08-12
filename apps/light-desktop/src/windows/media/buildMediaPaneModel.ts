import type { MediaServerInspection } from "../../api/client/mediaOutput";
import type { MediaServerFixture } from "../../api/types";
import type { ProgrammerFixtureValue } from "../../features/programmerValues/contracts";
import type {
	MediaControlSection,
	MediaPaneLayer,
	MediaPaneModel,
	MediaPreviewState,
} from "./mediaPaneModel";

export interface BuildMediaPaneModelInput {
	inspection: MediaServerInspection;
	inspectionError: string | null;
	servers: MediaServerFixture[];
	selectedServer: MediaServerFixture | undefined;
	selectedServerId: string;
	selectedLayerId: string;
	browserMode: MediaPaneModel["browserMode"];
	mainSectionId: string;
	rightPaneVisible: boolean;
	draftFolderId: string;
	draftFileId: string | null;
	thumbnailUrls: Record<string, string>;
	previewUrls: Record<string, string>;
	liveProgrammer: readonly ProgrammerFixtureValue[] | undefined;
}

export function buildMediaPaneModel(
	input: BuildMediaPaneModelInput,
): MediaPaneModel {
	const selectedLayer = input.selectedServer?.layers.find(
		(layer) => layer.fixture_id === input.selectedLayerId,
	);
	const selectedStatus = selectedLayer
		? input.inspection.layers.find(
				(layer) => layer.layer === selectedLayer.head_index,
			)
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
	return {
		servers: serverChoices(input),
		selectedServerId: input.selectedServerId,
		selectedLayerId: input.selectedLayerId,
		preview: previewState(input),
		layers: layerModels(input),
		browserMode: input.browserMode,
		maskBrowser: "supported",
		...libraryModel(input),
		...selectionModel(input, liveFolder, liveFile),
		controlSections: controlSections(
			input,
			capabilities?.secondary_controls ?? [],
		),
		selectedControlSectionId: capabilities?.secondary_controls.length
			? "advertised"
			: "",
		mainSectionId: input.mainSectionId,
		rightPaneVisible: input.rightPaneVisible,
	};
}

function serverChoices(input: BuildMediaPaneModelInput) {
	if (input.servers.length === 0 && !input.selectedServerId)
		return [
			{
				id: "",
				name: "No CITP Media Server available",
				statusLabel: "Not configured",
				disabled: true,
			},
		];
	return [
		...(input.selectedServerId && !input.selectedServer
			? [
					{
						id: input.selectedServerId,
						name: "Configured server unavailable",
						statusLabel: "Missing patch",
						disabled: true,
					},
				]
			: []),
		...input.servers.map((server) => ({
			id: server.fixture_id,
			name: server.name,
			statusLabel: !server.endpoint
				? "Not configured"
				: server.status.online
					? "Online"
					: "Offline",
		})),
	];
}

function previewState(input: BuildMediaPaneModelInput): MediaPreviewState {
	if (!input.selectedServer)
		return {
			kind: "missing_patch",
			detail:
				"No CITP Media Server is available. Configure one in Show Patch > Media Servers.",
		};
	if (!input.selectedServer.endpoint)
		return {
			kind: "offline",
			detail:
				"No CITP Media Server is available. Configure one in Show Patch > Media Servers.",
		};
	if (input.inspectionError || !input.selectedServer.status.online)
		return {
			kind: "offline",
			detail:
				input.inspectionError ??
				input.selectedServer.status.last_error ??
				"Server offline",
		};
	const source = input.inspection.preview_sources.find(
		(candidate) => candidate.layer == null,
	);
	return source
		? {
				kind: "ready",
				imageSrc:
					input.previewUrls[`${input.selectedServer.fixture_id}:${source.id}`],
			}
		: {
				kind: "unsupported",
				capability: "preview",
				detail: "No composite preview source is advertised.",
			};
}

function layerModels(input: BuildMediaPaneModelInput): MediaPaneLayer[] {
	return (input.selectedServer?.layers ?? []).map((head) => {
		const status = input.inspection.layers.find(
			(layer) => layer.layer === head.head_index,
		);
		const source = input.inspection.preview_sources.find(
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
				? input.previewUrls[`${input.selectedServer?.fixture_id}:${source.id}`]
				: undefined,
			liveSourceLabel: status
				? `Folder ${status.folder} · File ${status.file}`
				: undefined,
		};
	});
}

function libraryModel(input: BuildMediaPaneModelInput) {
	const draftFolder = Number(input.draftFolderId);
	const advertisedFolders = new Map(
		input.inspection.folders.map((folder) => [folder.id, folder]),
	);
	const advertisedFiles = new Map(
		input.inspection.files
			.filter((file) => file.folder_id === draftFolder)
			.map((file) => [file.id, file]),
	);
	return {
		libraryFolders: Array.from({ length: 256 }, (_, id) => {
			const folder = advertisedFolders.get(id);
			return {
				id: String(id),
				kind: "folder" as const,
				name: folder?.name || `Folder ${id}`,
				detail: folder
					? `${folder.element_count} files`
					: "Configurable slot · not advertised",
			};
		}),
		libraryFiles: Array.from({ length: 256 }, (_, id) => {
			const file = advertisedFiles.get(id);
			return {
				id: String(id),
				kind: "file" as const,
				name: file?.name || `File ${id}`,
				detail: file
					? `${file.width}×${file.height}`
					: "Configurable slot · not advertised",
				thumbnailSrc: input.thumbnailUrls[String(id)],
			};
		}),
	};
}

function selectionModel(
	input: BuildMediaPaneModelInput,
	liveFolder: number | undefined,
	liveFile: number | undefined,
) {
	return {
		draftFolderId: input.draftFolderId,
		draftFileId: input.draftFileId,
		liveSelection: {
			folderId: liveFolder == null ? null : String(liveFolder),
			fileId: liveFile == null ? null : String(liveFile),
			maskFolderId: null,
			maskFileId: null,
		},
		draftSelection: {
			folderId: input.draftFolderId || null,
			fileId: input.draftFileId,
			maskFolderId: null,
			maskFileId: null,
		},
		liveSelectionLabel:
			liveFolder == null
				? "No live media"
				: `Folder ${liveFolder} / File ${liveFile ?? "None"}`,
		draftSelectionLabel: input.draftFolderId
			? `Folder ${input.draftFolderId} / File ${input.draftFileId ?? "Choose"}`
			: "Choose a folder",
	};
}

function controlSections(
	input: BuildMediaPaneModelInput,
	controls: Array<{ attribute: string }>,
): MediaControlSection[] {
	if (controls.length === 0 || !input.selectedServer) return [];
	return [
		{
			id: "advertised",
			label: "Layer controls",
			capability: "supported",
			controls: controls.map((control) => ({
				id: control.attribute,
				label: control.attribute,
				kind: "value",
				value:
					normalizedAttribute(input.liveProgrammer, control.attribute) ?? 0,
				minimum: 0,
				maximum: 255,
				step: 1,
			})),
		},
	];
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
