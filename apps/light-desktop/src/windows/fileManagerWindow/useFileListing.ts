import { useCallback, useEffect, useRef } from "react";
import { useFiles } from "../../features/files/FilesContext";
import { sortFileEntries } from "./fileUtilities";
import type { FileManagerState } from "./useFileManagerState";

interface ShownLocation {
	rootId: string | null;
	currentPath: string;
	hidden: boolean;
}

const sameLocation = (left: ShownLocation, right: ShownLocation) =>
	left.rootId === right.rootId &&
	left.currentPath === right.currentPath &&
	left.hidden === right.hidden;

/**
 * Keeps the shown listing about the folder the operator is actually in.
 *
 * An operation's refresh and the navigation that follows it race, and a late answer for a folder
 * the operator has already left would otherwise appear under this folder's breadcrumb.
 */
export function useFileListing(
	state: FileManagerState,
	location: ShownLocation,
) {
	const server = useFiles();
	const { rootId, currentPath, hidden } = location;
	const shown = useRef(location);
	shown.current = location;
	const refresh = useCallback(async () => {
		if (!rootId) return;
		const asked = { rootId, currentPath, hidden };
		try {
			const next = await server.fileEntries(rootId, currentPath, hidden);
			if (!sameLocation(shown.current, asked)) return;
			state.setListing({ ...next, entries: sortFileEntries(next.entries) });
			state.setMessage((value) =>
				value.startsWith("Could not open this location:") ? "" : value,
			);
		} catch (error) {
			if (!sameLocation(shown.current, asked)) return;
			state.setListing(null);
			state.setMessage(`Could not open this location: ${String(error)}`);
		}
	}, [currentPath, server.fileEntries, hidden, rootId]);
	useEffect(() => {
		void refresh();
	}, [refresh]);
	return refresh;
}
