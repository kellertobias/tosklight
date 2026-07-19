import { useCallback, useEffect } from "react";
import { useFiles } from "../../features/files/FilesContext";
import { sortFileEntries } from "./fileUtilities";
import type { FileManagerLocation, FileManagerPickerOptions } from "./types";
import { currentLocation, type FileManagerState } from "./useFileManagerState";

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

	const loadRoots = useCallback(async () => {
		try {
			const items = await server.fileRoots();
			state.setRoots(items);
			if (!state.initialized.current && items.length) {
				state.initialized.current = true;
				const requested = picker?.initialRootId;
				const initialRoot =
					items.find((root) => root.id === requested) ?? items[0];
				state.setHistory([
					{ rootId: initialRoot.id, path: picker?.initialDirectory ?? "" },
				]);
				state.setHistoryIndex(0);
			}
		} catch (error) {
			state.setMessage(`Locations unavailable: ${String(error)}`);
		}
	}, [server.fileRoots, picker?.initialDirectory, picker?.initialRootId]);

	useEffect(() => {
		void loadRoots();
		const timer = window.setInterval(() => void loadRoots(), 5000);
		return () => window.clearInterval(timer);
	}, [loadRoots]);

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

	const refresh = useCallback(async () => {
		if (!rootId) return;
		try {
			const next = await server.fileEntries(rootId, currentPath, hidden);
			state.setListing({ ...next, entries: sortFileEntries(next.entries) });
			state.setMessage((value) =>
				value.startsWith("Could not open this location:") ? "" : value,
			);
		} catch (error) {
			state.setListing(null);
			state.setMessage(`Could not open this location: ${String(error)}`);
		}
	}, [currentPath, server.fileEntries, hidden, rootId]);

	useEffect(() => {
		void refresh();
	}, [refresh]);

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
