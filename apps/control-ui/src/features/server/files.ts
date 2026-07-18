import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createFileActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "fileRoots"
	| "fileEntries"
	| "fileMetadata"
	| "readFileNote"
	| "saveFileNote"
	| "readTextFile"
	| "saveTextFile"
	| "fileOperation"
	| "fileContent"
	| "fileStreamUrl"
	| "fileThumbnail"
	| "claimFileInput"
	| "releaseFileInput"
> {
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
