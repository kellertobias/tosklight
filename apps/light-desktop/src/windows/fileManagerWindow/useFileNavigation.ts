import { useCallback, useEffect } from "react";
import { useFiles } from "../../features/files/FilesContext";
import { sortFileEntries } from "./fileUtilities";
import type { FileManagerLocation, FileManagerPickerOptions } from "./types";
import { useFileListing } from "./useFileListing";
import { currentLocation, type FileManagerState } from "./useFileManagerState";
import { useFileRoots } from "./useFileRoots";

interface NavigationOptions {
	state: FileManagerState;
	picker?: FileManagerPickerOptions;
	hidden: boolean;
	confirmDiscardEditor: () => boolean;
}

export function useFileNavigation({
	state,
	picker,
	hidden,
	confirmDiscardEditor,
}: NavigationOptions) {
	const server = useFiles();
	const current = currentLocation(state);
	const rootId = current?.rootId ?? "";
	const currentPath = current?.path ?? "";
	const currentRoot = state.roots.find((root) => root.id === rootId) ?? null;

	const navigate = useCallback(
		(next: FileManagerLocation) => {
			if (!confirmDiscardEditor()) return;
			const retained = state.history.slice(0, state.historyIndex + 1);
			const previous = retained.at(-1);
			if (previous?.rootId === next.rootId && previous.path === next.path)
				return;
			const nextHistory = [...retained, next];
			state.setHistory(nextHistory);
			state.setHistoryIndex(nextHistory.length - 1);
			state.setSelected([]);
			state.setSelectionAnchor(null);
			state.setEditor(null);
			state.setEditorConflict(null);
			state.setSidePanel("none");
			state.setMessage("");
		},
		[state.history, state.historyIndex, confirmDiscardEditor],
	);

	const roots = useFileRoots(state, picker);

	useEffect(() => {
		if (
			!state.initialized.current ||
			!rootId ||
			state.roots.some((root) => root.id === rootId)
		)
			return;
		const fallback = state.roots[0];
		state.setMessage(`The location “${rootId}” was disconnected.`);
		if (fallback) {
			state.setHistory((value) => [
				...value.slice(0, state.historyIndex + 1),
				{ rootId: fallback.id, path: "" },
			]);
			state.setHistoryIndex((value) => value + 1);
		}
	}, [state.historyIndex, rootId, state.roots]);

	const refresh = useFileListing(state, { rootId, currentPath, hidden });

	useEffect(() => {
		state.setTreeChildren({});
		state.setTreeExpanded(new Set());
	}, [hidden]);

	const loadTreeFolder = useCallback(
		async (location: FileManagerLocation) => {
			const key = `${location.rootId}:${location.path}`;
			if (state.treeExpanded.has(key)) {
				state.setTreeExpanded((values) => {
					const next = new Set(values);
					next.delete(key);
					return next;
				});
				return;
			}
			state.setTreeExpanded((values) => new Set(values).add(key));
			if (state.treeChildren[key]) return;
			try {
				const contents = await server.fileEntries(
					location.rootId,
					location.path,
					hidden,
				);
				state.setTreeChildren((values) => ({
					...values,
					[key]: sortFileEntries(contents.entries).filter(
						(entry) => entry.kind === "folder",
					),
				}));
			} catch (error) {
				state.setMessage(`Could not expand folder: ${String(error)}`);
			}
		},
		[hidden, server.fileEntries, state.treeChildren, state.treeExpanded],
	);

	const refreshAfterMutation = useCallback(async () => {
		state.setSelected([]);
		state.setTreeChildren({});
		state.setTreeExpanded(new Set());
		await refresh();
	}, [refresh]);

	return {
		...roots,
		current,
		rootId,
		currentPath,
		currentRoot,
		navigate,
		refresh,
		refreshAfterMutation,
		loadTreeFolder,
	};
}

export type FileNavigation = ReturnType<typeof useFileNavigation>;
