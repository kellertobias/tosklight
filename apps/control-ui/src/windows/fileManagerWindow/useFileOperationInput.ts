import { useEffect, useRef } from "react";
import { useFiles } from "../../features/files/FilesContext";
import type { FilesContextValue } from "../../features/files/types";
import { fileOperationOwnership } from "./operationOwnership";
import type { FileManagerPickerOptions } from "./types";
import type { FileManagerState } from "./useFileManagerState";
import type { FileOperationActions } from "./useFileOperations";

function useFileOperationRouting(
	state: FileManagerState,
	operations: FileOperationActions,
	server: FilesContextValue,
) {
	const serverRef = useRef(server);
	serverRef.current = server;
	useEffect(
		() => () => {
			if (fileOperationOwnership.claimed === state.instanceId) {
				fileOperationOwnership.claimed = null;
				void serverRef.current
					.releaseFileInput(state.instanceId)
					.catch(() => undefined);
			}
		},
		[state.instanceId],
	);

	useEffect(() => {
		const routeDeskAction = (event: Event) => {
			const action = String(
				(event as CustomEvent<string>).detail ?? "",
			).toLowerCase();
			const next =
				action === "set"
					? "rename"
					: action === "copy" || action === "cpy"
						? "copy"
						: action === "move" || action === "mov"
							? "move"
							: action === "delete" || action === "del"
								? "delete"
								: null;
			if (next) fileOperationOwnership.pending = next;
			if (
				fileOperationOwnership.claimed !== state.instanceId ||
				!state.operationRef.current
			)
				return;
			if (action === "escape" || action === "esc") {
				event.preventDefault();
				operations.cancelOperation();
			}
			if (action === "enter" || action === "ent") {
				event.preventDefault();
				void operations.completeOperation();
			}
		};
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
		window.addEventListener("light:desk-action", routeDeskAction);
		window.addEventListener("light:file-manager-input", routeFileInput);
		document.addEventListener("pointerdown", releaseUnclaimed, true);
		return () => {
			window.removeEventListener("light:desk-action", routeDeskAction);
			window.removeEventListener("light:file-manager-input", routeFileInput);
			document.removeEventListener("pointerdown", releaseUnclaimed, true);
		};
	});

	useEffect(() => {
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
	}, [state.instanceId, state.operation?.kind]);

	useEffect(() => {
		if (server.status === "connected" || !state.operationRef.current) return;
		operations.cancelOperation(
			"The file operation was cancelled because the desk connection was lost.",
		);
	}, [server.status]);
}

function useFileOperationKeys(
	state: FileManagerState,
	operations: FileOperationActions,
	picker: FileManagerPickerOptions | undefined,
	pickerValid: boolean,
) {
	useEffect(() => {
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
	});
}

export function useFileOperationInput(
	state: FileManagerState,
	operations: FileOperationActions,
	picker: FileManagerPickerOptions | undefined,
	pickerValid: boolean,
) {
	const server = useFiles();
	useFileOperationRouting(state, operations, server);
	useFileOperationKeys(state, operations, picker, pickerValid);
}
