import type { FileConflictChoice } from "../../api/types";
import type { FileManagerState } from "./useFileManagerState";
import type { FileOperationActions } from "./useFileOperations";

export function useConflictActions(
	state: FileManagerState,
	operations: FileOperationActions,
) {
	async function resolveConflictChoice(choice: FileConflictChoice) {
		if (!state.conflict || state.busy) return;
		const activeOperation = state.conflict.operation;
		state.setBusy(true);
		try {
			if (
				state.conflict.applyToAll ||
				activeOperation.sources.length === 1 ||
				activeOperation.kind === "rename"
			) {
				await operations.runOperation(
					activeOperation,
					choice,
					state.conflict.applyToAll,
				);
				await operations.finishSuccessfulOperation(activeOperation);
				if (choice === "skip")
					state.setMessage(
						"Conflicting item(s) skipped; existing items were left unchanged.",
					);
				return;
			}
			const destinationNames = new Set(
				(state.listing?.entries ?? []).map((entry) =>
					entry.name.toLocaleLowerCase(),
				),
			);
			const conflicting = activeOperation.sources.filter((source) =>
				destinationNames.has(source.entry.name.toLocaleLowerCase()),
			);
			const clear = activeOperation.sources.filter(
				(source) =>
					!destinationNames.has(source.entry.name.toLocaleLowerCase()),
			);
			for (const source of clear) {
				await operations.runOperation({
					...activeOperation,
					sources: [source],
				});
			}
			const [resolved, ...remaining] = conflicting;
			if (resolved) {
				await operations.runOperation(
					{ ...activeOperation, sources: [resolved] },
					choice,
					false,
				);
			}
			if (remaining.length) {
				const next = { ...activeOperation, sources: remaining };
				operations.setOperation(next);
				state.setConflict({ operation: next, applyToAll: false });
				state.setMessage(
					`Resolved one conflict. ${remaining.length} conflict${remaining.length === 1 ? " remains" : "s remain"}.`,
				);
			} else {
				await operations.finishSuccessfulOperation(activeOperation);
				if (choice === "skip")
					state.setMessage(
						"Conflicting item skipped; the existing item was left unchanged.",
					);
			}
		} catch (error) {
			const label =
				choice === "keep_both"
					? "Keep Both"
					: choice === "replace"
						? "Replace"
						: "Skip";
			state.setMessage(`${label} failed: ${String(error)}`);
		} finally {
			state.setBusy(false);
		}
	}

	return {
		resolveReplace: () => resolveConflictChoice("replace"),
		resolveSkip: () => resolveConflictChoice("skip"),
		resolveKeepBoth: () => resolveConflictChoice("keep_both"),
	};
}

export type ConflictActions = ReturnType<typeof useConflictActions>;
