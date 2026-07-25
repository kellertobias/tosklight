import { useEffect, useState } from "react";
import type { UpdateResult, UpdateSettings } from "../../api/types";
import { useProgrammingUpdate } from "../../features/programmingUpdate/ProgrammingUpdateProvider";
import { useApp } from "../../state/AppContext";
import { useCommandLineSurface } from "../control/commandLine/useCommandLineSurface";
import { defaultUpdateSettings } from "../control/updateWorkflow";
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

	useUpdateWorkflowEvents({
		commandLine,
		operation,
		busy,
		disarm,
		loadMenu: menu.load,
		setBusy,
		setLocalError,
		setMenuOpen: menu.setOpen,
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
		...actions,
	};
}
