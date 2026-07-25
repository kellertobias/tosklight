import type { TextDocument } from "../../api/types";
import { openFileManagerPicker } from "../FileManagerPickerHost";
import { publishTextFileSaved } from "../textFileSync";
import type { ReloadTextFiles } from "./catalog";
import {
	friendlyError,
	MAX_TEXT_FILE_BYTES,
	parentDirectory,
	TEXT_FILE_EXTENSIONS,
} from "./files";
import type { TextEditorState } from "./state";

export function useTextFileOpenActions(
	model: TextEditorState,
	reloadFiles: ReloadTextFiles,
	acceptDocument: (next: TextDocument, message?: string) => void,
) {
	const confirmDiscard = () =>
		!model.dirtyRef.current || window.confirm("Discard unsaved changes?");
	const associateFile = (root: string, path: string) => {
		if (!confirmDiscard()) return;
		if (model.paneId) {
			model.dispatch({
				type: "SET_TEXT_EDITOR_FILE",
				id: model.paneId,
				root,
				path,
			});
		}
	};
	const openFile = async () => {
		const result = await openFileManagerPicker({
			purpose: "Open a text file",
			target: "files",
			multiple: false,
			allowedExtensions: [...TEXT_FILE_EXTENSIONS],
			initialRootId: model.selectedRoot || undefined,
			initialDirectory: parentDirectory(model.selectedPath),
		});
		if (!result || !confirmDiscard()) return;
		if (Array.isArray(result)) {
			const selected = result[0];
			if (selected && model.paneId) {
				model.dispatch({
					type: "SET_TEXT_EDITOR_FILE",
					id: model.paneId,
					root: selected.rootId,
					path: selected.entry.path,
				});
			}
			return;
		}
		if (model.paneReadOnly) {
			model.setNotice({
				kind: "error",
				text: "This Text Editor pane is read-only and cannot import a system-picked file.",
			});
			return;
		}
		const file = result.files[0];
		const targetRoot = model.selectedRoot || model.roots[0]?.id;
		if (!file || !targetRoot) return;
		if (file.size > MAX_TEXT_FILE_BYTES) {
			model.setNotice({
				kind: "error",
				text: "Text Editor files may not exceed 4 MiB.",
			});
			return;
		}
		try {
			const text = new TextDecoder("utf-8", { fatal: true }).decode(
				await file.arrayBuffer(),
			);
			const next = await model.serverRef.current.saveTextFile(
				targetRoot,
				file.name,
				text,
				null,
			);
			acceptDocument(next, `Imported ${file.name} from the system picker.`);
			if (model.paneId) {
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
			model.setNotice({
				kind: "error",
				text: `Could not import ${file.name}: ${friendlyError(error)}`,
			});
		}
	};
	return { associateFile, openFile };
}

export type TextFileOpenActions = ReturnType<typeof useTextFileOpenActions>;
