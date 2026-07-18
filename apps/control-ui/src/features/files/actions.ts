import type { LightApiClient } from "../../api/LightApiClient";
import type { FileCapabilities } from "./types";

interface FileActionDependencies {
	client: LightApiClient;
	fileRoots: FileCapabilities["fileRoots"];
	fileEntries: FileCapabilities["fileEntries"];
}

export function createFileActions(
	model: FileActionDependencies,
): FileCapabilities {
	const { client, fileRoots, fileEntries } = model;
	return {
		fileRoots,
		fileEntries,
		fileMetadata: (root, path) => client.fileMetadata(root, path),
		readFileNote: (root, path) => client.readFileNote(root, path),
		saveFileNote: (root, path, note) => client.saveFileNote(root, path, note),
		readTextFile: (root, path) => client.readTextFile(root, path),
		saveTextFile: (root, path, text, revision) =>
			client.saveTextFile(root, path, text, revision),
		fileOperation: (root, input) => client.fileOperation(root, input),
		fileContent: (root, path) => client.fileContent(root, path),
		fileStreamUrl: (root, path) => client.fileStreamUrl(root, path),
		fileThumbnail: (root, path, maxSize) =>
			client.fileThumbnail(root, path, maxSize),
		claimFileInput: (instanceId, action, origin) =>
			client.claimFileInput(instanceId, action, origin),
		releaseFileInput: (instanceId) => client.releaseFileInput(instanceId),
	};
}
