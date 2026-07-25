import type { TextDocument } from "../../api/types";
import { publishTextFileSaved } from "../textFileSync";
import type { ReloadTextFiles } from "./catalog";
import {
	friendlyError,
	isMissingError,
	isSameDocumentVersion,
	isSupportedTextFile,
} from "./files";
import type { TextEditorState } from "./state";

export function useTextFileSaveActions(
	model: TextEditorState,
	reloadFiles: ReloadTextFiles,
	acceptDocument: (next: TextDocument, message?: string) => void,
	surfaceExternalDocument: (next: TextDocument, source: string) => void,
) {
	const saveTo = async (
		path: string,
		revision: string | null,
		associationChanges: boolean,
		successMessage: string,
	) => {
		const targetRoot = model.documentRef.current?.root_id ?? model.selectedRoot;
		if (!targetRoot || model.saving) return;
		model.setSaving(true);
		try {
			const next = await model.serverRef.current.saveTextFile(
				targetRoot,
				path,
				model.textRef.current,
				revision,
			);
			acceptDocument(next, successMessage);
			if (associationChanges && model.paneId) {
				model.dispatch({
					type: "SET_TEXT_EDITOR_FILE",
					id: model.paneId,
					root: targetRoot,
					path: next.path,
				});
			}
			publishTextFileSaved(next, model.paneId);
			void reloadFiles(targetRoot);
		} catch (error) {
			if (
				revision &&
				(await surfaceSaveConflict(model, path, surfaceExternalDocument))
			) {
				return;
			}
			model.setNotice({
				kind: "error",
				text: `Save failed: ${friendlyError(error)}`,
			});
		} finally {
			model.setSaving(false);
		}
	};
	const save = () => {
		const current = model.documentRef.current;
		if (
			!current ||
			model.paneReadOnly ||
			current.read_only ||
			model.externalDocumentRef.current ||
			model.availability === "missing"
		) {
			return;
		}
		void saveTo(current.path, current.revision, false, "Saved");
	};
	const saveAs = () => {
		if (!model.selectedRoot || model.paneReadOnly) return;
		const suggested = model.selectedPath || "operator-notes.txt";
		const path = window
			.prompt("Save as path (relative to this file location)", suggested)
			?.trim();
		if (!path) return;
		if (!isSupportedTextFile(path)) {
			model.setNotice({
				kind: "error",
				text: "Text Editor supports .txt, .md, .csv, and .log files.",
			});
			return;
		}
		void saveTo(
			path,
			null,
			path !== model.selectedPath,
			path === model.selectedPath ? "File recreated" : `Saved as ${path}`,
		);
	};
	const recreate = () => {
		if (
			model.paneReadOnly ||
			!model.selectedPath ||
			!window.confirm(
				`Recreate ${model.selectedPath} from the text retained in this window?`,
			)
		) {
			return;
		}
		void saveTo(model.selectedPath, null, false, "File recreated");
	};
	const reloadExternal = () => {
		const latest = model.externalDocumentRef.current;
		if (!latest) return;
		if (
			model.dirtyRef.current &&
			!window.confirm("Discard your unsaved version and load the newer file?")
		) {
			return;
		}
		acceptDocument(latest, "Reloaded the newer file");
	};
	return { recreate, reloadExternal, save, saveAs };
}

async function surfaceSaveConflict(
	model: TextEditorState,
	path: string,
	surfaceExternalDocument: (next: TextDocument, source: string) => void,
) {
	try {
		const latest = await model.serverRef.current.readTextFile(
			model.selectedRoot,
			path,
		);
		if (!isSameDocumentVersion(model.documentRef.current, latest)) {
			surfaceExternalDocument(latest, "Another editor or external program");
			return true;
		}
	} catch (error) {
		if (isMissingError(error)) {
			model.availabilityRef.current = "missing";
			model.setAvailability("missing");
			model.setNotice({
				kind: "error",
				text: "The file was removed before it could be saved. Your unsaved text is preserved; recreate it or save a copy.",
			});
			return true;
		}
	}
	return false;
}

export type TextFileSaveActions = ReturnType<typeof useTextFileSaveActions>;
