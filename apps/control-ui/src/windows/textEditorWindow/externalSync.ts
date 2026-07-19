import { useEffect } from "react";
import type { TextDocument } from "../../api/types";
import {
	TEXT_FILE_OPERATION_EVENT,
	TEXT_FILE_SAVED_EVENT,
	textDocumentFromSavedEvent,
	textFileLocationChange,
	textFileOperationFromEvent,
} from "../textFileSync";
import type { ReloadTextFiles } from "./catalog";
import {
	EXTERNAL_CHECK_INTERVAL_MILLIS,
	isMissingError,
	isSameDocumentVersion,
} from "./files";
import type { TextEditorState } from "./state";

type SurfaceExternalDocument = (next: TextDocument, source: string) => void;

export function useExternalDocumentPolling(
	model: TextEditorState,
	surfaceExternalDocument: SurfaceExternalDocument,
) {
	const {
		availabilityRef,
		documentRef,
		externalDocumentRef,
		selectedPath,
		selectedRoot,
		serverRef,
		setAvailability,
		setNotice,
		textRef,
	} = model;
	useEffect(() => {
		if (!selectedRoot || !selectedPath) return;
		let cancelled = false;
		let checking = false;
		const check = async () => {
			if (checking) return;
			checking = true;
			try {
				const next = await serverRef.current.readTextFile(
					selectedRoot,
					selectedPath,
				);
				if (cancelled) return;
				if (availabilityRef.current === "missing") {
					surfaceExternalDocument(next, "Another editor or external program");
					return;
				}
				if (isSameDocumentVersion(externalDocumentRef.current, next)) return;
				if (!isSameDocumentVersion(documentRef.current, next)) {
					surfaceExternalDocument(next, "Another editor or external program");
				}
			} catch (error) {
				if (!cancelled && isMissingError(error)) {
					availabilityRef.current = "missing";
					setAvailability("missing");
					setNotice({
						kind: "error",
						text: `The selected file is missing, moved, deleted, or its location is unavailable: ${selectedPath}. ${
							textRef.current
								? "The last loaded text is retained in this window."
								: ""
						}`.trim(),
					});
				}
			} finally {
				checking = false;
			}
		};
		const timer = window.setInterval(
			() => void check(),
			EXTERNAL_CHECK_INTERVAL_MILLIS,
		);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [
		availabilityRef,
		documentRef,
		externalDocumentRef,
		selectedPath,
		selectedRoot,
		serverRef,
		setAvailability,
		setNotice,
		surfaceExternalDocument,
		textRef,
	]);
}

export function useTextEditorEvents(
	model: TextEditorState,
	reloadFiles: ReloadTextFiles,
	surfaceExternalDocument: SurfaceExternalDocument,
) {
	useSavedDocumentEvents(model, surfaceExternalDocument);
	useFileOperationEvents(model, reloadFiles);
}

function useSavedDocumentEvents(
	model: TextEditorState,
	surfaceExternalDocument: SurfaceExternalDocument,
) {
	const {
		availabilityRef,
		documentRef,
		externalDocumentRef,
		paneId,
		selectedPath,
		selectedRoot,
	} = model;
	useEffect(() => {
		const saved = (event: Event) => {
			const detail = textDocumentFromSavedEvent(event);
			if (!detail || detail.sourcePaneId === paneId) return;
			const next = detail.document;
			if (next.root_id !== selectedRoot || next.path !== selectedPath) return;
			if (
				availabilityRef.current !== "missing" &&
				(isSameDocumentVersion(externalDocumentRef.current, next) ||
					isSameDocumentVersion(documentRef.current, next))
			) {
				return;
			}
			surfaceExternalDocument(next, "Another Text Editor window");
		};
		window.addEventListener(TEXT_FILE_SAVED_EVENT, saved);
		return () => window.removeEventListener(TEXT_FILE_SAVED_EVENT, saved);
	}, [
		availabilityRef,
		documentRef,
		externalDocumentRef,
		paneId,
		selectedPath,
		selectedRoot,
		surfaceExternalDocument,
	]);
}

function useFileOperationEvents(
	model: TextEditorState,
	reloadFiles: ReloadTextFiles,
) {
	const {
		availabilityRef,
		dispatch,
		documentRef,
		externalDocumentRef,
		paneId,
		relocatedAssociation,
		selectedPath,
		selectedRoot,
		setAvailability,
		setDocument,
		setExternalDocument,
		setNotice,
	} = model;
	useEffect(() => {
		const operated = (event: Event) => {
			const detail = textFileOperationFromEvent(event);
			if (!detail) return;
			const current = documentRef.current;
			const path = current?.path ?? selectedPath;
			const change = textFileLocationChange(
				current?.root_id ?? selectedRoot,
				path,
				detail,
			);
			if (!change) return;
			if (change.kind === "deleted") {
				availabilityRef.current = "missing";
				setAvailability("missing");
				setNotice({
					kind: "error",
					text: `The selected file was deleted or moved to Trash: ${path}. The last loaded text is retained in this window.`,
				});
				return;
			}
			moveOpenDocuments(
				{
					availabilityRef,
					documentRef,
					externalDocumentRef,
					setAvailability,
					setDocument,
					setExternalDocument,
				},
				change.rootId,
				change.path,
			);
			setNotice({
				kind: "info",
				text: `The open file moved from ${path} to ${change.path}. Unsaved text, if any, is still in this editor.`,
			});
			if (current) {
				relocatedAssociation.current = {
					root: change.rootId,
					path: change.path,
				};
			}
			if (paneId) {
				dispatch({
					type: "SET_TEXT_EDITOR_FILE",
					id: paneId,
					root: change.rootId,
					path: change.path,
				});
			}
			void reloadFiles(change.rootId);
		};
		window.addEventListener(TEXT_FILE_OPERATION_EVENT, operated);
		return () =>
			window.removeEventListener(TEXT_FILE_OPERATION_EVENT, operated);
	}, [
		availabilityRef,
		dispatch,
		documentRef,
		externalDocumentRef,
		paneId,
		reloadFiles,
		relocatedAssociation,
		selectedPath,
		selectedRoot,
		setAvailability,
		setDocument,
		setExternalDocument,
		setNotice,
	]);
}

function moveOpenDocuments(
	model: Pick<
		TextEditorState,
		| "availabilityRef"
		| "documentRef"
		| "externalDocumentRef"
		| "setAvailability"
		| "setDocument"
		| "setExternalDocument"
	>,
	root: string,
	path: string,
) {
	const current = model.documentRef.current;
	if (current) {
		const moved = { ...current, root_id: root, path };
		model.documentRef.current = moved;
		model.setDocument(moved);
	}
	const pending = model.externalDocumentRef.current;
	if (pending) {
		const moved = { ...pending, root_id: root, path };
		model.externalDocumentRef.current = moved;
		model.setExternalDocument(moved);
	}
	model.availabilityRef.current = "ready";
	model.setAvailability("ready");
}
