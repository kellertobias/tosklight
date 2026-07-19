import { useId, useRef, useState } from "react";
import type { FileEntry, FileRoot, TextDocument } from "../../api/types";
import { usePaneChromeTargets } from "../../components/shell/PaneChromeContext";
import { useFiles } from "../../features/files/FilesContext";
import { useApp } from "../../state/AppContext";
import type { Availability, Notice } from "./files";

export function useTextEditorState(paneId: string | undefined) {
	const server = useFiles();
	const serverRef = useRef(server);
	serverRef.current = server;
	const { state, dispatch } = useApp();
	const paneChrome = usePaneChromeTargets();
	const pane = state.desks
		.flatMap((desk) => desk.panes)
		.find((candidate) => candidate.id === paneId);
	const selectedPath = pane?.textFilePath ?? "";
	const paneReadOnly = Boolean(pane?.textEditorReadOnly);
	const editorMode = pane?.textEditorMode ?? "plain";
	const [roots, setRoots] = useState<FileRoot[]>([]);
	const [files, setFiles] = useState<FileEntry[]>([]);
	const [filesLoading, setFilesLoading] = useState(false);
	const [document, setDocument] = useState<TextDocument | null>(null);
	const [externalDocument, setExternalDocument] = useState<TextDocument | null>(
		null,
	);
	const [text, setText] = useState("");
	const [dirty, setDirty] = useState(false);
	const [saving, setSaving] = useState(false);
	const [availability, setAvailability] = useState<Availability>("none");
	const [notice, setNotice] = useState<Notice>(null);
	const messageId = useId();
	const textarea = useRef<HTMLTextAreaElement>(null);
	const documentRef = useRef<TextDocument | null>(null);
	const externalDocumentRef = useRef<TextDocument | null>(null);
	const dirtyRef = useRef(false);
	const textRef = useRef("");
	const availabilityRef = useRef<Availability>("none");
	const fileListRequest = useRef(0);
	const relocatedAssociation = useRef<{ root: string; path: string } | null>(
		null,
	);
	documentRef.current = document;
	externalDocumentRef.current = externalDocument;
	dirtyRef.current = dirty;
	textRef.current = text;
	availabilityRef.current = availability;
	const selectedRoot = pane?.textFileRoot ?? roots[0]?.id ?? "";

	return {
		availability,
		availabilityRef,
		dirty,
		dirtyRef,
		dispatch,
		document,
		documentRef,
		editorMode,
		externalDocument,
		externalDocumentRef,
		fileListRequest,
		files,
		filesLoading,
		messageId,
		notice,
		pane,
		paneChrome,
		paneId,
		paneReadOnly,
		relocatedAssociation,
		roots,
		saving,
		selectedPath,
		selectedRoot,
		serverRef,
		setAvailability,
		setDirty,
		setDocument,
		setExternalDocument,
		setFiles,
		setFilesLoading,
		setNotice,
		setRoots,
		setSaving,
		setText,
		text,
		textarea,
		textRef,
	};
}

export type TextEditorState = ReturnType<typeof useTextEditorState>;
