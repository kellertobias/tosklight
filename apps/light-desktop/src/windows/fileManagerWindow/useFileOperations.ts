import type {
	MouseEvent as ReactMouseEvent,
	PointerEvent as ReactPointerEvent,
} from "react";
import { useCallback } from "react";
import type { FileConflictChoice, FileEntry } from "../../api/types";
import { useFiles } from "../../features/files/FilesContext";
import type { FilesContextValue } from "../../features/files/types";
import { publishTextFileOperation } from "../textFileSync";
import {
	assertFileOperationComplete,
	operationFromCommandLine,
	selectionKey,
	validItemName,
} from "./fileUtilities";
import { emptyOperation, fileOperationOwnership } from "./operationOwnership";
import type {
	FileManagerOperationKind,
	FileManagerPickerOptions,
	FileManagerSelection,
	FileOperationState,
} from "./types";
import type { FileManagerState } from "./useFileManagerState";
import type { FileNavigation } from "./useFileNavigation";
import { useCommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";

type CommandLineSurface = Pick<
	ReturnType<typeof useCommandLineSurface>,
	"read" | "reset"
>;

interface OperationExecutionContext {
	server: FilesContextValue;
	rootId: string;
	currentPath: string;
	instanceId: string;
	setMessage: (message: string) => void;
}

async function runOperation(
	context: OperationExecutionContext,
	activeOperation: FileOperationState,
	conflictChoice?: FileConflictChoice,
	applyToAll = false,
) {
	if (!activeOperation.sources.length) {
		context.setMessage("Select at least one source item.");
		return;
	}
	const sourceRoot = activeOperation.sources[0].rootId;
	if (activeOperation.sources.some((source) => source.rootId !== sourceRoot)) {
		context.setMessage(
			"A single operation cannot mix sources from different roots.",
		);
		return;
	}
	if (activeOperation.kind === "rename") {
		if (!validItemName(activeOperation.renameDraft)) {
			context.setMessage(
				"Names may not be empty, dot paths, or contain path separators.",
			);
			return;
		}
		const result = await context.server.fileOperation(sourceRoot, {
			operation: "rename",
			sources: [activeOperation.sources[0].entry.path],
			name: activeOperation.renameDraft.trim(),
			conflict: conflictChoice,
			apply_to_all: applyToAll,
		});
		publishTextFileOperation("rename", result, context.instanceId);
		assertFileOperationComplete(result);
		return;
	}
	if (activeOperation.kind === "delete") {
		const useTrash = activeOperation.sources.every(
			(source) => source.entry.trash_supported,
		);
		const operation = useTrash ? "trash" : "delete";
		const result = await context.server.fileOperation(sourceRoot, {
			operation,
			sources: activeOperation.sources.map((source) => source.entry.path),
		});
		publishTextFileOperation(operation, result, context.instanceId);
		assertFileOperationComplete(result);
		return;
	}
	const result = await context.server.fileOperation(sourceRoot, {
		operation: activeOperation.kind,
		sources: activeOperation.sources.map((source) => source.entry.path),
		destination: context.currentPath,
		destination_root_id: context.rootId,
		conflict: conflictChoice,
		apply_to_all: applyToAll,
	});
	publishTextFileOperation(activeOperation.kind, result, context.instanceId);
	assertFileOperationComplete(result);
}

type SetOperation = (next: FileOperationState | null) => void;

function useOperationOwnershipActions(
	state: FileManagerState,
	server: FilesContextValue,
	commandLine: CommandLineSurface,
	setOperation: SetOperation,
	picker?: FileManagerPickerOptions,
) {
	function cancelOperation(reason = "File operation cancelled.") {
		state.setConflict(null);
		setOperation(null);
		if (fileOperationOwnership.claimed === state.instanceId) {
			fileOperationOwnership.claimed = null;
			void server.releaseFileInput(state.instanceId).catch(() => undefined);
		}
		state.setMessage(reason);
	}

	function beginOperation(
		kind: FileManagerOperationKind,
		sources = state.selected,
	) {
		if ((kind === "rename" || kind === "delete") && !sources.length) {
			state.setMessage(
				`${kind === "rename" ? "Rename" : "Delete"} requires a selection.`,
			);
			return;
		}
		if (kind === "rename" && sources.length !== 1) {
			state.setMessage("Rename requires exactly one selected item.");
			return;
		}
		fileOperationOwnership.claimed = state.instanceId;
		setOperation(emptyOperation(kind, sources));
		state.setConflict(null);
		state.setMessage(
			kind === "copy" || kind === "move"
				? `${kind === "copy" ? "Copy" : "Move"} is ready. Select sources, choose a destination, then press ENTER.`
				: "",
		);
		void server
			.claimFileInput(state.instanceId, kind, "toolbar")
			.catch((error) => {
				if (fileOperationOwnership.claimed === state.instanceId)
					fileOperationOwnership.claimed = null;
				setOperation(null);
				state.setMessage(
					`Could not claim File Manager input: ${String(error)}`,
				);
			});
	}

	function claimPendingAction(event: ReactPointerEvent<HTMLElement>) {
		if (state.operationRef.current || picker) return;
		const pending =
			fileOperationOwnership.pending ??
			operationFromCommandLine(commandLine.read().text);
		if (!pending) return;
		event.stopPropagation();
		fileOperationOwnership.pending = null;
		fileOperationOwnership.claimed = state.instanceId;
		setOperation(emptyOperation(pending));
		state.setMessage(
			`${pending === "rename" ? "Rename" : pending[0].toUpperCase() + pending.slice(1)} claimed by this File Manager. Select the source.`,
		);
		void server
			.claimFileInput(state.instanceId, pending, "pending")
			.then(() => commandLine.reset())
			.catch((error) => {
				if (fileOperationOwnership.claimed === state.instanceId)
					fileOperationOwnership.claimed = null;
				fileOperationOwnership.pending = pending;
				setOperation(null);
				state.setMessage(
					`Could not claim the pending desk action: ${String(error)}`,
				);
			});
	}

	return { cancelOperation, beginOperation, claimPendingAction };
}

function useOperationSelectionActions(
	state: FileManagerState,
	navigation: FileNavigation,
	server: FilesContextValue,
	setOperation: SetOperation,
	picker?: FileManagerPickerOptions,
) {
	function selectEntry(
		item: FileEntry,
		event: ReactMouseEvent<HTMLButtonElement>,
	) {
		const value = { rootId: navigation.rootId, entry: item };
		const activeOperation = state.operationRef.current;
		if (activeOperation) {
			const key = selectionKey(value);
			const sources =
				activeOperation.kind === "rename"
					? [value]
					: activeOperation.sources.some(
								(source) => selectionKey(source) === key,
							)
						? activeOperation.sources.filter(
								(source) => selectionKey(source) !== key,
							)
						: [...activeOperation.sources, value];
			setOperation({
				...activeOperation,
				sources,
				renameDraft:
					activeOperation.kind === "rename"
						? item.name
						: activeOperation.renameDraft,
				confirming: false,
			});
			state.setSelected(
				sources.filter((source) => source.rootId === navigation.rootId),
			);
			state.setSelectionAnchor(item.path);
			return;
		}
		selectRegularEntry(state, picker, navigation.rootId, item, value, event);
	}

	async function create(folder: boolean) {
		const name = prompt(folder ? "Folder name" : "File name")?.trim();
		if (!name) return;
		if (!validItemName(name)) {
			state.setMessage(
				"Names may not be empty, dot paths, or contain path separators.",
			);
			return;
		}
		state.setBusy(true);
		try {
			await server.fileOperation(navigation.rootId, {
				operation: folder ? "create_folder" : "create_file",
				destination: navigation.currentPath,
				name,
			});
			state.setMessage(`${folder ? "Folder" : "File"} created.`);
			await navigation.refreshAfterMutation();
		} catch (error) {
			state.setMessage(
				`Could not create ${folder ? "folder" : "file"}: ${String(error)}`,
			);
		} finally {
			state.setBusy(false);
		}
	}

	return { selectEntry, create };
}

function useOperationExecutionActions(
	state: FileManagerState,
	navigation: FileNavigation,
	server: FilesContextValue,
	setOperation: SetOperation,
) {
	const execution = {
		server,
		rootId: navigation.rootId,
		currentPath: navigation.currentPath,
		instanceId: state.instanceId,
		setMessage: state.setMessage,
	};
	async function finishSuccessfulOperation(
		activeOperation: FileOperationState,
	) {
		state.setConflict(null);
		setOperation(null);
		if (fileOperationOwnership.claimed === state.instanceId) {
			fileOperationOwnership.claimed = null;
			void server.releaseFileInput(state.instanceId).catch(() => undefined);
		}
		const label =
			activeOperation.kind === "delete"
				? "Delete"
				: activeOperation.kind[0].toUpperCase() + activeOperation.kind.slice(1);
		state.setMessage(`${label} completed.`);
		await navigation.refreshAfterMutation();
	}
	async function completeOperation() {
		const activeOperation = state.operationRef.current;
		if (!activeOperation || state.busy) return;
		if (!activeOperation.sources.length) {
			state.setMessage("Select at least one source item.");
			return;
		}
		if (activeOperation.kind === "delete" && !activeOperation.confirming) {
			setOperation({ ...activeOperation, confirming: true });
			return;
		}
		state.setBusy(true);
		try {
			await runOperation(execution, activeOperation);
			await finishSuccessfulOperation(activeOperation);
		} catch (error) {
			const reason = String(error);
			if (
				/409|already exist|conflict/i.test(reason) &&
				activeOperation.kind !== "delete"
			) {
				state.setConflict({ operation: activeOperation, applyToAll: false });
			} else state.setMessage(`File operation failed: ${reason}`);
		} finally {
			state.setBusy(false);
		}
	}
	return {
		completeOperation,
		runOperation: (
			operation: FileOperationState,
			choice?: FileConflictChoice,
			applyToAll?: boolean,
		) => runOperation(execution, operation, choice, applyToAll),
		finishSuccessfulOperation,
	};
}

export function useFileOperationActions(
	state: FileManagerState,
	navigation: FileNavigation,
	picker?: FileManagerPickerOptions,
	enabled = true,
) {
	const server = useFiles();
	const commandLine = useCommandLineSurface({ enabled, observeCommand: false });
	const setOperation = useCallback((next: FileOperationState | null) => {
		state.operationRef.current = next;
		state.setOperationState(next);
	}, []);
	const ownership = useOperationOwnershipActions(
		state,
		server,
		commandLine,
		setOperation,
		picker,
	);
	const selection = useOperationSelectionActions(
		state,
		navigation,
		server,
		setOperation,
		picker,
	);
	const execution = useOperationExecutionActions(
		state,
		navigation,
		server,
		setOperation,
	);
	return { setOperation, ...ownership, ...selection, ...execution };
}

function selectRegularEntry(
	state: FileManagerState,
	picker: FileManagerPickerOptions | undefined,
	rootId: string,
	item: FileEntry,
	value: FileManagerSelection,
	event: ReactMouseEvent<HTMLButtonElement>,
) {
	const multiple = picker?.multiple ?? true;
	const toggle = multiple && (event.metaKey || event.ctrlKey);
	if (multiple && event.shiftKey && state.selectionAnchor && state.listing) {
		const ordered = state.listing.entries;
		const first = ordered.findIndex(
			(entry) => entry.path === state.selectionAnchor,
		);
		const last = ordered.findIndex((entry) => entry.path === item.path);
		if (first >= 0 && last >= 0) {
			const [start, end] = first < last ? [first, last] : [last, first];
			state.setSelected(
				ordered.slice(start, end + 1).map((entry) => ({ rootId, entry })),
			);
			return;
		}
	}
	if (toggle) {
		const key = selectionKey(value);
		state.setSelected((values) =>
			values.some((candidate) => selectionKey(candidate) === key)
				? values.filter((candidate) => selectionKey(candidate) !== key)
				: [...values, value],
		);
	} else state.setSelected([value]);
	state.setSelectionAnchor(item.path);
}

export type FileOperationActions = ReturnType<typeof useFileOperationActions>;
