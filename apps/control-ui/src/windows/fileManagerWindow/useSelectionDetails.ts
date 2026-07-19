import { useEffect, useRef } from "react";
import { useFiles } from "../../features/files/FilesContext";
import {
	audioExtensions,
	extension,
	imageExtensions,
	releaseObjectUrl,
	safeObjectUrl,
} from "./fileUtilities";
import type { FileManagerState } from "./useFileManagerState";

export function useSelectionDetails(state: FileManagerState) {
	const server = useFiles();
	const serverRef = useRef(server);
	serverRef.current = server;
	const previewSelection =
		state.selected.length === 1 && state.selected[0].entry.kind === "file"
			? {
					rootId: state.selected[0].rootId,
					path: state.selected[0].entry.path,
					name: state.selected[0].entry.name,
				}
			: null;

	useEffect(() => {
		let cancelled = false;
		let allocated = "";
		state.setPreviewUrl((value) => {
			releaseObjectUrl(value);
			return "";
		});
		if (!previewSelection) return;
		const fileExtension = extension(previewSelection.name);
		if (
			!imageExtensions.has(fileExtension) &&
			!audioExtensions.has(fileExtension)
		)
			return;
		if (audioExtensions.has(fileExtension)) {
			void serverRef.current
				.fileStreamUrl(previewSelection.rootId, previewSelection.path)
				.then((url) => {
					if (!cancelled) state.setPreviewUrl(url);
				})
				.catch((error) =>
					state.setMessage(`Preview unavailable: ${String(error)}`),
				);
		} else {
			void serverRef.current
				.fileThumbnail(previewSelection.rootId, previewSelection.path)
				.then((blob) => {
					allocated = safeObjectUrl(blob);
					if (cancelled) releaseObjectUrl(allocated);
					else state.setPreviewUrl(allocated);
				})
				.catch((error) =>
					state.setMessage(`Preview unavailable: ${String(error)}`),
				);
		}
		return () => {
			cancelled = true;
			releaseObjectUrl(allocated);
		};
	}, [
		previewSelection?.name,
		previewSelection?.path,
		previewSelection?.rootId,
	]);

	const noteSelection =
		state.selected.length === 1 && state.selected[0].entry.note_supported
			? { rootId: state.selected[0].rootId, path: state.selected[0].entry.path }
			: null;

	useEffect(() => {
		let cancelled = false;
		state.setNativeNote(null);
		state.setNoteDraft("");
		if (!noteSelection) return;
		void serverRef.current
			.readFileNote(noteSelection.rootId, noteSelection.path)
			.then((note) => {
				if (cancelled) return;
				state.setNativeNote(note);
				state.setNoteDraft(note.note ?? "");
			})
			.catch((error) => {
				if (!cancelled) state.setMessage(`Notes unavailable: ${String(error)}`);
			});
		return () => {
			cancelled = true;
		};
	}, [noteSelection?.path, noteSelection?.rootId]);

	async function saveNativeNote() {
		if (!state.nativeNote) return;
		state.setBusy(true);
		try {
			const saved = await server.saveFileNote(
				state.nativeNote.root_id,
				state.nativeNote.path,
				state.noteDraft,
			);
			state.setNativeNote(saved);
			state.setNoteDraft(saved.note ?? "");
			state.setMessage("Native filesystem note saved.");
		} catch (error) {
			state.setMessage(`Could not save the native note: ${String(error)}`);
		} finally {
			state.setBusy(false);
		}
	}

	return { saveNativeNote };
}
