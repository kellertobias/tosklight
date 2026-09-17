import { useEffect, useState } from "react";
import type { UpdateResult, UpdateSettings } from "../../api/types";
import { useProgrammingUpdate } from "../../features/programmingUpdate/ProgrammingUpdateProvider";
import { useApp } from "../../state/AppContext";
import { useCommandLineSurface } from "../control/commandLine/useCommandLineSurface";
import { defaultUpdateSettings } from "../control/updateWorkflow";
import { useCueRecording } from "../../features/cueRecording/CueRecordingProvider";
import { loadRecordSettings } from "../setup/ProgrammerDefaults";
import { useUpdateChoice } from "./useUpdateChoice";
import { useUpdateWorkflowActions } from "./useUpdateWorkflowActions";
import {
	type UpdateOperation,
	useUpdateWorkflowEvents,
} from "./useUpdateWorkflowEvents";
import { useUpdateWorkflowMenu } from "./useUpdateWorkflowMenu";

export function useUpdateWorkflowController() {
	const update = useProgrammingUpdate();
	const commandLine = useCommandLineSurface({ observeCommand: false });
	const { state, dispatch } = useApp();
	const [settings, setSettings] = useState<UpdateSettings>(
		defaultUpdateSettings,
	);
	const [settingsOpen, setSettingsOpen] = useState(false);
	const [operation, setOperation] = useState<UpdateOperation | null>(null);
	const [result, setResult] = useState<UpdateResult | null>(null);
	const [busy, setBusy] = useState(false);
	const [localError, setLocalError] = useState<string | null>(null);
	const scopeKey = update?.scopeKey ?? "unavailable";
	const menu = useUpdateWorkflowMenu({
		update,
		scopeKey,
		setBusy,
		setLocalError,
	});

	useEffect(() => {
		setBusy(false);
		setLocalError(null);
		setOperation(null);
		setResult(null);
		setSettingsOpen(false);
	}, [scopeKey]);

	const disarm = () => {
		dispatch({ type: "SET_UPDATE_ARMED", value: false });
		dispatch({ type: "SET_SHIFT_ARMED", value: false });
		if (/^UPDATE\b/i.test(commandLine.read().text.trim()))
			void commandLine.reset();
	};

	const cueRecording = useCueRecording();
	const choice = useUpdateChoice({
		update,
		commandLine,
		openTargets: () => {
			disarm();
			menu.setOpen(true);
			void menu.load("eligible_for_update_existing");
		},
	});
	const recordNewCue = async (cueListId: string) =>
		(await cueRecording?.record({
			target: { kind: "cue_list", cueListId },
			operation: "add_cue",
			timing: {},
			cueOnly: loadRecordSettings().cueOnly,
			capturePolicy: "current_capture",
			activationPolicy: "hold",
		})) ?? null;

	useUpdateWorkflowEvents({
		commandLine,
		openChoice: () => void choice.show(),
		recordNewCue,
		operation,
		busy,
		disarm,
		setBusy,
		setLocalError,
		setOperation,
		setResult,
		setSettings,
		setSettingsOpen,
	});
	const actions = useUpdateWorkflowActions({
		operation,
		settings,
		disarm,
		setBusy,
		setBusyKey: menu.setBusyKey,
		setLocalError,
		setMenuOpen: menu.setOpen,
		setOperation,
		setResult,
		setSettingsOpen,
	});

	const cancelOperation = () => {
		setOperation(null);
		setLocalError(null);
		disarm();
	};
	const cancelSettings = () => {
		setSettingsOpen(false);
		setLocalError(null);
	};

	return {
		armed: state.updateArmed,
		busy,
		localError,
		operation,
		settings,
		settingsOpen,
		setSettings,
		result,
		closeResult: () => setResult(null),
		cancelOperation,
		cancelSettings,
		menu,
		choice,
		...actions,
	};
}
