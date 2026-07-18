import { useServer } from "../../api/ServerContext";
import type {
	UpdateMenuEntry,
	UpdateMode,
	UpdateResult,
	UpdateSettings,
	UpdateTargetIdentity,
	UpdateTargetRequest,
} from "../../api/types";
import { updateTargetKey } from "../control/updateWorkflow";
import type { UpdateOperation } from "./useUpdateWorkflowEvents";

function requestFromIdentity(
	target: UpdateTargetIdentity,
): UpdateTargetRequest {
	return {
		family: target.family,
		object_id: target.object_id,
		...(target.playback_number == null
			? {}
			: { playback_number: target.playback_number }),
		...(target.cue
			? { cue_id: target.cue.id, cue_number: target.cue.number }
			: {}),
		...(target.playback_number == null
			? {}
			: { validate_active_context: true }),
	};
}

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
	const server = useServer();

	const changeOperationMode = async (mode: UpdateMode) => {
		if (!operation) return;
		setBusy(true);
		setLocalError(null);
		const preview = await server.previewUpdate(operation.request, mode);
		setBusy(false);
		if (preview) setOperation({ ...operation, preview });
		else setLocalError("This Update mode could not be previewed.");
	};
	const applyOperation = async () => {
		if (!operation) return;
		setBusy(true);
		setLocalError(null);
		const applied = await server.applyUpdate(
			operation.request,
			operation.preview.mode,
			operation.preview.revision,
			operation.preview.programmer_revision,
		);
		setBusy(false);
		if (!applied) {
			setLocalError("Update failed; no show data was changed.");
			return;
		}
		setOperation(null);
		disarm();
		setResult(applied);
	};
	const saveSettings = async () => {
		setBusy(true);
		setLocalError(null);
		const saved = await server.saveUpdateSettings(settings);
		setBusy(false);
		if (saved) setSettingsOpen(false);
		else setLocalError("Update Settings were not saved.");
	};
	const applyMenuTarget = async (entry: UpdateMenuEntry, mode: UpdateMode) => {
		const key = updateTargetKey(entry.target);
		setBusyKey(key);
		setLocalError(null);
		const selectedPreview =
			entry.add_new_preview?.mode.target_type === mode.target_type &&
			entry.add_new_preview.mode.mode === mode.mode
				? entry.add_new_preview
				: entry.existing_preview;
		const applied = await server.applyUpdate(
			requestFromIdentity(entry.target),
			mode,
			entry.revision,
			selectedPreview.programmer_revision,
		);
		setBusyKey(null);
		if (!applied) {
			setLocalError("Update failed; no show data was changed.");
			return;
		}
		setMenuOpen(false);
		setResult(applied);
	};
	return { changeOperationMode, applyOperation, saveSettings, applyMenuTarget };
}
