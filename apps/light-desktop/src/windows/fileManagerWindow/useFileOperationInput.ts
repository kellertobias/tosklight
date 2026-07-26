import { useEffect, useRef } from "react";
import {
	type ControlSurfaceIntent,
	registerControlSurfaceTarget,
} from "../../features/controlSurfaceInteraction/registry";
import { useControlSurfaceTarget } from "../../features/controlSurfaceInteraction/useControlSurfaceTarget";
import { useFiles } from "../../features/files/FilesContext";
import type { FilesContextValue } from "../../features/files/types";
import { useConnectionStatus } from "../../features/shellStatus/ShellStatusState";
import { fileOperationOwnership } from "./operationOwnership";
import type { FileManagerPickerOptions } from "./types";
import type { FileManagerState } from "./useFileManagerState";
import type { FileOperationActions } from "./useFileOperations";

let pendingOwnerUsers = 0;
let releasePendingOwner: (() => void) | null = null;

function retainPendingFileOperationOwner() {
	pendingOwnerUsers += 1;
	if (pendingOwnerUsers === 1)
		releasePendingOwner = registerControlSurfaceTarget({
			id: "file-manager:pending-operation",
			priority: 400,
			accepts: isPendingFileOperationIntent,
			handle: (intent) => {
				if (intent.type !== "set" && intent.type !== "file_operation_key")
					return;
				const action = intent.type === "set" ? "rename" : intent.action;
				if (
					action === "rename" ||
					action === "copy" ||
					action === "move" ||
					action === "delete"
				)
					fileOperationOwnership.pending = action;
			},
		});
	return () => {
		pendingOwnerUsers -= 1;
		if (pendingOwnerUsers !== 0) return;
		releasePendingOwner?.();
		releasePendingOwner = null;
	};
}

function isPendingFileOperationIntent(intent: ControlSurfaceIntent) {
	return (
		intent.type === "set" ||
		(intent.type === "file_operation_key" &&
			intent.action !== "escape" &&
			intent.action !== "enter")
	);
}

function useFileOperationRouting(
	state: FileManagerState,
	operations: FileOperationActions,
	server: FilesContextValue,
	enabled: boolean,
) {
	const connectionStatus = useConnectionStatus();
	const serverRef = useRef(server);
	serverRef.current = server;
	useEffect(() => {
		if (!enabled) return;
		return retainPendingFileOperationOwner();
	}, [enabled]);
	useControlSurfaceTarget(
		enabled
			? {
					id: `file-manager:${state.instanceId}`,
					priority: 500,
					accepts: (intent) =>
						intent.type === "file_operation_key" &&
						(intent.action === "escape" || intent.action === "enter") &&
						fileOperationOwnership.claimed === state.instanceId &&
						Boolean(state.operationRef.current),
					handle: (intent) => {
						if (intent.type !== "file_operation_key") return;
						const action = intent.action;
						if (
							fileOperationOwnership.claimed !== state.instanceId ||
							!state.operationRef.current
						)
							return;
						if (action === "escape") operations.cancelOperation();
						if (action === "enter") void operations.completeOperation();
					},
				}
			: null,
	);
	useEffect(() => {
		if (!enabled) return;
		return () => {
			if (fileOperationOwnership.claimed === state.instanceId) {
				fileOperationOwnership.claimed = null;
				void serverRef.current
					.releaseFileInput(state.instanceId)
					.catch(() => undefined);
			}
		};
	}, [enabled, state.instanceId]);

	useEffect(() => {
		if (!enabled) return;
		const routeFileInput = (event: Event) => {
			const detail = (
				event as CustomEvent<{ action?: string; instance_id?: string }>
			).detail;
			if (
				detail?.instance_id !== state.instanceId ||
				fileOperationOwnership.claimed !== state.instanceId ||
				!state.operationRef.current
			)
				return;
			if (detail.action === "escape") {
				event.preventDefault();
				operations.cancelOperation();
			} else if (detail.action === "enter") {
				event.preventDefault();
				void operations.completeOperation();
			}
		};
		const releaseUnclaimed = (event: PointerEvent) => {
			if (
				fileOperationOwnership.pending &&
				(!(event.target instanceof Element) ||
					!event.target.closest(".file-manager"))
			) {
				fileOperationOwnership.pending = null;
			}
		};
		window.addEventListener("light:file-manager-input", routeFileInput);
		document.addEventListener("pointerdown", releaseUnclaimed, true);
		return () => {
			window.removeEventListener("light:file-manager-input", routeFileInput);
			document.removeEventListener("pointerdown", releaseUnclaimed, true);
		};
	}, [enabled, operations, state.instanceId, state.operationRef]);

	useEffect(() => {
		if (!enabled) return;
		const operation = state.operation;
		if (!operation || fileOperationOwnership.claimed !== state.instanceId)
			return;
		const timer = window.setInterval(() => {
			void serverRef.current
				.claimFileInput(state.instanceId, operation.kind, "toolbar")
				.catch(() =>
					operations.cancelOperation(
						"The server released this File Manager input context.",
					),
				);
		}, 30_000);
		return () => window.clearInterval(timer);
	}, [enabled, operations, state.instanceId, state.operation?.kind]);

	useEffect(() => {
		if (!enabled) return;
		if (connectionStatus === "connected" || !state.operationRef.current) return;
		operations.cancelOperation(
			"The file operation was cancelled because the desk connection was lost.",
		);
	}, [enabled, operations, connectionStatus, state.operationRef]);
}

function useFileOperationKeys(
	state: FileManagerState,
	operations: FileOperationActions,
	picker: FileManagerPickerOptions | undefined,
	pickerValid: boolean,
	enabled: boolean,
) {
	useEffect(() => {
		if (!enabled) return;
		const interceptKeys = (event: KeyboardEvent) => {
			const target = event.target;
			const editingName =
				target instanceof Element &&
				Boolean(target.closest(".file-rename-editor"));
			if (
				event.key === "Escape" &&
				state.operationRef.current &&
				fileOperationOwnership.claimed === state.instanceId
			) {
				event.preventDefault();
				event.stopImmediatePropagation();
				operations.cancelOperation();
				return;
			}
			if (
				event.key === "Enter" &&
				state.operationRef.current &&
				fileOperationOwnership.claimed === state.instanceId &&
				!editingName
			) {
				event.preventDefault();
				event.stopImmediatePropagation();
				void operations.completeOperation();
				return;
			}
			if (!picker || editingName) return;
			if (event.key === "Escape") {
				event.preventDefault();
				event.stopImmediatePropagation();
				picker.onCancel();
			} else if (event.key === "Enter" && pickerValid) {
				event.preventDefault();
				event.stopImmediatePropagation();
				picker.onSelect(state.selected);
			}
		};
		const interceptTouchKey = (event: MouseEvent) => {
			if (
				fileOperationOwnership.claimed !== state.instanceId ||
				!state.operationRef.current
			)
				return;
			const key = (event.target as Element | null)?.closest<HTMLElement>(
				"[data-keypad-key]",
			)?.dataset.keypadKey;
			if (key !== "ENT" && key !== "ESC") return;
			event.preventDefault();
			event.stopImmediatePropagation();
			if (key === "ENT") void operations.completeOperation();
			else operations.cancelOperation();
		};
		window.addEventListener("keydown", interceptKeys, true);
		document.addEventListener("click", interceptTouchKey, true);
		return () => {
			window.removeEventListener("keydown", interceptKeys, true);
			document.removeEventListener("click", interceptTouchKey, true);
		};
	}, [enabled, operations, picker, pickerValid, state]);
}

export function useFileOperationInput(
	state: FileManagerState,
	operations: FileOperationActions,
	picker: FileManagerPickerOptions | undefined,
	pickerValid: boolean,
	enabled = true,
) {
	const server = useFiles();
	useFileOperationRouting(state, operations, server, enabled);
	useFileOperationKeys(state, operations, picker, pickerValid, enabled);
}
