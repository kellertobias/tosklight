import { useId, useRef, useState } from "react";
import type {
	FileDirectory,
	FileNativeNote,
	FileRoot,
	TextDocument,
} from "../../api/types";
import type {
	ConflictState,
	FileHeaderMenu,
	FileManagerLocation,
	FileManagerProps,
	FileManagerSelection,
	FileManagerSidePanel,
	FileManagerView,
	FileOperationState,
} from "./types";

export function useFileManagerState(suppliedInstanceId?: string) {
	const generatedId = useId();
	const instanceId = useRef(
		suppliedInstanceId ?? `file-manager-${generatedId.replaceAll(":", "")}`,
	).current;
	const [roots, setRoots] = useState<FileRoot[]>([]);
	const [listing, setListing] = useState<FileDirectory | null>(null);
	const [history, setHistory] = useState<FileManagerLocation[]>([]);
	const [historyIndex, setHistoryIndex] = useState(-1);
	const [selected, setSelected] = useState<FileManagerSelection[]>([]);
	const [selectionAnchor, setSelectionAnchor] = useState<string | null>(null);
	const [localHidden, setLocalHidden] = useState(false);
	const [view, setView] = useState<FileManagerView>("list");
	const [sidePanel, setSidePanel] = useState<FileManagerSidePanel>("none");
	const [propertiesVisible, setPropertiesVisible] = useState(true);
	const [headerMenu, setHeaderMenu] = useState<FileHeaderMenu | null>(null);
	const [message, setMessage] = useState("");
	const [busy, setBusy] = useState(false);
	const [previewUrl, setPreviewUrl] = useState("");
	const [nativeNote, setNativeNote] = useState<FileNativeNote | null>(null);
	const [noteDraft, setNoteDraft] = useState("");
	const [editor, setEditor] = useState<TextDocument | null>(null);
	const [editorText, setEditorText] = useState("");
	const [editorConflict, setEditorConflict] = useState<TextDocument | null>(
		null,
	);
	const [editorMissing, setEditorMissing] = useState(false);
	const editorRef = useRef<TextDocument | null>(null);
	const editorTextRef = useRef("");
	editorRef.current = editor;
	editorTextRef.current = editorText;
	const [operation, setOperationState] = useState<FileOperationState | null>(
		null,
	);
	const operationRef = useRef<FileOperationState | null>(null);
	const [conflict, setConflict] = useState<ConflictState | null>(null);
	const [treeChildren, setTreeChildren] = useState<
		Record<string, FileDirectory["entries"]>
	>({});
	const [treeExpanded, setTreeExpanded] = useState<Set<string>>(new Set());
	const initialized = useRef(false);

	return {
		instanceId,
		roots,
		setRoots,
		listing,
		setListing,
		history,
		setHistory,
		historyIndex,
		setHistoryIndex,
		selected,
		setSelected,
		selectionAnchor,
		setSelectionAnchor,
		localHidden,
		setLocalHidden,
		view,
		setView,
		sidePanel,
		setSidePanel,
		propertiesVisible,
		setPropertiesVisible,
		headerMenu,
		setHeaderMenu,
		message,
		setMessage,
		busy,
		setBusy,
		previewUrl,
		setPreviewUrl,
		nativeNote,
		setNativeNote,
		noteDraft,
		setNoteDraft,
		editor,
		setEditor,
		editorText,
		setEditorText,
		editorConflict,
		setEditorConflict,
		editorMissing,
		setEditorMissing,
		editorRef,
		editorTextRef,
		operation,
		setOperationState,
		operationRef,
		conflict,
		setConflict,
		treeChildren,
		setTreeChildren,
		treeExpanded,
		setTreeExpanded,
		initialized,
	};
}

export type FileManagerState = ReturnType<typeof useFileManagerState>;

export function currentLocation(state: FileManagerState) {
	return state.historyIndex >= 0 ? state.history[state.historyIndex] : null;
}

export function operationLabel(state: FileOperationState | null) {
	if (state?.kind === "copy") return "Copy";
	if (state?.kind === "move") return "Move";
	if (state?.kind === "rename") return "Rename";
	if (state?.kind === "delete") return "Delete";
	return "";
}

export type { FileManagerProps };
