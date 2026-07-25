import { useCallback, useEffect } from "react";
import { friendlyError, listTextEditorFiles } from "./files";
import type { TextEditorState } from "./state";

export function useTextFileCatalog(model: TextEditorState) {
	const {
		fileListRequest,
		selectedRoot,
		serverRef,
		setFiles,
		setFilesLoading,
		setNotice,
		setRoots,
	} = model;
	useEffect(() => {
		let cancelled = false;
		void serverRef.current
			.fileRoots()
			.then((next) => {
				if (!cancelled) setRoots(next);
			})
			.catch((error) => {
				if (!cancelled) {
					setNotice({
						kind: "error",
						text: `Could not load file locations: ${friendlyError(error)}`,
					});
				}
			});
		return () => {
			cancelled = true;
		};
	}, [serverRef, setNotice, setRoots]);

	const reloadFiles = useCallback(
		async (root = selectedRoot) => {
			const request = ++fileListRequest.current;
			if (!root) {
				setFiles([]);
				return;
			}
			setFilesLoading(true);
			try {
				const result = await listTextEditorFiles(
					serverRef.current.fileEntries,
					root,
				);
				if (request !== fileListRequest.current) return;
				setFiles(result.files);
				if (result.truncated) {
					setNotice({
						kind: "info",
						text: "The file chooser reached its safety limit. Use Save As with a root-relative path or narrow the configured location.",
					});
				}
			} catch (error) {
				if (request === fileListRequest.current) {
					setFiles([]);
					setNotice({
						kind: "error",
						text: `Could not list text files: ${friendlyError(error)}`,
					});
				}
			} finally {
				if (request === fileListRequest.current) setFilesLoading(false);
			}
		},
		[
			fileListRequest,
			selectedRoot,
			serverRef,
			setFiles,
			setFilesLoading,
			setNotice,
		],
	);

	useEffect(() => {
		void reloadFiles(selectedRoot);
	}, [reloadFiles, selectedRoot]);
	return reloadFiles;
}

export type ReloadTextFiles = ReturnType<typeof useTextFileCatalog>;
