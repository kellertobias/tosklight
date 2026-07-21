import { type Dispatch, useEffect, useLayoutEffect, useRef } from "react";
import { usePlaybackRuntimeActions } from "../../../features/playbackRuntime/PlaybackRuntimeView";
import { usePlaybackTopologyActions } from "../../../features/playbackTopology/PlaybackTopologyProvider";
import { useApp } from "../../../state/AppContext";
import type { Action } from "../../../state/appReducer";
import type { AppState } from "../../../types";
import type { CommandTargetMode } from "../../../controlSurface/commandTarget";
import {
	editTargetedCommandWithSoftwareKey,
	softwareKeyFromKeyboard,
} from "../softwareKeypad";
import { openUpdateSettings } from "../updateWorkflow";
import { KeyboardHeldActions } from "./keyboardFlashActions";
import { usePlaybackShortcutAuthority } from "./playbackShortcutAuthority";
import {
	type PlaybackShortcutContext,
	KeyboardPageActions,
	pressPlaybackSlot,
	releasePlaybackSlot,
	stepPlaybackPage,
} from "./playbackShortcutKeys";

interface ShortcutCallbacks {
	completed: boolean;
	commandLine: string;
	commandTargetMode: CommandTargetMode;
	commandLinePristine: boolean;
	persistentError: string | null;
	replaceCommand: (value: string, pristine?: boolean) => void;
	execute: (command?: string) => Promise<void>;
	armUpdateOrMenu: () => void;
	dismissPersistentError: () => void;
}

interface UpdateGesture {
	hold: { current: number | null };
	active: { current: boolean };
	held: { current: boolean };
}

interface ShortcutContext extends ShortcutCallbacks, PlaybackShortcutContext {
	state: AppState;
	dispatch: Dispatch<Action>;
	update: UpdateGesture;
}

function isExternalEditor(target: EventTarget | null) {
	if (!(target instanceof HTMLElement)) return false;
	return Boolean(
		target.closest("input,textarea,select,[contenteditable=true]") &&
			!target.closest(".command-input"),
	);
}

function handleFunctionKey(context: ShortcutContext, event: KeyboardEvent) {
	if (!/^F(?:[1-9]|1[0-3])$/.test(event.key)) return false;
	event.preventDefault();
	const number = Number(event.key.slice(1));
	if (number <= 8) {
		// A loading Page/desk/topology consumes the key but sends nothing.
		if (context.authority.ready) pressPlaybackSlot(context, event, number);
		return true;
	}
	const group = String.fromCharCode(65 + number - 9) as
		| "A"
		| "B"
		| "C"
		| "D"
		| "E";
	context.dispatch({ type: "SET_SPEED_GROUP", value: group });
	window.dispatchEvent(
		new CustomEvent("light:speed-group-tap", { detail: group }),
	);
	return true;
}

function handlePageKey(context: ShortcutContext, event: KeyboardEvent) {
	if (event.code !== "PageUp" && event.code !== "PageDown") return false;
	event.preventDefault();
	if (event.repeat) return true;
	// A loading Page/desk/topology consumes the key but creates nothing.
	if (context.authority.ready)
		stepPlaybackPage(context, event.code === "PageUp" ? 1 : -1);
	return true;
}

function handleEscape(context: ShortcutContext, event: KeyboardEvent) {
	event.preventDefault();
	if (document.querySelector("[role=dialog],.stacked-modal-layer")) return;
	const { state, dispatch } = context;
	if (state.updateArmed) {
		dispatch({ type: "SET_UPDATE_ARMED", value: false });
		dispatch({ type: "SET_SHIFT_ARMED", value: false });
		context.replaceCommand("", true);
	} else if (state.storeArmed) {
		dispatch({ type: "SET_STORE_ARMED", value: false });
	} else if (state.cueListSetArmed) {
		dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
	} else if (context.persistentError) {
		context.dismissPersistentError();
	} else {
		context.replaceCommand("", true);
	}
}

function beginUpdateGesture(context: ShortcutContext, event: KeyboardEvent) {
	if (event.repeat || context.update.active.current) return;
	context.update.active.current = true;
	context.update.held.current = false;
	context.update.hold.current = window.setTimeout(() => {
		context.update.held.current = true;
		openUpdateSettings();
	}, 650);
}

function applySoftwareEdit(
	context: ShortcutContext,
	key: Parameters<typeof editTargetedCommandWithSoftwareKey>[1],
) {
	const edited = editTargetedCommandWithSoftwareKey(
		context.completed ? context.commandTargetMode : context.commandLine,
		key,
		context.commandTargetMode,
		context.completed || context.commandLinePristine,
	);
	context.replaceCommand(edited.command, edited.pristine);
	if (edited.execute) void context.execute(edited.command);
}

function handleSoftwareKey(context: ShortcutContext, event: KeyboardEvent) {
	const key = softwareKeyFromKeyboard(event, true);
	if (!key) return;
	if (key === "ESC") {
		handleEscape(context, event);
		return;
	}
	event.preventDefault();
	if (
		key === "SET" &&
		(context.completed || context.commandLinePristine) &&
		context.state.builtIn === "patch"
	) {
		document
			.querySelector<HTMLButtonElement>('[data-keypad-key="SET"]')
			?.click();
	} else if (key === "REC" && event.shiftKey) {
		beginUpdateGesture(context, event);
	} else if (key === "REC") {
		document.querySelector<HTMLButtonElement>(".global-store-button")?.click();
	} else if (key === "PRE") {
		document.querySelector<HTMLButtonElement>(".preload-button")?.click();
	} else if (key === "CLR" || key === "UND") {
		document
			.querySelector<HTMLButtonElement>(`[data-keypad-key="${key}"]`)
			?.click();
	} else if (key === "ENT") {
		void context.execute();
	} else {
		applySoftwareEdit(context, key);
	}
}

function handleKeyDown(context: ShortcutContext, event: KeyboardEvent) {
	if (event.defaultPrevented || isExternalEditor(event.target)) return;
	if (event.metaKey || event.ctrlKey || event.altKey) return;
	if (handleFunctionKey(context, event)) return;
	if (handlePageKey(context, event)) return;
	handleSoftwareKey(context, event);
}

function finishUpdateGesture(context: ShortcutContext) {
	if (context.update.hold.current)
		window.clearTimeout(context.update.hold.current);
	context.update.hold.current = null;
	context.update.active.current = false;
	if (!context.update.held.current) context.armUpdateOrMenu();
	context.update.held.current = false;
}

function handleKeyUp(context: ShortcutContext, event: KeyboardEvent) {
	if (event.code === "End" && context.update.active.current) {
		finishUpdateGesture(context);
		return;
	}
	releasePlaybackSlot(context, event);
}

function releaseHeldControls(context: ShortcutContext) {
	if (context.update.hold.current)
		window.clearTimeout(context.update.hold.current);
	context.update.hold.current = null;
	context.update.active.current = false;
	context.heldActions.releaseAll();
	context.pageActions.invalidate();
}

function useRunningMenuShortcut(hardware: boolean) {
	const { state, dispatch } = useApp();
	useEffect(() => {
		if (hardware || !state.regularNumberShortcuts) return;
		const openRunningMenu = (event: KeyboardEvent) => {
			if (
				event.code !== "Delete" ||
				!event.shiftKey ||
				event.metaKey ||
				event.ctrlKey ||
				event.altKey ||
				isExternalEditor(event.target)
			)
				return;
			event.preventDefault();
			dispatch({ type: "SET_MODAL", modal: "systemControlsOpen", value: true });
		};
		window.addEventListener("keydown", openRunningMenu);
		return () => window.removeEventListener("keydown", openRunningMenu);
	}, [dispatch, hardware, state.regularNumberShortcuts]);
}

export function useCommandLineShortcuts(
	hardware: boolean,
	callbacks: ShortcutCallbacks,
) {
	const { state, dispatch } = useApp();
	const active = !hardware && state.regularNumberShortcuts;
	const authority = usePlaybackShortcutAuthority(active);
	const runtimeActions = usePlaybackRuntimeActions();
	const topologyActions = usePlaybackTopologyActions();
	const update: UpdateGesture = {
		hold: useRef<number | null>(null),
		active: useRef(false),
		held: useRef(false),
	};
	const heldActions = useRef(new KeyboardHeldActions()).current;
	const pageActions = useRef(new KeyboardPageActions()).current;
	const context = useRef<ShortcutContext | null>(null);
	context.current = {
		state,
		dispatch,
		authority,
		runtimeActions,
		update,
		heldActions,
		pageActions,
		...callbacks,
	};
	useRunningMenuShortcut(hardware);
	useLayoutEffect(() => {
		if (!active) return;
		heldActions.syncAuthority(runtimeActions);
		return () => heldActions.releaseAll();
	}, [active, heldActions, runtimeActions]);
	useLayoutEffect(() => {
		if (!active || !authority.ready) return pageActions.invalidate();
		pageActions.syncAuthority(
			topologyActions?.createPage ?? null,
			runtimeActions?.setActivePage ?? null,
		);
		return () => pageActions.invalidate();
	}, [
		active,
		authority.ready,
		pageActions,
		runtimeActions?.setActivePage,
		topologyActions?.createPage,
	]);
	useEffect(() => {
		if (!active) return;
		const current = () => context.current as ShortcutContext;
		const keydown = (event: KeyboardEvent) => handleKeyDown(current(), event);
		const keyup = (event: KeyboardEvent) => handleKeyUp(current(), event);
		const blur = () => heldActions.releaseAll();
		window.addEventListener("keydown", keydown);
		window.addEventListener("keyup", keyup);
		window.addEventListener("blur", blur);
		return () => {
			window.removeEventListener("keydown", keydown);
			window.removeEventListener("keyup", keyup);
			window.removeEventListener("blur", blur);
			releaseHeldControls(current());
		};
	}, [active, heldActions]);
}
