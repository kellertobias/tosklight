import type { FileEntry } from "../../api/types";

export type FileManagerOperationKind = "rename" | "copy" | "move" | "delete";
export type FileManagerTarget = "files" | "folders" | "either";
export type FileManagerView = "list" | "grid";
export type FileManagerSidePanel = "none" | "navigation" | "info";
export type FileHeaderMenuKind = "location" | "edit" | "create" | "view";

export interface FileManagerSelection {
	rootId: string;
	entry: FileEntry;
}

export interface FileManagerPickerOptions {
	purpose?: string;
	target?: FileManagerTarget;
	multiple?: boolean;
	allowedExtensions?: string[];
	initialRootId?: string;
	initialDirectory?: string;
	selectLabel?: string;
	cancelLabel?: string;
	hideCancel?: boolean;
	onSelect: (selection: FileManagerSelection[]) => void;
	onCancel: () => void;
}

export interface FileManagerProps {
	picker?: FileManagerPickerOptions;
	instanceId?: string;
	paneId?: string;
	closeable?: boolean;
	purpose?: string;
}

export interface FileManagerLocation {
	rootId: string;
	path: string;
}

export interface FileOperationState {
	kind: FileManagerOperationKind;
	sources: FileManagerSelection[];
	renameDraft: string;
	confirming: boolean;
}

export interface ConflictState {
	operation: FileOperationState;
	applyToAll: boolean;
}

export interface FileHeaderMenu {
	kind: FileHeaderMenuKind;
	anchor: DOMRect;
}
