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
import { useRecordGesture } from "./commandLine/useRecordGesture";
import "./CommandLineHistory.css";
import { programmerValueCount } from "./programmerActivity";
import { openUpdateTargetMenu } from "./updateWorkflow";

function useCommandErrors(setCompleted: Dispatch<SetStateAction<boolean>>) {
	const server = useServer();
	const [commandError, setCommandError] = useState<string | null>(null);
	const [persistentError, setPersistentError] = useState<string | null>(null);
	const [errorOpen, setErrorOpen] = useState(false);
	useEffect(() => {
		if (server.error) setPersistentError(server.error);
	}, [server.error]);
	useEffect(() => {
		if (commandError && server.error) setCommandError(server.error);
	}, [server.error, commandError]);
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
	const hardware = Boolean(
		server.bootstrap?.hardware_connected || state.midiProfile,
	);
	const [completed, setCompleted] = useState(false);
	const errors = useCommandErrors(setCompleted);
	const [historyOpen, setHistoryOpen] = useState(false);
	const historyPanel = useRef<HTMLElement | null>(null);
	useHistoryDismissal(historyOpen, historyPanel, setHistoryOpen);
	const ownProgrammer = server.bootstrap?.active_programmers.find(
		(programmer) => programmer.session_id === server.session?.session_id,
	);
	const hasRecordableContent =
		server.selectedFixtures.length > 0 ||
		programmerValueCount(ownProgrammer) > 0 ||
		state.preload !== "idle" ||
		state.preloadActive;
	const pendingCount =
		(ownProgrammer?.preload_pending?.length ?? 0) +
		Object.values(ownProgrammer?.preload_group_pending ?? {}).reduce(
			(count, attributes) => count + Object.keys(attributes).length,
			0,
		);
	const pendingLabels = (ownProgrammer?.preload_playback_pending ?? []).map(
		(pending) =>
			`${pending.action.replaceAll("-", " ").toUpperCase()} ${pending.playback_number}`,
	);
	const pendingSummary = [
		pendingCount ? `PROG ${pendingCount}` : "",
		...pendingLabels,
	]
		.filter(Boolean)
		.join(" · ");
	const replaceCommand = (value: string, pristine = false) => {
		setCompleted(false);
		errors.setCommandError(null);
		server.setCommandLine(value, pristine);
	};
	const execute = async () => {
		const ok = await server.executeCommandLine();
		setCompleted(ok);
		if (ok && state.storeArmed)
			dispatch({ type: "SET_STORE_ARMED", value: false });
		if (ok && state.updateArmed)
			dispatch({ type: "SET_UPDATE_ARMED", value: false });
		if (!ok)
			errors.setCommandError(
				server.error ?? "The command could not be executed.",
			);
	};
	const toggleRecord = () => {
		const armed = !state.storeArmed;
		if (armed && state.cueListSetArmed)
			dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
		dispatch({ type: "SET_STORE_ARMED", value: armed });
		if (armed) replaceCommand("RECORD ");
		else if (/^RECORD\b/i.test(server.commandLine))
			replaceCommand(server.commandLine.replace(/^RECORD\s*/i, ""));
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
		await server.preloadAction(state.preload === "blind" ? "go" : "enter");
		dispatch({ type: "ADVANCE_PRELOAD" });
	};
	const releasePreload = async () => {
		await server.preloadAction("release");
		dispatch({ type: "RELEASE_PRELOAD" });
	};
	useCommandLineShortcuts(hardware, {
		completed,
		persistentError: errors.persistentError,
		replaceCommand,
		execute,
		armUpdateOrMenu,
		dismissPersistentError: errors.acknowledgePersistent,
	});
	return (
		<header
			className={`command-line-bar command-line-left ${state.controlMode === "playbacks" ? "playback-mode" : ""} ${errors.commandError ? "has-command-error" : ""}`}
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
				preloadLabel={state.preload === "blind" ? "PRELOAD GO" : "PRELOAD"}
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
