import { useCallback } from "react";
import type { ServerState } from "./useServerState";

export function useFileAccess(state: ServerState) {
	const { client } = state;
	const fileRoots = useCallback(() => client.fileRoots(), [client]);
	const fileEntries = useCallback(
		(root: string, path?: string, hidden?: boolean) =>
			client.fileEntries(root, path, hidden),
		[client],
	);
	return { fileRoots, fileEntries };
}
