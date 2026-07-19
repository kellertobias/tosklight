import type {
	ConnectionStatus,
	FileDirectory,
	FileInputAction,
	FileInputContext,
	FileMetadata,
	FileNativeNote,
	FileOperationInput,
	FileOperationResult,
	FileRoot,
	TextDocument,
} from "../../api/types";

export interface FileCapabilities {
	fileRoots: () => Promise<FileRoot[]>;
	fileEntries: (
		root: string,
		path?: string,
		hidden?: boolean,
	) => Promise<FileDirectory>;
	fileMetadata: (root: string, path: string) => Promise<FileMetadata>;
	readFileNote: (root: string, path: string) => Promise<FileNativeNote>;
	saveFileNote: (
		root: string,
		path: string,
		note: string,
	) => Promise<FileNativeNote>;
	readTextFile: (root: string, path: string) => Promise<TextDocument>;
	saveTextFile: (
		root: string,
		path: string,
		text: string,
		revision: string | null,
	) => Promise<TextDocument>;
	fileOperation: (
		root: string,
		input: FileOperationInput,
	) => Promise<FileOperationResult>;
	fileContent: (root: string, path: string) => Promise<Blob>;
	fileStreamUrl: (root: string, path: string) => Promise<string>;
	fileThumbnail: (
		root: string,
		path: string,
		maxSize?: number,
	) => Promise<Blob>;
	claimFileInput: (
		instanceId: string,
		action: FileInputAction,
		origin: "pending" | "toolbar",
	) => Promise<FileInputContext>;
	releaseFileInput: (instanceId: string) => Promise<void>;
}

export interface FilesContextValue extends FileCapabilities {
	status: ConnectionStatus;
	systemPickerFallback: boolean;
}
