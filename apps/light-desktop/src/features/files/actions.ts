import type { LightApi } from "../../api/client/api";
import type { FileCapabilities } from "./types";

interface FileActionDependencies {
	api: LightApi;
	fileRoots: FileCapabilities["fileRoots"];
	fileEntries: FileCapabilities["fileEntries"];
}

export function createFileActions(
	model: FileActionDependencies,
): FileCapabilities {
	const { api, fileRoots, fileEntries } = model;
	return {
		fileRoots,
		fileEntries,
		fileMetadata: (root, path) => api.files.fileMetadata(root, path),
		readFileNote: (root, path) => api.files.readFileNote(root, path),
		saveFileNote: (root, path, note) => api.files.saveFileNote(root, path, note),
		readTextFile: (root, path) => api.files.readTextFile(root, path),
		saveTextFile: (root, path, text, revision) =>
			api.files.saveTextFile(root, path, text, revision),
		fileOperation: (root, input) => api.files.fileOperation(root, input),
		fileContent: (root, path) => api.files.fileContent(root, path),
		fileStreamUrl: (root, path) => api.files.fileStreamUrl(root, path),
		fileThumbnail: (root, path, maxSize) =>
			api.files.fileThumbnail(root, path, maxSize),
		claimFileInput: (instanceId, action, origin) =>
			api.files.claimFileInput(instanceId, action, origin),
		releaseFileInput: (instanceId) => api.files.releaseFileInput(instanceId),
	};
}
