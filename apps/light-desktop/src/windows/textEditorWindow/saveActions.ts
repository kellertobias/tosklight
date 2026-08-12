import type { TextDocument } from "../../api/types";
import { openFileManagerPicker } from "../FileManagerPickerHost";
import { publishTextFileSaved } from "../textFileSync";
import type { ReloadTextFiles } from "./catalog";
import {
	friendlyError,
	isMissingError,
	isSameDocumentVersion,
	isSupportedTextFile,
	parentDirectory,
	TEXT_FILE_EXTENSIONS,
} from "./files";
import type { TextEditorState } from "./state";

export function useTextFileSaveActions(
	model: TextEditorState,
	reloadFiles: ReloadTextFiles,
	acceptDocument: (next: TextDocument, message?: string) => void,
	surfaceExternalDocument: (next: TextDocument, source: string) => void,
) {
	const saveTo = async (
		root: string,
		path: string,
		revision: string | null,
		associationChanges: boolean,
		successMessage: string,
	) => {
		if (!root || model.saving) return;
		model.setSaving(true);
		try {
			const next = await model.serverRef.current.saveTextFile(
				root,
				path,
				model.textRef.current,
				revision,
			);
			acceptDocument(next, successMessage);
			if (associationChanges && model.paneId) {
				model.dispatch({
					type: "SET_TEXT_EDITOR_FILE",
					id: model.paneId,
					root,
					path: next.path,
				});
			}
			publishTextFileSaved(next, model.paneId);
			void reloadFiles(root);
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
		void saveTo(
			current.root_id,
			current.path,
			current.revision,
			false,
			"Saved",
		);
	};
	const saveAs = async () => {
		if (!model.selectedRoot || model.paneReadOnly) return;
		const result = await openFileManagerPicker({
			purpose: "Save text as",
			target: "files",
			multiple: false,
			allowedExtensions: [...TEXT_FILE_EXTENSIONS],
			initialRootId: model.selectedRoot,
			initialDirectory: parentDirectory(model.selectedPath),
			selectLabel: "Save As",
		});
		if (!result) return;
		if (!Array.isArray(result)) {
			model.setNotice({
				kind: "error",
				text: "Save As requires a file in the ToskLight File Manager.",
			});
			return;
		}
		const selected = result[0];
		const path = selected?.entry.path;
		if (!selected || !path) return;
		if (!isSupportedTextFile(path)) {
			model.setNotice({
				kind: "error",
				text: "Text Editor supports .txt, .md, .csv, and .log files.",
			});
			return;
		}
		let revision: string;
		try {
			const destination = await model.serverRef.current.readTextFile(
				selected.rootId,
				path,
			);
			if (!window.confirm(`Replace ${path} with this text?`)) return;
			revision = destination.revision;
		} catch (error) {
			model.setNotice({
				kind: "error",
				text: `Could not prepare ${path} for Save As: ${friendlyError(error)}`,
			});
			return;
		}
		void saveTo(
			selected.rootId,
			path,
			revision,
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
		void saveTo(
			model.selectedRoot,
			model.selectedPath,
			null,
			false,
			"File recreated",
		);
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
