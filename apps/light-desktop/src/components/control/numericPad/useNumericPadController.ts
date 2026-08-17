import { useRef } from "react";
import type { ControlSurfaceSource } from "../../../features/controlSurfaceInteraction/registry";
import { routeControlSurfaceIntent } from "../../../features/controlSurfaceInteraction/registry";
import { useSetInteraction } from "../../../features/controlSurfaceInteraction/SetInteractionProvider";
import { useDeskLockActions } from "../../../features/deskLock/DeskLockActionsProvider";
import { useDeskLock } from "../../../features/deskLock/DeskLockState";
import {
	usePlaybackDeskView,
	usePlaybackRuntimeStatus,
} from "../../../features/playbackRuntime/PlaybackRuntimeView";
import { useProgrammerActions } from "../../../features/programmerActions/ProgrammerActionsContext";
import { useProgrammerPreloadLifecycleView } from "../../../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView";
import { useProgrammerValuesActions } from "../../../features/programmerValues/ProgrammerValuesView";
import { useProgrammerValuesActivity } from "../../../features/programmerValues/useProgrammerValuesActivity";
import { useProgrammingSelectionActions } from "../../../features/programmingInteraction/ProgrammingInteractionView";
import { useApp } from "../../../state/AppContext";
import { useCommandLineSurface } from "../commandLine/useCommandLineSurface";
import {
	editTargetedCommandWithSoftwareKey,
	type SoftwareKey,
} from "../softwareKeypad";
import { openUpdateSettings } from "../updateWorkflow";
import {
	applyGestureCommand,
	type CommandKeyGestureIntent,
	resolveCommandKeyGesture,
} from "./commandKeyGesture";

export function useNumericPadController() {
	const programmerActions = useProgrammerActions();
	const command = useCommandLineSurface({
		selection: true,
		observeCommand: false,
	});
	const { state, dispatch } = useApp();
	const values = useProgrammerValuesActivity();
	const valuesActions = useProgrammerValuesActions();
	const selectionActions = useProgrammingSelectionActions(true);
	const playbackDesk = usePlaybackDeskView();
	const playbackStatus = usePlaybackRuntimeStatus();
	const preload = useProgrammerPreloadLifecycleView();
	const setInteraction = useSetInteraction();
	const hasSelection = command.selected.length > 0;
	const deskLock = useDeskLock();
	const deskLockActions = useDeskLockActions();
	const gesture = useRef<{ key: SoftwareKey | null; at: number }>({
		key: null,
		at: 0,
	});
	const hasProgrammerValues = values.ready && values.valueCount > 0;
	const context = {
		programmerActions,
		command,
		state,
		deskLocked: deskLock?.locked === true,
		unfreezeNext: /^\s*FREEZE\b/i.test(command.text),
		dispatch,
		values,
		valuesActions,
		selectionActions,
		playbackDesk,
		playbackReady: playbackStatus.status === "ready" && playbackDesk !== null,
		preload,
		setInteraction,
		deskLock,
		deskLockActions,
		gesture,
	};
	return {
		state,
		deskLocked: deskLock?.locked === true,
		unfreezeNext: /^\s*FREEZE\b/i.test(command.text),
		preload,
		clearState: hasSelection
			? ("selection" as const)
			: hasProgrammerValues
				? ("active-values" as const)
				: ("idle" as const),
		toggleRecord: () => toggleRecord(context),
		advancePreload: () => advancePreload(context),
		toggleFixtureFreeze: () => advanceFixtureFreezeCommand(command),
		selectFixtureFreezeFamily: (key: "1" | "2" | "3" | "4") =>
			selectFixtureFreezeFamily(command, key),
		escape: () => {
			if (setInteraction)
				void setInteraction.cancel().then((consumed) => {
					if (!consumed) void command.reset();
				});
			else void command.reset();
		},
		press: (key: SoftwareKey, source: ControlSurfaceSource = "touch") =>
			pressKey(context, key, source),
		hold: (key: SoftwareKey) =>
			applyGestureIntent(
				context,
				resolveCommandKeyGesture(key, {
					kind: "hold",
					shifted: state.shiftArmed,
				}),
			),
		pressShifted: (key: SoftwareKey) =>
			handleShiftedKey(context, key, command.read().text, false),
	};
}

interface NumericPadContext {
	programmerActions: ReturnType<typeof useProgrammerActions>;
	command: ReturnType<typeof useCommandLineSurface>;
	state: ReturnType<typeof useApp>["state"];
	dispatch: ReturnType<typeof useApp>["dispatch"];
	values: ReturnType<typeof useProgrammerValuesActivity>;
	valuesActions: ReturnType<typeof useProgrammerValuesActions>;
	selectionActions: ReturnType<typeof useProgrammingSelectionActions>;
	playbackDesk: ReturnType<typeof usePlaybackDeskView>;
	playbackReady: boolean;
	preload: ReturnType<typeof useProgrammerPreloadLifecycleView>;
	setInteraction: ReturnType<typeof useSetInteraction>;
	deskLock: ReturnType<typeof useDeskLock>;
	deskLockActions: ReturnType<typeof useDeskLockActions>;
	gesture: { current: { key: SoftwareKey | null; at: number } };
}

function toggleRecord({ state, dispatch, command }: NumericPadContext) {
	const currentCommand = command.read();
	const armed = !state.storeArmed;
	if (armed && state.cueListSetArmed)
		dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
	dispatch({ type: "SET_STORE_ARMED", value: armed });
	if (armed) void command.replace("RECORD ", false);
	else if (/^RECORD\b/i.test(currentCommand.text))
		void command.replace(currentCommand.text.replace(/^RECORD\s*/i, ""), false);
}

async function advancePreload({ preload }: NumericPadContext) {
	if (!preload.ready || !preload.actions) return;
	if (preload.armed) await preload.actions.go();
	else await preload.actions.enter();
}

function pressKey(
	context: NumericPadContext,
	key: SoftwareKey,
	source: ControlSurfaceSource,
) {
	const { state, dispatch, command, programmerActions } = context;
	const currentCommand = command.read();
	const now = performance.now();
	const repeated =
		context.gesture.current.key === key &&
		now - context.gesture.current.at <= 450;
	context.gesture.current = { key, at: now };
	if (key === "SHIFT") {
		dispatch({ type: "SET_SHIFT_ARMED", value: !state.shiftArmed });
		return;
	}
	if (
		state.shiftArmed &&
		handleShiftedKey(context, key, currentCommand.text, repeated)
	)
		return;
	if (repeated && handleRepeatedKey(context, key)) return;
	if (key === "CLR") return clearStep(context);
	if (key === "SET" && currentCommand.pristine && handleSet(source)) return;
	if (key === "UND") return void programmerActions?.undoProgrammer();
	if (key === "ENT") return executeCommand(context);
	const edited = editTargetedCommandWithSoftwareKey(
		currentCommand.text,
		key,
		currentCommand.target,
		currentCommand.pristine,
		key === "." ? repeated && command.selected.length > 0 : repeated,
	);
	void command.replace(edited.command, edited.pristine);
	if (edited.execute) void command.execute(edited.command);
}

function handleShiftedKey(
	context: NumericPadContext,
	key: SoftwareKey,
	text: string,
	repeated: boolean,
) {
	const { command } = context;
	if (key === "SET" && context.setInteraction) {
		void context.setInteraction.arm("touch");
		return true;
	}
	if (/^\s*(?:FREEZE|UNFREEZE)\b/i.test(text) && /^[1-4]$/.test(key)) {
		selectFixtureFreezeFamily(command, key as "1" | "2" | "3" | "4");
		return true;
	}
	const intent = resolveCommandKeyGesture(key, {
		kind: repeated ? "double" : "regular",
		shifted: true,
	});
	return applyGestureIntent(context, intent);
}

function handleRepeatedKey(context: NumericPadContext, key: SoftwareKey) {
	return applyGestureIntent(
		context,
		resolveCommandKeyGesture(key, { kind: "double", shifted: false }),
	);
}

function applyGestureIntent(
	context: NumericPadContext,
	intent: CommandKeyGestureIntent,
) {
	if (!intent) return false;
	if (intent.type === "command") {
		const current = context.command.read();
		void context.command.replace(
			applyGestureCommand(current.text, current.pristine, intent),
			false,
		);
		return true;
	}
	switch (intent.action) {
		case "undo":
			void context.programmerActions?.undoProgrammer();
			break;
		case "lock":
			if (context.deskLockActions) {
				if (context.deskLock?.locked) void context.deskLockActions.unlockDesk();
				else void context.deskLockActions.lockDesk();
			}
			break;
		case "clear-preload":
			void context.preload.actions?.clearPending();
			break;
		case "running-output":
			context.dispatch({
				type: "SET_MODAL",
				modal: "systemControlsOpen",
				value: true,
			});
			break;
		case "inspect-groups":
			context.dispatch({ type: "OPEN_BUILTIN", kind: "groups" });
			break;
		case "inspect-fixtures":
			context.dispatch({ type: "OPEN_BUILTIN", kind: "fixtures" });
			break;
		case "inspect-preload":
			context.dispatch({
				type: "SET_MODAL",
				modal: "preloadStoreOpen",
				value: true,
			});
			break;
		case "record-options":
			context.dispatch({
				type: "SET_MODAL",
				modal: "storeSettingsOpen",
				value: true,
			});
			break;
		case "update-options":
			openUpdateSettings();
			break;
		case "align-off":
			window.dispatchEvent(new Event("light:align-off"));
			break;
	}
	return true;
}

function advanceFixtureFreezeCommand(command: NumericPadContext["command"]) {
	const current = command.read().text.trim();
	const next = /^FREEZE\b/i.test(current)
		? current.replace(/^FREEZE\b/i, "UNFREEZE")
		: "FREEZE";
	void command.replace(next, false);
}

function selectFixtureFreezeFamily(
	command: NumericPadContext["command"],
	key: "1" | "2" | "3" | "4",
) {
	const family = {
		"1": "INTENSITY",
		"2": "COLOR",
		"3": "POSITION",
		"4": "BEAM",
	}[key];
	const current = command.read().text.trim();
	if (!/^\s*(?:FREEZE|UNFREEZE)\b/i.test(current)) return;
	if (new RegExp(`(?:^|\\s)${family}(?:\\s|$)`, "i").test(current)) return;
	void command.replace(`${current} ${family}`, false);
}

function clearStep(context: NumericPadContext) {
	if (context.setInteraction) {
		void context.setInteraction.clear().then((consumed) => {
			if (!consumed) clearProgrammerStep(context);
		});
		return;
	}
	clearProgrammerStep(context);
}

function clearProgrammerStep(context: NumericPadContext) {
	const {
		state,
		dispatch,
		command,
		values,
		valuesActions,
		selectionActions,
		preload,
	} = context;
	for (const [type, armed] of clearableArmedStates(state))
		if (armed) dispatch({ type, value: false });
	void command.reset();
	if (preload.armed || preload.active || values.authority === "preload") {
		if (preload.ready) void preload.actions?.clearPending();
		return;
	}
	if (command.selected.length > 0) {
		void selectionActions?.replace({ resolvedFixtures: [] });
		return;
	}
	if (values.ready && values.valueCount > 0 && valuesActions)
		void valuesActions.clear(crypto.randomUUID());
}

function clearableArmedStates(state: NumericPadContext["state"]) {
	return [
		["SET_UPDATE_ARMED", state.updateArmed],
		["SET_STORE_ARMED", state.storeArmed],
		["SET_CUELIST_SET_ARMED", state.cueListSetArmed],
		["SET_PLAYBACK_SET_ARMED", state.playbackSetArmed],
	] as const;
}

function handleSet(source: ControlSurfaceSource) {
	return (
		routeControlSurfaceIntent({ type: "set", source }).status === "handled"
	);
}

function executeCommand(context: NumericPadContext) {
	const { state, dispatch, command, setInteraction } = context;
	return void (async () => {
		if (setInteraction && (await setInteraction.enter("touch"))) return;
		const ok = await command.execute();
		if (ok && state.storeArmed)
			dispatch({ type: "SET_STORE_ARMED", value: false });
		if (ok && state.updateArmed)
			dispatch({ type: "SET_UPDATE_ARMED", value: false });
	})();
}
