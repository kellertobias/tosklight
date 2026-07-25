import type { FileEntry } from "../../api/types";
import { useTextFileCatalog } from "./catalog";
import {
	useDocumentAcceptance,
	useSelectedTextDocument,
} from "./documentLifecycle";
import {
	useExternalDocumentPolling,
	useTextEditorEvents,
} from "./externalSync";
import { useTextEditorGuards } from "./guards";
import { useTextFileOpenActions } from "./openActions";
import { useTextFileSaveActions } from "./saveActions";
import { useTextEditorState } from "./state";

function textEditorStatus(model: ReturnType<typeof useTextEditorState>) {
	if (model.saving) return "Saving…";
	if (model.externalDocument) return "Conflict";
	if (model.availability === "loading") return "Opening…";
	if (model.availability === "missing") return "Missing";
	if (model.dirty) return "Unsaved";
	if (model.document?.read_only || model.paneReadOnly) return "Read-only";
	if (model.document) return "Saved";
	return "No file";
}

function chooserFiles(model: ReturnType<typeof useTextEditorState>) {
	const current =
		model.selectedPath &&
		!model.files.some((file) => file.path === model.selectedPath)
			? ({
					name: model.selectedPath,
					path: model.selectedPath,
					kind: "file",
					size: 0,
					modified_millis: null,
					created_millis: null,
					hidden: false,
					writable: false,
				} satisfies FileEntry)
			: null;
	return current ? [current, ...model.files] : model.files;
}

export function useTextEditorController(paneId: string | undefined) {
	const model = useTextEditorState(paneId);
	const reloadFiles = useTextFileCatalog(model);
	const acceptance = useDocumentAcceptance(model);
	useSelectedTextDocument(model, acceptance.acceptDocument);
	useExternalDocumentPolling(model, acceptance.surfaceExternalDocument);
	useTextEditorEvents(model, reloadFiles, acceptance.surfaceExternalDocument);
	const persistViewState = useTextEditorGuards(model);
	const openActions = useTextFileOpenActions(
		model,
		reloadFiles,
		acceptance.acceptDocument,
	);
	const saveActions = useTextFileSaveActions(
		model,
		reloadFiles,
		acceptance.acceptDocument,
		acceptance.surfaceExternalDocument,
	);
	const changeText = (text: string) => {
		model.textRef.current = text;
		model.dirtyRef.current = text !== model.documentRef.current?.text;
		model.setText(text);
		model.setDirty(model.dirtyRef.current);
		if (!model.dirtyRef.current && model.externalDocumentRef.current) {
			acceptance.acceptDocument(
				model.externalDocumentRef.current,
				"Your edits now match the stored revision",
			);
		}
	};
	return {
		...model,
		...openActions,
		...saveActions,
		changeText,
		chooserFiles: chooserFiles(model),
		label: model.document?.path || model.selectedPath || "No file selected",
		persistViewState,
		reloadFiles,
		status: textEditorStatus(model),
	};
}

export type TextEditorController = ReturnType<typeof useTextEditorController>;
