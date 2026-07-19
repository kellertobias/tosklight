import { useCallback, useEffect, useRef } from "react";
import type { TextDocument } from "../../api/types";
import { registerPaneRemovalGuard } from "../../components/shell/paneRemovalGuard";
import { useFiles } from "../../features/files/FilesContext";
import {
	publishTextFileSaved,
	TEXT_FILE_OPERATION_EVENT,
	TEXT_FILE_SAVED_EVENT,
	textDocumentFromSavedEvent,
	textFileLocationChange,
	textFileOperationFromEvent,
} from "../textFileSync";
import { isMissingFileError } from "./fileUtilities";
import type { FileManagerSelection } from "./types";
import type { FileManagerState } from "./useFileManagerState";

export function hasUnsavedEditorText(state: FileManagerState) {
	return Boolean(
		state.editorRef.current &&
			state.editorTextRef.current !== state.editorRef.current.text,
	);
}

export function confirmDiscardEditor(state: FileManagerState) {
	return (
		!hasUnsavedEditorText(state) || window.confirm("Discard unsaved changes?")
	);
}

function useTextEditorSynchronization(
	state: FileManagerState,
	paneId?: string,
) {
	const server = useFiles();
	const serverRef = useRef(server);
	serverRef.current = server;

	const surfaceTextRevision = useCallback(
		(next: TextDocument, source: string) => {
			const currentEditor = state.editorRef.current;
			if (
				!currentEditor ||
				currentEditor.root_id !== next.root_id ||
				currentEditor.path !== next.path
			)
				return;
			if (
				currentEditor.revision === next.revision &&
				currentEditor.text === next.text &&
				currentEditor.read_only === next.read_only
			)
				return;
			if (
				state.editorTextRef.current !== currentEditor.text &&
				state.editorTextRef.current !== next.text
			) {
				state.setEditorConflict(next);
				state.setMessage(
					`${source} changed this file while the File Manager has unsaved edits. The local text has been preserved.`,
				);
				return;
			}
			state.editorRef.current = next;
			state.editorTextRef.current = next.text;
			state.setEditor(next);
			state.setEditorText(next.text);
			state.setEditorConflict(null);
			state.setEditorMissing(false);
			state.setMessage(
				`${source} saved a newer version. The File Manager editor has been updated.`,
			);
		},
		[],
	);

	useEffect(() => {
		if (!paneId) return;
		return registerPaneRemovalGuard(paneId, () =>
			hasUnsavedEditorText(state)
				? "File Manager has unsaved text changes."
				: null,
		);
	}, [paneId]);

	useEffect(() => {
		const saved = (event: Event) => {
			const detail = textDocumentFromSavedEvent(event);
			if (!detail || detail.sourcePaneId === state.instanceId) return;
			surfaceTextRevision(detail.document, "Another editor");
		};
		window.addEventListener(TEXT_FILE_SAVED_EVENT, saved);
		return () => window.removeEventListener(TEXT_FILE_SAVED_EVENT, saved);
	}, [state.instanceId, surfaceTextRevision]);

	useEffect(() => {
		const operated = (event: Event) => {
			const detail = textFileOperationFromEvent(event);
			const currentEditor = state.editorRef.current;
			if (!detail || !currentEditor) return;
			const change = textFileLocationChange(
				currentEditor.root_id,
				currentEditor.path,
				detail,
			);
			if (!change) return;
			if (change.kind === "deleted") {
				state.setEditorMissing(true);
				state.setMessage(
					"The open file was deleted or moved to Trash. Its last loaded text is retained.",
				);
				return;
			}
			const moved = {
				...currentEditor,
				root_id: change.rootId,
				path: change.path,
			};
			state.editorRef.current = moved;
			state.setEditor(moved);
			state.setEditorConflict((pending) =>
				pending
					? { ...pending, root_id: change.rootId, path: change.path }
					: null,
			);
			state.setEditorMissing(false);
			state.setMessage(
				`The open file moved to ${change.path}. Unsaved text, if any, was retained.`,
			);
		};
		window.addEventListener(TEXT_FILE_OPERATION_EVENT, operated);
		return () =>
			window.removeEventListener(TEXT_FILE_OPERATION_EVENT, operated);
	}, []);

	useEffect(() => {
		const openEditor = state.editor;
		if (!openEditor) return;
		let cancelled = false;
		let checking = false;
		const check = async () => {
			if (checking) return;
			checking = true;
			try {
				const next = await serverRef.current.readTextFile(
					openEditor.root_id,
					openEditor.path,
				);
				if (!cancelled)
					surfaceTextRevision(next, "Another editor or external program");
			} catch (error) {
				if (!cancelled && isMissingFileError(error)) {
					state.setEditorMissing(true);
					state.setMessage(
						"The open file is missing, moved, deleted, or its location is unavailable. Its last loaded text is retained.",
					);
				}
			} finally {
				checking = false;
			}
		};
		const timer = window.setInterval(() => void check(), 1_500);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [state.editor?.path, state.editor?.root_id, surfaceTextRevision]);

	return surfaceTextRevision;
}

export function useTextFileEditor(
	state: FileManagerState,
	paneId: string | undefined,
	refreshAfterMutation: () => Promise<void>,
) {
	const server = useFiles();
	const surfaceTextRevision = useTextEditorSynchronization(state, paneId);

	async function openText(file: FileManagerSelection) {
		try {
			const document = await server.readTextFile(file.rootId, file.entry.path);
			state.setEditor(document);
			state.setEditorText(document.text);
			state.setEditorConflict(null);
			state.setEditorMissing(false);
		} catch (error) {
			state.setMessage(`Text editor unavailable: ${String(error)}`);
		}
	}

	async function saveText() {
		if (!state.editor || state.editorConflict || state.editorMissing) return;
		state.setBusy(true);
		try {
			const document = await server.saveTextFile(
				state.editor.root_id,
				state.editor.path,
				state.editorText,
				state.editor.revision,
			);
			state.setEditor(document);
			state.setEditorText(document.text);
			state.setEditorConflict(null);
			state.setMessage("Saved.");
			publishTextFileSaved(document, state.instanceId);
		} catch (error) {
			try {
				const latest = await server.readTextFile(
					state.editor.root_id,
					state.editor.path,
				);
				if (
					latest.revision !== state.editor.revision ||
					latest.text !== state.editor.text ||
					latest.read_only !== state.editor.read_only
				) {
					surfaceTextRevision(latest, "Another editor or external program");
					return;
				}
			} catch (latestError) {
				if (isMissingFileError(latestError)) {
					state.setEditorMissing(true);
					state.setMessage(
						"The file was removed before it could be saved. The File Manager retained your text; recreate the file to store it.",
					);
					return;
				}
			}
			state.setMessage(`Could not save: ${String(error)}`);
		} finally {
			state.setBusy(false);
		}
	}

	async function recreateText() {
		if (
			!state.editor ||
			!state.editorMissing ||
			!window.confirm(`Recreate ${state.editor.path} from the retained text?`)
		)
			return;
		state.setBusy(true);
		try {
			const document = await server.saveTextFile(
				state.editor.root_id,
				state.editor.path,
				state.editorText,
				null,
			);
			state.editorRef.current = document;
			state.setEditor(document);
			state.setEditorText(document.text);
			state.setEditorConflict(null);
			state.setEditorMissing(false);
			state.setMessage("File recreated.");
			publishTextFileSaved(document, state.instanceId);
			await refreshAfterMutation();
		} catch (error) {
			state.setMessage(`Could not recreate the file: ${String(error)}`);
		} finally {
			state.setBusy(false);
		}
	}

	function closeText() {
		if (!confirmDiscardEditor(state)) return;
		state.setEditor(null);
		state.setEditorConflict(null);
		state.setEditorMissing(false);
	}

	function reloadConflict() {
		if (!state.editorConflict) return;
		state.setEditor(state.editorConflict);
		state.setEditorText(state.editorConflict.text);
		state.setEditorConflict(null);
		state.setMessage("Reloaded the newer file revision.");
	}

	return { openText, saveText, recreateText, closeText, reloadConflict };
}

export type TextFileEditor = ReturnType<typeof useTextFileEditor>;
