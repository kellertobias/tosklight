import { useOptionalApp } from "../../state/AppContext";
import { pickerSelectionIsValid, selectionKey } from "./fileUtilities";
import type { FileManagerProps } from "./types";
import { useConflictActions } from "./useConflictActions";
import { operationLabel, useFileManagerState } from "./useFileManagerState";
import { useFileNavigation } from "./useFileNavigation";
import { useFileOperationInput } from "./useFileOperationInput";
import { useFileOperationActions } from "./useFileOperations";
import { useSelectionDetails } from "./useSelectionDetails";
import { confirmDiscardEditor, useTextFileEditor } from "./useTextFileEditor";

export function useFileManagerController({
	picker,
	instanceId: suppliedInstanceId,
	paneId,
	closeable = false,
	purpose = "Browse and manage files",
}: FileManagerProps) {
	const app = useOptionalApp();
	const state = useFileManagerState(suppliedInstanceId);
	const pane = app?.state.desks
		.flatMap((desk) => desk.panes)
		.find((candidate) => candidate.id === paneId);
	const hidden = paneId
		? Boolean(pane?.fileManagerShowHidden)
		: state.localHidden;
	const setHidden = (value: boolean) => {
		if (paneId && app) {
			app.dispatch({ type: "SET_FILE_MANAGER_SHOW_HIDDEN", id: paneId, value });
		} else state.setLocalHidden(value);
	};
	const navigation = useFileNavigation({
		state,
		picker,
		hidden,
		confirmDiscardEditor: () => confirmDiscardEditor(state),
	});
	const details = useSelectionDetails(state);
	const editor = useTextFileEditor(
		state,
		paneId,
		navigation.refreshAfterMutation,
	);
	const operations = useFileOperationActions(state, navigation, picker);
	const conflicts = useConflictActions(state, operations);
	const pickerValid = Boolean(
		picker && pickerSelectionIsValid(state.selected, picker),
	);
	useFileOperationInput(state, operations, picker, pickerValid);

	const breadcrumbs = navigation.currentPath
		? navigation.currentPath.split("/")
		: [];
	const label = operationLabel(state.operation);
	const trashForOperation = Boolean(
		state.operation?.kind === "delete" &&
			state.operation.sources.length &&
			state.operation.sources.every((source) => source.entry.trash_supported),
	);

	return {
		app,
		state,
		navigation,
		details,
		editor,
		operations,
		conflicts,
		picker,
		paneId,
		closeable,
		purpose,
		hidden,
		setHidden,
		pickerValid,
		breadcrumbs,
		operationLabel: label,
		trashForOperation,
		sourceKeys: new Set(state.operation?.sources.map(selectionKey) ?? []),
		selectedKeys: new Set(state.selected.map(selectionKey)),
	};
}

export type FileManagerController = ReturnType<typeof useFileManagerController>;
