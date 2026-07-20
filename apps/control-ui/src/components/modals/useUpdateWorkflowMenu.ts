import { useCallback, useEffect, useRef, useState } from "react";
import type { UpdateMode, UpdateTargetFilter } from "../../api/types";
import type {
	ProgrammingUpdateCapability,
	ProgrammingUpdateMenuEntry,
} from "../../features/programmingUpdate/contracts";
import { updateTargetKey } from "../control/updateWorkflow";

interface UpdateWorkflowMenuOptions {
	update: ProgrammingUpdateCapability | null;
	scopeKey: string;
	setBusy: (busy: boolean) => void;
	setLocalError: (error: string | null) => void;
}

export function useUpdateWorkflowMenu({
	update,
	scopeKey,
	setBusy,
	setLocalError,
}: UpdateWorkflowMenuOptions) {
	const [open, setOpen] = useState(false);
	const [filter, setFilter] = useState<UpdateTargetFilter>(
		"eligible_for_update_existing",
	);
	const [entries, setEntries] = useState<ProgrammingUpdateMenuEntry[]>([]);
	const [modes, setModes] = useState<Record<string, UpdateMode>>({});
	const [busyKey, setBusyKey] = useState<string | null>(null);
	const request = useRef(0);
	const scopeKeyRef = useRef(scopeKey);
	scopeKeyRef.current = scopeKey;

	useEffect(() => {
		request.current += 1;
		setBusyKey(null);
		setEntries([]);
		setOpen(false);
	}, [scopeKey]);

	const load = useCallback(
		async (nextFilter: UpdateTargetFilter) => {
			const requestId = ++request.current;
			const requestedScope = scopeKey;
			setFilter(nextFilter);
			setBusy(true);
			setLocalError(null);
			try {
				const authority = await update?.targets(nextFilter);
				if (!isCurrent(requestId, request.current, requestedScope, scopeKeyRef))
					return;
				setBusy(false);
				if (!authority)
					return setLocalError("Eligible Update targets could not be loaded.");
				setEntries(authority.entries);
				setModes(initialModes(authority.entries));
			} catch (reason) {
				if (!isCurrent(requestId, request.current, requestedScope, scopeKeyRef))
					return;
				setBusy(false);
				setLocalError(errorMessage(reason));
			}
		},
		[scopeKey, setBusy, setLocalError, update],
	);

	const setMode = (key: string, mode: UpdateMode) =>
		setModes((current) => ({ ...current, [key]: mode }));
	const close = () => {
		setOpen(false);
		setLocalError(null);
	};

	return {
		open,
		setOpen,
		filter,
		entries,
		modes,
		busyKey,
		setBusyKey,
		load,
		setMode,
		close,
	};
}

function initialModes(entries: ProgrammingUpdateMenuEntry[]) {
	return Object.fromEntries(
		entries.map((entry) => [
			updateTargetKey(entry.target),
			entry.existing_preview.mode,
		]),
	);
}

function isCurrent(
	request: number,
	currentRequest: number,
	requestedScope: string,
	scopeKeyRef: { current: string },
) {
	return request === currentRequest && requestedScope === scopeKeyRef.current;
}

function errorMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
