import "./FileManagerWindow.css";
import { FileManagerView } from "./fileManagerWindow/FileManagerView";
import type { FileManagerProps } from "./fileManagerWindow/types";
import { useFileManagerController } from "./fileManagerWindow/useFileManagerController";
import type { WindowProps } from "./windowTypes";

export {
	extension,
	joinPath,
	nextKeepBothName,
	operationFromCommandLine,
	parentPath,
	pickerSelectionIsValid,
	sortFileEntries,
	validItemName,
} from "./fileManagerWindow/fileUtilities";
export type {
	FileManagerPickerOptions,
	FileManagerProps,
	FileManagerSelection,
	FileManagerTarget,
} from "./fileManagerWindow/types";

export function FileManagerWindow({ active = true, builtIn, paneId }: WindowProps) {
	return (
		<FileManager
			active={active}
			instanceId={paneId}
			paneId={paneId}
			closeable={builtIn}
		/>
	);
}

export function FileManager(props: FileManagerProps) {
	const controller = useFileManagerController(props);
	return <FileManagerView controller={controller} />;
}
