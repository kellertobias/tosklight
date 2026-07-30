import { useState } from "react";
import type { SelectiveImportCatalog } from "../../api/selectiveImportModels";
import type { ShowEntry } from "../../api/types";
import { useActiveShow } from "../../features/deskSnapshot/DeskSnapshotState";
import { useOptionalSelectiveImport } from "../../features/selectiveImport/SelectiveImportContext";
import { useShowLifecycle } from "../../features/showLifecycle/ShowLifecycleContext";
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
	active = true,
	picker,
	instanceId: suppliedInstanceId,
	paneId,
	closeable = false,
	purpose = "Browse and manage files",
}: FileManagerProps) {
	const app = useOptionalApp();
	const activeShow = useActiveShow();
	const lifecycle = useShowLifecycle();
	const selectiveImport = useOptionalSelectiveImport();
	const [partialImport, setPartialImport] = useState<{
		source: ShowEntry;
		catalog: SelectiveImportCatalog;
	} | null>(null);
	const [partialImportLoading, setPartialImportLoading] = useState(false);
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
	const operations = useFileOperationActions(state, navigation, picker, active);
	const conflicts = useConflictActions(state, operations);
	const pickerValid = Boolean(
		picker && pickerSelectionIsValid(state.selected, picker),
	);
	useFileOperationInput(state, operations, picker, pickerValid, active);
	const partialImportSource =
		!picker && activeShow && lifecycle && selectiveImport
			? selectedLibraryShow(state.selected, lifecycle.shows, activeShow.id)
			: null;
	const openPartialImport = async () => {
		if (!partialImportSource || !activeShow || !selectiveImport) return;
		setPartialImportLoading(true);
		try {
			const catalog = await selectiveImport.catalog(
				activeShow.id,
				partialImportSource.id,
			);
			setPartialImport({ source: partialImportSource, catalog });
		} finally {
			setPartialImportLoading(false);
		}
	};

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
		partialImport: {
			activeShow,
			capability: selectiveImport,
			source: partialImportSource,
			dialog: partialImport,
			loading: partialImportLoading,
			open: openPartialImport,
			close: () => setPartialImport(null),
		},
	};
}

export type FileManagerController = ReturnType<typeof useFileManagerController>;

function selectedLibraryShow(
	selection: ReturnType<typeof useFileManagerState>["selected"],
	shows: ShowEntry[],
	activeShowId: string,
) {
	if (selection.length !== 1 || selection[0].rootId !== "shows") return null;
	const entry = selection[0].entry;
	if (entry.kind !== "file" || !entry.name.toLowerCase().endsWith(".show"))
		return null;
	return (
		shows.find(
			(show) =>
				show.id !== activeShowId &&
				show.path.replaceAll("\\", "/").split("/").at(-1) === entry.name,
		) ?? null
	);
}
