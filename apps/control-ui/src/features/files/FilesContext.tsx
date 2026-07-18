import {
	createContext,
	type PropsWithChildren,
	useContext,
	useMemo,
} from "react";
import { useStableCallback } from "../shared/useStableCallback";
import type { FileCapabilities, FilesContextValue } from "./types";

const FilesContext = createContext<FilesContextValue | null>(null);

function useStableFileCapabilities(source: FileCapabilities) {
	const fileRoots = useStableCallback(source.fileRoots);
	const fileEntries = useStableCallback(source.fileEntries);
	const fileMetadata = useStableCallback(source.fileMetadata);
	const readFileNote = useStableCallback(source.readFileNote);
	const saveFileNote = useStableCallback(source.saveFileNote);
	const readTextFile = useStableCallback(source.readTextFile);
	const saveTextFile = useStableCallback(source.saveTextFile);
	const fileOperation = useStableCallback(source.fileOperation);
	const fileContent = useStableCallback(source.fileContent);
	const fileStreamUrl = useStableCallback(source.fileStreamUrl);
	const fileThumbnail = useStableCallback(source.fileThumbnail);
	const claimFileInput = useStableCallback(source.claimFileInput);
	const releaseFileInput = useStableCallback(source.releaseFileInput);
	return useMemo(
		() => ({
			fileRoots,
			fileEntries,
			fileMetadata,
			readFileNote,
			saveFileNote,
			readTextFile,
			saveTextFile,
			fileOperation,
			fileContent,
			fileStreamUrl,
			fileThumbnail,
			claimFileInput,
			releaseFileInput,
		}),
		[
			fileRoots,
			fileEntries,
			fileMetadata,
			readFileNote,
			saveFileNote,
			readTextFile,
			saveTextFile,
			fileOperation,
			fileContent,
			fileStreamUrl,
			fileThumbnail,
			claimFileInput,
			releaseFileInput,
		],
	);
}

export function FilesProvider({
	source,
	children,
}: PropsWithChildren<{ source: FilesContextValue }>) {
	const capabilities = useStableFileCapabilities(source);
	const resetCommandLine = useStableCallback(source.resetCommandLine);
	const value = useMemo(
		() => ({
			...capabilities,
			status: source.status,
			commandLine: source.commandLine,
			resetCommandLine,
			systemPickerFallback: source.systemPickerFallback,
		}),
		[
			capabilities,
			resetCommandLine,
			source.status,
			source.commandLine,
			source.systemPickerFallback,
		],
	);
	return (
		<FilesContext.Provider value={value}>{children}</FilesContext.Provider>
	);
}

export function useFiles() {
	const context = useContext(FilesContext);
	if (!context) throw new Error("useFiles must be used inside FilesProvider");
	return context;
}
