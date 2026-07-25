import { useRef } from "react";
import type { UpdateMode, UpdateResult, UpdateSettings } from "../../api/types";
import type { ProgrammingUpdateMenuEntry } from "../../features/programmingUpdate/contracts";
import { useProgrammingUpdate } from "../../features/programmingUpdate/ProgrammingUpdateProvider";
import {
	requestFromUpdateIdentity,
	updateTargetKey,
} from "../control/updateWorkflow";
import type { UpdateOperation } from "./useUpdateWorkflowEvents";

interface UpdateWorkflowActionOptions {
	operation: UpdateOperation | null;
	settings: UpdateSettings;
	disarm: () => void;
	setBusy: (busy: boolean) => void;
	setBusyKey: (key: string | null) => void;
	setLocalError: (error: string | null) => void;
	setMenuOpen: (open: boolean) => void;
	setOperation: (operation: UpdateOperation | null) => void;
	setResult: (result: UpdateResult) => void;
	setSettingsOpen: (open: boolean) => void;
}

export function useUpdateWorkflowActions({
	operation,
	settings,
	disarm,
	setBusy,
	setBusyKey,
	setLocalError,
	setMenuOpen,
	setOperation,
	setResult,
	setSettingsOpen,
}: UpdateWorkflowActionOptions) {
	const update = useProgrammingUpdate();
	const actionEpoch = useRef(0);
	const scopeKeyRef = useRef(update?.scopeKey ?? "unavailable");
	scopeKeyRef.current = update?.scopeKey ?? "unavailable";
	const begin = () => ({
		epoch: ++actionEpoch.current,
		scopeKey: scopeKeyRef.current,
	});
	const current = (attempt: ReturnType<typeof begin>) =>
		attempt.epoch === actionEpoch.current &&
		attempt.scopeKey === scopeKeyRef.current;

	const changeOperationMode = async (mode: UpdateMode) => {
		if (!operation) return;
		const attempt = begin();
		setBusy(true);
		setLocalError(null);
		try {
			const authority = await update?.preview(operation.request, mode);
			if (!current(attempt)) return;
			setBusy(false);
			if (authority)
				setOperation({
					request: requestFromUpdateIdentity(authority.preview.target),
					preview: authority.preview,
					authority,
				});
			else setLocalError("This Update mode could not be previewed.");
		} catch (reason) {
			if (!current(attempt)) return;
			setBusy(false);
			setLocalError(errorMessage(reason));
		}
	};
	const applyOperation = async () => {
		if (!operation) return;
		const attempt = begin();
		setBusy(true);
		setLocalError(null);
		try {
			const applied = await update?.confirm(operation.authority);
			if (!current(attempt)) return;
			setBusy(false);
			if (!applied) {
				setLocalError("Update failed; no show data was changed.");
				return;
			}
			setOperation(null);
			disarm();
			setResult(applied.result);
		} catch (reason) {
			if (!current(attempt)) return;
			setBusy(false);
			setLocalError(errorMessage(reason));
		}
	};
	const saveSettings = async () => {
		const attempt = begin();
		setBusy(true);
		setLocalError(null);
		try {
			const saved = await update?.saveSettings(settings);
			if (!current(attempt)) return;
			setBusy(false);
			if (saved) setSettingsOpen(false);
			else setLocalError("Update Settings were not saved.");
		} catch (reason) {
			if (!current(attempt)) return;
			setBusy(false);
			setLocalError(errorMessage(reason));
		}
	};
	const applyMenuTarget = async (
		entry: ProgrammingUpdateMenuEntry,
		mode: UpdateMode,
	) => {
		const attempt = begin();
		const key = updateTargetKey(entry.target);
		setBusyKey(key);
		setLocalError(null);
		const authority = authorityForMode(entry, mode);
		try {
			const applied = await update?.confirm(authority);
			if (!current(attempt)) return;
			setBusyKey(null);
			if (!applied) {
				setLocalError("Update failed; no show data was changed.");
				return;
			}
			setMenuOpen(false);
			setResult(applied.result);
		} catch (reason) {
			if (!current(attempt)) return;
			setBusyKey(null);
			setLocalError(errorMessage(reason));
		}
	};
	return { changeOperationMode, applyOperation, saveSettings, applyMenuTarget };
}

function authorityForMode(entry: ProgrammingUpdateMenuEntry, mode: UpdateMode) {
	const candidate = entry.addNewAuthority;
	return candidate.preview.mode.target_type === mode.target_type &&
		candidate.preview.mode.mode === mode.mode
		? candidate
		: entry.existingAuthority;
}

function errorMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
