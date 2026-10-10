import { useCallback, useEffect, useRef, useState } from "react";
import { useFiles } from "../../features/files/FilesContext";
import type { FileManagerPickerOptions } from "./types";
import type { FileManagerState } from "./useFileManagerState";

export function useFileRoots(
	state: FileManagerState,
	picker?: FileManagerPickerOptions,
) {
	const server = useFiles();
	const [rootsStatus, setRootsStatus] = useState<
		"loading" | "ready" | "failed"
	>("loading");
	const request = useRef(0);
	const pending = useRef(false);
	const loadRoots = useCallback(
		async (showLoading = false) => {
			if (pending.current && !showLoading) return;
			const asked = ++request.current;
			pending.current = true;
			if (showLoading) setRootsStatus("loading");
			try {
				const items = await server.fileRoots();
				if (request.current !== asked) return;
				state.setRoots(items);
				setRootsStatus("ready");
				if (!state.initialized.current && items.length) {
					state.initialized.current = true;
					const initialRoot =
						items.find((root) => root.id === picker?.initialRootId) ?? items[0];
					state.setHistory([
						{ rootId: initialRoot.id, path: picker?.initialDirectory ?? "" },
					]);
					state.setHistoryIndex(0);
				}
			} catch {
				if (request.current === asked) setRootsStatus("failed");
			} finally {
				if (request.current === asked) pending.current = false;
			}
		},
		[server.fileRoots, picker?.initialDirectory, picker?.initialRootId],
	);

	useEffect(() => {
		void loadRoots(true);
		const timer = window.setInterval(() => void loadRoots(), 5000);
		return () => {
			++request.current;
			pending.current = false;
			window.clearInterval(timer);
		};
	}, [loadRoots]);
	return { rootsStatus, retryRoots: () => void loadRoots(true) };
}
