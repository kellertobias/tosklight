import { useCallback } from "react";
import type { ServerState } from "./useServerState";

export function useFileAccess(state: ServerState) {
	const { api } = state;
	const fileRoots = useCallback(() => api.files.fileRoots(), [api]);
	const fileEntries = useCallback(
		(root: string, path?: string, hidden?: boolean) =>
			api.files.fileEntries(root, path, hidden),
		[api],
	);
	return { fileRoots, fileEntries };
}
