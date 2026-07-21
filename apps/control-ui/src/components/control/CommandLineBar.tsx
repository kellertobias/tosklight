import { useServerError } from "../../features/shellStatus/ShellStatusState";
import {
	type Dispatch,
	type RefObject,
	type SetStateAction,
	useEffect,
	useRef,
	useState,
} from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { CommandInput } from "./commandLine/CommandInput";
import { CommandLineHistoryPanel } from "./commandLine/CommandLineHistoryPanel";
import {
	CommandErrorBanner,
	PersistentErrorPopover,
} from "./commandLine/CommandLineStatus";
import { CommandRecordPreload } from "./commandLine/CommandRecordPreload";
import { useCommandLineShortcuts } from "./commandLine/useCommandLineShortcuts";
import { useCommandLineSurface } from "./commandLine/useCommandLineSurface";
import { useRecordGesture } from "./commandLine/useRecordGesture";
import "./CommandLineHistory.css";
import { useProgrammerPreloadPlaybackQueueView } from "../../features/programmerPreloadPlaybackQueue/ProgrammerPreloadPlaybackQueueView";
import { useProgrammerPreloadLifecycleView } from "../../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView";
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
	const server = useServer();
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
		server.dismissError();
	};
	const acknowledgePersistent = () => {
		setPersistentError(null);
		setErrorOpen(false);
		server.dismissError();
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

function useHistoryDismissal(
	open: boolean,
	panel: RefObject<HTMLElement | null>,
	setOpen: Dispatch<SetStateAction<boolean>>,
) {
	useEffect(() => {
		if (!open) return;
		const closeOnEscape = (event: KeyboardEvent) => {
			if (event.key !== "Escape") return;
			event.preventDefault();
			setOpen(false);
		};
		const closeOutside = (event: PointerEvent) => {
			if (panel.current?.contains(event.target as Node)) return;
			if ((event.target as Element | null)?.closest(".command-input")) return;
			setOpen(false);
		};
		window.addEventListener("keydown", closeOnEscape, true);
		window.addEventListener("pointerdown", closeOutside, true);
		return () => {
			window.removeEventListener("keydown", closeOnEscape, true);
			window.removeEventListener("pointerdown", closeOutside, true);
		};
	}, [open, panel, setOpen]);
}

export function CommandLineBar() {
	const { state, dispatch } = useApp();
	const server = useServer();
	const serverError = useServerError();
	const command = useCommandLineSurface({ selection: true });
	const programmerActivity = useProgrammerValuesActivity();
	const preloadPlaybackQueue = useProgrammerPreloadPlaybackQueueView();
	const preload = useProgrammerPreloadLifecycleView();
	const hardware =
		Boolean(server.bootstrap?.hardware_connected) || Boolean(state.midiProfile);
	const [completed, setCompleted] = useState(false);
	const editGeneration = useRef(0);
	const errors = useCommandErrors(setCompleted);
	const [historyOpen, setHistoryOpen] = useState(false);
	const historyPanel = useRef<HTMLElement | null>(null);
	useHistoryDismissal(historyOpen, historyPanel, setHistoryOpen);
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
	});
	return (
		<header
			className={`command-line-bar command-line-left ${state.controlMode === "playbacks" ? "playback-mode" : ""} ${errors.commandError ? "has-command-error" : ""}`}
			aria-busy={!command.ready}
			data-command-authority={command.ready ? "ready" : "loading"}
		>
			<CommandErrorBanner
				message={errors.commandError}
				onAcknowledge={errors.acknowledgeCommand}
			/>
			<CommandInput
				playback={state.controlMode === "playbacks"}
				hardware={hardware}
				completed={completed}
				commandError={errors.commandError}
				commandLine={command.text}
				commandTarget={command.target}
				preloadArmed={preload.armed}
				onReplace={replaceCommand}
				onExecute={execute}
				onOpenHistory={() => setHistoryOpen(true)}
			/>
			<CommandLineHistoryPanel
				open={historyOpen}
				panel={historyPanel}
				onClose={() => setHistoryOpen(false)}
				onReuse={(command) => {
					replaceCommand(command);
					setHistoryOpen(false);
				}}
			/>
			<PersistentErrorPopover
				message={errors.persistentError}
				open={errors.errorOpen}
				onClose={() => errors.setErrorOpen(false)}
				onAcknowledge={errors.acknowledgePersistent}
			/>
			<CommandRecordPreload
				hasRecordableContent={hasRecordableContent}
				pendingSummary={pendingSummary}
				preloadLabel={preload.armed ? "PRELOAD GO" : "PRELOAD"}
				preloadArmed={preload.armed}
				preloadActive={preload.active}
				preloadReady={preload.ready}
				onRecordStart={record.begin}
				onRecordEnd={record.end}
				onRecordCancel={record.cancel}
				onRecordComplete={record.complete}
				onAdvancePreload={advancePreload}
				onReleasePreload={releasePreload}
			/>
		</header>
	);
}
