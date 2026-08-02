import { CommandLine, type CommandStatus } from "@tosklight/ui";
import {
	type Dispatch,
	type SetStateAction,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { useCommandHistory } from "../../features/commandHistory/CommandHistoryState";
import {
	mergeCommandHistory,
	useContentErrorHistory,
} from "../../features/commandHistory/useContentErrorHistory";
import { useSetInteraction } from "../../features/controlSurfaceInteraction/SetInteractionProvider";
import {
	useActiveTimecode,
	useFrameRateHz,
	useHardwareConnected,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { useHighlightSnapshot } from "../../features/highlight/HighlightState";
import { useOutputRuntimeBlackout } from "../../features/outputRuntime/OutputRuntimeView";
import { useShellStatusActions } from "../../features/shellStatus/ShellStatusActionsProvider";
import {
	useConnectionStatus,
	useServerError,
} from "../../features/shellStatus/ShellStatusState";
import { useApp } from "../../state/AppContext";
import { useCommandLineShortcuts } from "./commandLine/useCommandLineShortcuts";
import { useCommandLineSurface } from "./commandLine/useCommandLineSurface";
import { useRecordGesture } from "./commandLine/useRecordGesture";
import { useNumericPadController } from "./numericPad/useNumericPadController";
import "./CommandLineHistory.css";
import { useProgrammerPreloadLifecycleView } from "../../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView";
import { useProgrammerPreloadPlaybackQueueView } from "../../features/programmerPreloadPlaybackQueue/ProgrammerPreloadPlaybackQueueView";
import { useProgrammerValuesActivity } from "../../features/programmerValues/useProgrammerValuesActivity";
import { openUpdateTargetMenu } from "./updateWorkflow";

const queuedPlaybackLabels = {
	back: "GO MINUS",
	temporary_on: "TEMP ON",
	temporary_off: "TEMP OFF",
} as const;

function queuedPlaybackLabel(action: string, playbackNumber: number) {
	const label =
		queuedPlaybackLabels[action as keyof typeof queuedPlaybackLabels] ??
		action.replaceAll("_", " ").toUpperCase();
	return `${label} ${playbackNumber}`;
}

function useCommandErrors(setCompleted: Dispatch<SetStateAction<boolean>>) {
	const shellStatus = useShellStatusActions();
	const serverError = useServerError();
	const [commandError, setCommandError] = useState<string | null>(null);
	const [persistentError, setPersistentError] = useState<string | null>(null);
	const [errorOpen, setErrorOpen] = useState(false);
	useEffect(() => {
		if (serverError) setPersistentError(serverError);
	}, [serverError]);
	useEffect(() => {
		if (commandError && serverError) setCommandError(serverError);
	}, [serverError, commandError]);
	useEffect(() => {
		const showCommandError = (event: Event) => {
			setCompleted(false);
			setCommandError(
				(event as CustomEvent<string>).detail ||
					"The command could not be executed.",
			);
		};
		window.addEventListener("light:command-error", showCommandError);
		return () =>
			window.removeEventListener("light:command-error", showCommandError);
	}, [setCompleted]);
	const acknowledgeCommand = () => {
		setCommandError(null);
		shellStatus?.dismissError();
	};
	const acknowledgePersistent = () => {
		setPersistentError(null);
		setErrorOpen(false);
		shellStatus?.dismissError();
	};
	return {
		commandError,
		setCommandError,
		persistentError,
		errorOpen,
		setErrorOpen,
		acknowledgeCommand,
		acknowledgePersistent,
	};
}

function useCommandLineBarModel() {
	const { state, dispatch } = useApp();
	const hardwareAttached = useHardwareConnected();
	const authoritativeHistory = useCommandHistory();
	const contentErrorHistory = useContentErrorHistory();
	const history = useMemo(
		() => mergeCommandHistory(authoritativeHistory, contentErrorHistory),
		[authoritativeHistory, contentErrorHistory],
	);
	const connection = useConnectionStatus();
	const frequency = useFrameRateHz();
	const timecode = useActiveTimecode();
	const blackout = useOutputRuntimeBlackout() === true;
	const highlight = useHighlightSnapshot()?.active === true;
	const serverError = useServerError();
	const command = useCommandLineSurface({ selection: true });
	const programmerActivity = useProgrammerValuesActivity();
	const preloadPlaybackQueue = useProgrammerPreloadPlaybackQueueView();
	const preload = useProgrammerPreloadLifecycleView();
	const numericPad = useNumericPadController();
	const setInteraction = useSetInteraction();
	const hardware = hardwareAttached || Boolean(state.midiProfile);
	const [completed, setCompleted] = useState(false);
	const editGeneration = useRef(0);
	const errors = useCommandErrors(setCompleted);
	const [historyOpen, setHistoryOpen] = useState(false);
	useEffect(() => {
		if (errors.commandError) setHistoryOpen(true);
	}, [errors.commandError]);
	const hasRecordableContent =
		command.selected.length > 0 ||
		(programmerActivity.ready && programmerActivity.valueCount > 0) ||
		preload.active;
	const pendingLabels = (preloadPlaybackQueue?.actions ?? []).map((pending) =>
		queuedPlaybackLabel(pending.action, pending.playbackNumber),
	);
	const pendingSummary = [
		programmerActivity.pendingValueCount
			? `PROG ${programmerActivity.pendingValueCount}`
			: "",
		...pendingLabels,
	]
		.filter(Boolean)
		.join(" · ");
	const replaceCommand = (value: string, pristine = false) => {
		if (!command.ready) return;
		editGeneration.current++;
		setCompleted(false);
		errors.setCommandError(null);
		void command.replace(value, pristine);
	};
	const execute = async (value?: string) => {
		if (!command.ready) return;
		if (
			value == null &&
			setInteraction &&
			(await setInteraction.enter("touch"))
		)
			return;
		const generation = editGeneration.current;
		const ok = await command.execute(value);
		setCompleted(ok && generation === editGeneration.current);
		if (ok && state.storeArmed)
			dispatch({ type: "SET_STORE_ARMED", value: false });
		if (ok && state.updateArmed)
			dispatch({ type: "SET_UPDATE_ARMED", value: false });
		if (!ok)
			errors.setCommandError(
				serverError ?? "The command could not be executed.",
			);
	};
	const toggleRecord = () => {
		const armed = !state.storeArmed;
		if (armed && state.cueListSetArmed)
			dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
		dispatch({ type: "SET_STORE_ARMED", value: armed });
		if (armed) replaceCommand("RECORD ");
		else if (/^RECORD\b/i.test(command.text))
			replaceCommand(command.text.replace(/^RECORD\s*/i, ""));
	};
	const armUpdateOrMenu = () => {
		if (state.updateArmed) {
			openUpdateTargetMenu();
			return;
		}
		if (state.cueListSetArmed)
			dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
		if (state.playbackSetArmed)
			dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
		if (state.presetSetArmed)
			dispatch({ type: "SET_PRESET_SET_ARMED", value: false });
		dispatch({ type: "SET_UPDATE_ARMED", value: true });
		replaceCommand("UPDATE ");
	};
	const record = useRecordGesture({ armUpdateOrMenu, toggleRecord });
	const advancePreload = async () => {
		if (!preload.ready || !preload.actions) return;
		if (preload.armed) await preload.actions.go();
		else await preload.actions.enter();
	};
	const releasePreload = async () => {
		if (!preload.ready || !preload.actions) return;
		await preload.actions.release();
	};
	const openSystemControls = () =>
		dispatch({
			type: "SET_MODAL",
			modal: "systemControlsOpen",
			value: true,
		});
	const toggleControlMode = () => dispatch({ type: "TOGGLE_CONTROL_MODE" });
	useCommandLineShortcuts(hardware, {
		completed,
		commandLine: command.text,
		commandTargetMode: command.target,
		commandLinePristine: command.pristine,
		persistentError: errors.persistentError,
		replaceCommand,
		execute,
		armUpdateOrMenu,
		dismissPersistentError: errors.acknowledgePersistent,
		pressSet: () => numericPad.press("SET", "keyboard"),
		toggleRecord,
		advancePreload: () => void advancePreload(),
		clear: () => numericPad.press("CLR"),
		undo: () => numericPad.press("UND"),
	});
	const status: CommandStatus = {
		connection,
		frequency: typeof frequency === "number" ? frequency : "—",
		timecode,
		blackout,
		highlight,
	};
	return {
		state,
		hardware,
		command,
		preload,
		history,
		status,
		completed,
		errors,
		historyOpen,
		setHistoryOpen,
		hasRecordableContent,
		pendingSummary,
		replaceCommand,
		execute,
		record,
		advancePreload,
		releasePreload,
		openSystemControls,
		toggleControlMode,
	};
}

export function CommandLineBar() {
	const model = useCommandLineBarModel();
	return (
		<CommandLine
			mode={model.state.controlMode}
			hardware={model.hardware}
			ready={model.command.ready}
			completed={model.completed}
			commandError={model.errors.commandError}
			persistentError={model.errors.persistentError}
			persistentErrorOpen={model.errors.errorOpen}
			commandLine={model.command.text}
			commandTarget={model.command.target}
			preloadArmed={model.preload.armed}
			preloadActive={model.preload.active}
			preloadReady={model.preload.ready}
			preloadLabel={model.preload.armed ? "PRELOAD GO" : "PRELOAD"}
			pendingSummary={model.pendingSummary}
			recordState={
				model.state.updateArmed
					? "update-armed"
					: model.state.storeArmed
						? "record-armed"
						: model.hasRecordableContent
							? "ready"
							: "empty"
			}
			recordShiftArmed={model.state.shiftArmed}
			history={model.history}
			historyOpen={model.historyOpen}
			status={model.status}
			onReplace={model.replaceCommand}
			onExecute={model.execute}
			onToggleMode={model.toggleControlMode}
			onHistoryOpenChange={model.setHistoryOpen}
			onReuseHistory={(command) => {
				model.replaceCommand(command);
				model.setHistoryOpen(false);
			}}
			onOpenStatus={model.openSystemControls}
			onAcknowledgeCommandError={model.errors.acknowledgeCommand}
			onPersistentErrorOpenChange={model.errors.setErrorOpen}
			onAcknowledgePersistentError={model.errors.acknowledgePersistent}
			onRecordStart={model.record.begin}
			onRecordEnd={model.record.end}
			onRecordCancel={model.record.cancel}
			onRecordComplete={model.record.complete}
			onAdvancePreload={model.advancePreload}
			onReleasePreload={model.releasePreload}
		/>
	);
}
