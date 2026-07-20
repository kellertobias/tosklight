import { useServer } from "../../../api/ServerContext";
import { useProgrammingSelectionActions } from "../../../features/programmingInteraction/ProgrammingInteractionView";
import { useProgrammerValuesActions } from "../../../features/programmerValues/ProgrammerValuesView";
import { useProgrammerValuesActivity } from "../../../features/programmerValues/useProgrammerValuesActivity";
import { useApp } from "../../../state/AppContext";
import type { BuiltInWindow } from "../../../types";
import { useCommandLineSurface } from "../commandLine/useCommandLineSurface";
import {
	editTargetedCommandWithSoftwareKey,
	type SoftwareKey,
} from "../softwareKeypad";

const shiftedWindows: Partial<Record<SoftwareKey, BuiltInWindow>> = {
	".": "help",
	"0": "fixtures",
	"1": "groups",
	"3": "cuelists",
	"5": "dynamics",
	"6": "channels",
};

export function useNumericPadController() {
	const server = useServer();
	const command = useCommandLineSurface({
		selection: true,
		observeCommand: false,
	});
	const { state, dispatch } = useApp();
	const values = useProgrammerValuesActivity();
	const valuesActions = useProgrammerValuesActions();
	const selectionActions = useProgrammingSelectionActions(true);
	const hasSelection = command.selected.length > 0;
	const hasProgrammerValues = values.ready && values.valueCount > 0;
	const context = {
		server,
		command,
		state,
		dispatch,
		values,
		valuesActions,
		selectionActions,
	};
	return {
		state,
		clearClass: hasSelection
			? "clear-active"
			: hasProgrammerValues
				? "clear-warning"
				: "clear-idle",
		toggleRecord: () => toggleRecord(context),
		advancePreload: () => advancePreload(context),
		escape: () => void command.reset(),
		press: (key: SoftwareKey) => pressKey(context, key),
	};
}

interface NumericPadContext {
	server: ReturnType<typeof useServer>;
	command: ReturnType<typeof useCommandLineSurface>;
	state: ReturnType<typeof useApp>["state"];
	dispatch: ReturnType<typeof useApp>["dispatch"];
	values: ReturnType<typeof useProgrammerValuesActivity>;
	valuesActions: ReturnType<typeof useProgrammerValuesActions>;
	selectionActions: ReturnType<typeof useProgrammingSelectionActions>;
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

async function advancePreload({ state, dispatch, server }: NumericPadContext) {
	await server.preloadAction(state.preload === "blind" ? "go" : "enter");
	dispatch({ type: "ADVANCE_PRELOAD" });
}

function pressKey(context: NumericPadContext, key: SoftwareKey) {
	const { state, dispatch, command, server } = context;
	const currentCommand = command.read();
	if (key === "SHIFT") {
		dispatch({ type: "SET_SHIFT_ARMED", value: !state.shiftArmed });
		return;
	}
	if (state.shiftArmed && handleShiftedKey(context, key, currentCommand.text))
		return;
	if (key === "CLR") return clearStep(context);
	if (key === "SET" && currentCommand.pristine && handleSet(context)) return;
	if (key === "UND") return void server.undoProgrammer();
	if (key === "ENT") return executeCommand(context);
	const edited = editTargetedCommandWithSoftwareKey(
		currentCommand.text,
		key,
		currentCommand.target,
		currentCommand.pristine,
	);
	void command.replace(edited.command, edited.pristine);
	if (edited.execute) void command.execute(edited.command);
}

function handleShiftedKey(
	{ state, dispatch, command, server }: NumericPadContext,
	key: SoftwareKey,
	text: string,
) {
	dispatch({ type: "SET_SHIFT_ARMED", value: false });
	if (key === "TIME") {
		const current = text.trim();
		const next =
			command.read().pristine || current === "FIXTURE" || current === "GROUP"
				? "SPD GRP"
				: `${current} SPD GRP`;
		void command.replace(next, false);
		return true;
	}
	if (key === "CLR" || key === "DEL") {
		dispatch({ type: "SET_MODAL", modal: "systemControlsOpen", value: true });
		return true;
	}
	if (key === "2") {
		dispatch({ type: "SET_PRESET_FAMILY", family: "Mixed" });
		dispatch({ type: "OPEN_BUILTIN", kind: "presets" });
		return true;
	}
	if (key === "4") {
		const activePlayback = server.playbacks?.selected_playback;
		dispatch({ type: "OPEN_BUILTIN", kind: "cuelists" });
		if (activePlayback != null)
			dispatch({ type: "OPEN_BUILTIN_CUELIST", number: activePlayback });
		return true;
	}
	if (key === "7" || key === "8" || key === "9") {
		const desk = state.desks[Number(key) - 7];
		if (desk) dispatch({ type: "OPEN_DESK", id: desk.id });
		return Boolean(desk);
	}
	const kind = shiftedWindows[key];
	if (!kind) return false;
	dispatch({ type: "OPEN_BUILTIN", kind });
	return true;
}

function clearStep(context: NumericPadContext) {
	const {
		state,
		dispatch,
		command,
		server,
		values,
		valuesActions,
		selectionActions,
	} = context;
	for (const [type, armed] of clearableArmedStates(state))
		if (armed) dispatch({ type, value: false });
	void command.reset();
	if (state.preload !== "idle" || values.authority === "preload") {
		void server.preloadAction("clear");
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

function handleSet({ state, dispatch }: NumericPadContext) {
	if (state.builtIn === "patch") {
		dispatch({ type: "SET_PATCH_ARMED", value: !state.patchSetArmed });
		return true;
	}
	if (document.querySelector(".cue-settings-compact-fallback")) {
		window.dispatchEvent(
			new CustomEvent("light:desk-action", { detail: "set" }),
		);
		return true;
	}
	if (document.querySelector(".cuelist-window.pool-window")) {
		if (state.storeArmed) dispatch({ type: "SET_STORE_ARMED", value: false });
		dispatch({ type: "SET_CUELIST_SET_ARMED", value: !state.cueListSetArmed });
		return true;
	}
	if (document.querySelector(".playback-fader-bank,.virtual-playback-grid")) {
		dispatch({
			type: "SET_PLAYBACK_SET_ARMED",
			value: !state.playbackSetArmed,
		});
		return true;
	}
	if (!presetSurfaceOpen(state)) return false;
	dispatch({ type: "SET_PRESET_SET_ARMED", value: !state.presetSetArmed });
	return true;
}

function presetSurfaceOpen(state: NumericPadContext["state"]) {
	if (state.builtIn === "presets") return true;
	const activeDesk = state.desks.find((desk) => desk.id === state.activeDeskId);
	return (
		state.builtIn == null &&
		Boolean(activeDesk?.panes.some((pane) => pane.kind === "presets"))
	);
}

function executeCommand({ state, dispatch, command }: NumericPadContext) {
	return void command.execute().then((ok) => {
		if (ok && state.storeArmed)
			dispatch({ type: "SET_STORE_ARMED", value: false });
		if (ok && state.updateArmed)
			dispatch({ type: "SET_UPDATE_ARMED", value: false });
	});
}
