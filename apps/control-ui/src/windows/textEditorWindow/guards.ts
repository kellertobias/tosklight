import { useCallback, useEffect } from "react";
import { registerPaneRemovalGuard } from "../../components/shell/paneRemovalGuard";
import { type LegacyTextEditorViewState, viewStateKey } from "./files";
import type { TextEditorState } from "./state";

export function useTextEditorGuards(model: TextEditorState) {
	const {
		availability,
		dirtyRef,
		dispatch,
		document,
		editorMode,
		pane,
		paneId,
		selectedPath,
		selectedRoot,
		textarea,
	} = model;
	useEffect(() => {
		const warn = (event: BeforeUnloadEvent) => {
			if (!dirtyRef.current) return;
			event.preventDefault();
			event.returnValue = "";
		};
		window.addEventListener("beforeunload", warn);
		return () => window.removeEventListener("beforeunload", warn);
	}, [dirtyRef]);

	useEffect(() => {
		if (!paneId) return;
		return registerPaneRemovalGuard(paneId, () =>
			dirtyRef.current ? "Text Editor has unsaved changes." : null,
		);
	}, [dirtyRef, paneId]);

	useEffect(() => {
		if (!selectedRoot || !selectedPath || availability !== "ready") return;
		try {
			const persisted = pane?.textEditorView;
			const saved =
				persisted?.root === selectedRoot && persisted.path === selectedPath
					? persisted
					: (JSON.parse(
							localStorage.getItem(
								viewStateKey(paneId, selectedRoot, selectedPath),
							) ?? "null",
						) as Partial<LegacyTextEditorViewState> | null);
			if (!saved || !textarea.current) return;
			const control = textarea.current;
			const start = Math.min(
				Math.max(0, saved.selectionStart ?? 0),
				control.value.length,
			);
			const end = Math.min(
				Math.max(start, saved.selectionEnd ?? start),
				control.value.length,
			);
			control.setSelectionRange(start, end);
			control.scrollTop = Math.max(0, saved.scrollTop ?? 0);
		} catch {
			// Corrupt view metadata is non-authoritative and safe to ignore.
		}
	}, [
		availability,
		document?.revision,
		editorMode,
		pane?.textEditorView,
		paneId,
		selectedPath,
		selectedRoot,
		textarea,
	]);

	return useCallback(() => {
		const control = textarea.current;
		if (!control || !selectedRoot || !selectedPath) return;
		const view = {
			root: selectedRoot,
			path: selectedPath,
			selectionStart: control.selectionStart,
			selectionEnd: control.selectionEnd,
			scrollTop: control.scrollTop,
		};
		try {
			localStorage.setItem(
				viewStateKey(paneId, selectedRoot, selectedPath),
				JSON.stringify(view),
			);
		} catch {
			// View state must never prevent editing or saving the file itself.
		}
		if (paneId) dispatch({ type: "SET_TEXT_EDITOR_VIEW", id: paneId, ...view });
	}, [dispatch, paneId, selectedPath, selectedRoot, textarea]);
}

export type PersistTextEditorView = ReturnType<typeof useTextEditorGuards>;
