import type { BuildMediaPaneModelInput } from "./buildMediaPaneModel";
import { isAudioPlayer } from "./mediaPaneServerModel";

/// The folder part of an indexed audio path, without its numeric address prefix.
///
/// A library folder is named by its address alone, so stripping it leaves nothing and the slot
/// keeps its own "Folder N" label rather than repeating the number.
function audioFolderName(relativePath: string): string {
	const folder = relativePath.split("/")[0] ?? relativePath;
	const stripped = folder.replace(/^\s*0*\d+\s*[-_. ]*/u, "");
	return stripped;
}

/// The file part of an indexed audio path, without its numeric address prefix.
///
/// A library entry is addressed by the number leading its name, which the operator does not need
/// to read twice because the slot already shows it.
function audioFileName(relativePath: string): string {
	const parts = relativePath.split("/");
	const file = parts[parts.length - 1] ?? relativePath;
	return file.replace(/^\s*0*\d+\s*[-_. ]*/u, "") || file;
}

export function libraryModel(input: BuildMediaPaneModelInput) {
	const sourceFilter = input.sourceFilter ?? "media";
	const draftFolder = Number(input.draftFolderId);
	// An Internal Audio Player advertises nothing over CITP; its addressable folders and files
	// come from the library the desk indexed for it.
	const audioLibrary = isAudioPlayer(input.selectedServer)
		? (input.selectedServer?.audio?.library ?? [])
		: null;
	const advertisedFolders = audioLibrary
		? new Map(
				audioLibrary.map((entry) => [
					entry.folder,
					{
						id: entry.folder,
						name: audioFolderName(entry.name),
						element_count: audioLibrary.filter(
							(item) => item.folder === entry.folder,
						).length,
					},
				]),
			)
		: new Map(input.inspection.folders.map((folder) => [folder.id, folder]));
	const advertisedFiles = audioLibrary
		? new Map(
				audioLibrary
					.filter((entry) => entry.folder === draftFolder)
					.map((entry) => [
						entry.file,
						{
							folder_id: entry.folder,
							id: entry.file,
							name: audioFileName(entry.name),
							width: 0,
							height: 0,
						},
					]),
			)
		: new Map(
				input.inspection.files
					.filter((file) => file.folder_id === draftFolder)
					.map((file) => [file.id, file]),
			);
	const [firstFolder, lastFolder] =
		input.selectedServer && input.selectedLayerId === "master"
			? [1, 1]
			: sourceFilter === "media"
				? [1, 199]
				: sourceFilter === "text"
					? [200, 249]
					: [250, 255];
	return {
		libraryFolders: Array.from(
			{ length: lastFolder - firstFolder + 1 },
			(_, index) => {
				const id = firstFolder + index;
				const folder = advertisedFolders.get(id);
				return {
					id: String(id),
					kind: "folder" as const,
					name: folder?.name || `Folder ${id}`,
					detail: folder
						? `${folder.element_count} files`
						: audioLibrary
							? "Empty audio folder"
							: "Configurable slot · not advertised",
				};
			},
		),
		libraryFiles: Array.from({ length: 254 }, (_, index) => {
			const id = index + 1;
			const file = advertisedFiles.get(id);
			const slotType =
				input.sourceFilter === "visualizers"
					? "Visualizer"
					: input.sourceFilter === "text"
						? "Text"
						: "Media";
			return {
				id: String(id),
				kind: "file" as const,
				name: file?.name || "Empty",
				detail: file
					? audioLibrary
						? "Audio file"
						: `${file.width}×${file.height}`
					: audioLibrary
						? "Empty audio slot"
						: `${slotType} slot · not advertised`,
				thumbnailSrc: input.thumbnailUrls[`${draftFolder}:${id}`],
				empty: !file,
			};
		}),
	};
}

export function selectionModel(
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
