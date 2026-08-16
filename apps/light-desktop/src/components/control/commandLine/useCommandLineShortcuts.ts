import { type Dispatch, useEffect, useLayoutEffect, useRef } from "react";
import type { CommandTargetMode } from "../../../controlSurface/commandTarget";
import { useSetInteraction } from "../../../features/controlSurfaceInteraction/SetInteractionProvider";
import { usePlaybackRuntimeActions } from "../../../features/playbackRuntime/PlaybackRuntimeView";
import { usePlaybackTopologyActions } from "../../../features/playbackTopology/PlaybackTopologyProvider";
import { useApp } from "../../../state/AppContext";
import type { Action } from "../../../state/appReducer";
import type { AppState } from "../../../types";
import {
	editTargetedCommandWithSoftwareKey,
	softwareKeyFromKeyboard,
} from "../softwareKeypad";
import { openUpdateSettings } from "../updateWorkflow";
import { KeyboardHeldActions } from "./keyboardFlashActions";
import { usePlaybackShortcutAuthority } from "./playbackShortcutAuthority";
import {
	KeyboardPageActions,
	type PlaybackShortcutContext,
	pressPlaybackSlot,
	releasePlaybackSlot,
	stepPlaybackPage,
} from "./playbackShortcutKeys";
import { RECORD_HOLD_MS } from "./useRecordGesture";

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
	pressSet: () => void;
	toggleRecord: () => void;
	advancePreload: () => void;
	inspectPreload: () => void;
	clear: () => void;
	toggleFixtureFreeze: () => void;
	selectFixtureFreezeFamily: (key: "1" | "2" | "3" | "4") => void;
	undo: () => void;
	pressKey?: (
		key: Parameters<typeof editTargetedCommandWithSoftwareKey>[1],
	) => void;
}

interface UpdateGesture {
	hold: { current: number | null };
	active: { current: boolean };
	held: { current: boolean };
	mode: { current: "record" | "update" | null };
}

interface PreloadGesture {
	hold: { current: number | null };
	active: { current: boolean };
	held: { current: boolean };
}

interface ShortcutContext extends ShortcutCallbacks, PlaybackShortcutContext {
	state: AppState;
	dispatch: Dispatch<Action>;
	update: UpdateGesture;
	preload: PreloadGesture;
	setInteraction: ReturnType<typeof useSetInteraction>;
	pageChord: PageKeyChord;
	keyboardShiftDown: { current: boolean };
	regularNumberShortcuts: boolean;
}

const PRELOAD_HOLD_MS = 650;

export class PageKeyChord {
	private held = new Set<"PageUp" | "PageDown">();
	private timer: number | null = null;
	private chord = false;

	down(code: "PageUp" | "PageDown", single: () => void) {
		this.held.add(code);
		if (this.held.size === 2) {
			if (this.timer != null) window.clearTimeout(this.timer);
			this.timer = null;
			this.chord = true;
			window.dispatchEvent(new Event("light:playback-page-menu"));
			return;
		}
		this.timer = window.setTimeout(() => {
			this.timer = null;
			if (!this.chord) {
				this.held.delete(code);
				single();
			}
		}, 140);
	}

	up(code: "PageUp" | "PageDown") {
		this.held.delete(code);
		if (this.held.size === 0) this.chord = false;
	}

	reset() {
		if (this.timer != null) window.clearTimeout(this.timer);
		this.timer = null;
		this.held.clear();
		this.chord = false;
	}
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
		if (context.authority.ready) {
			const identity = context.authority.playbackIdentity(number);
			if (
				identity &&
				(context.setInteraction?.state?.phase === "set_armed" ||
					context.setInteraction?.state?.phase === "group_source_pending")
			)
				void context.setInteraction.choosePlayback(identity, "keyboard");
			else pressPlaybackSlot(context, event, number);
		}
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
	context.pageChord.down(event.code, () => {
		if (context.authority.ready)
			stepPlaybackPage(context, event.code === "PageUp" ? 1 : -1);
	});
	return true;
}

function handleEscape(context: ShortcutContext, event: KeyboardEvent) {
	event.preventDefault();
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

function beginRecordGesture(
	context: ShortcutContext,
	event: KeyboardEvent,
	shifted: boolean,
) {
	if (event.repeat || context.update.active.current) return;
	context.update.active.current = true;
	context.update.held.current = false;
	context.update.mode.current = shifted ? "update" : "record";
	context.update.hold.current = window.setTimeout(() => {
		context.update.held.current = true;
		if (context.update.mode.current === "update") openUpdateSettings();
		else
			context.dispatch({
				type: "SET_MODAL",
				modal: "storeSettingsOpen",
				value: true,
			});
	}, RECORD_HOLD_MS);
}

function beginPreloadGesture(context: ShortcutContext, event: KeyboardEvent) {
	if (event.repeat || context.preload.active.current) return;
	context.preload.active.current = true;
	context.preload.held.current = false;
	context.preload.hold.current = window.setTimeout(() => {
		context.preload.held.current = true;
		context.inspectPreload();
	}, PRELOAD_HOLD_MS);
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
	if (
		event.key === "Shift" ||
		event.code === "ShiftLeft" ||
		event.code === "ShiftRight"
	) {
		event.preventDefault();
		if (!event.repeat && !context.keyboardShiftDown.current) {
			context.keyboardShiftDown.current = true;
			context.dispatch({ type: "SET_SHIFT_ARMED", value: true });
		}
		return;
	}
	if (
		event.shiftKey &&
		/^Digit[1-4]$/.test(event.code) &&
		/^\s*(?:FREEZE|UNFREEZE)\b/i.test(context.commandLine)
	) {
		event.preventDefault();
		context.selectFixtureFreezeFamily(
			event.code.slice(-1) as "1" | "2" | "3" | "4",
		);
		return;
	}
	const key = softwareKeyFromKeyboard(event, context.regularNumberShortcuts);
	if (!key) return;
	const shifted = context.state.shiftArmed || event.shiftKey;
	if (key === "REC" && shifted) {
		event.preventDefault();
		beginRecordGesture(context, event, true);
		return;
	}
	if (key === "CLR" && shifted && !context.pressKey) {
		event.preventDefault();
		context.toggleFixtureFreeze();
		return;
	}
	if (shifted) {
		event.preventDefault();
		if (context.pressKey) context.pressKey(key);
		else applySoftwareEdit(context, key);
		return;
	}
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
		context.pressSet();
	} else if (key === "REC") {
		beginRecordGesture(context, event, event.shiftKey);
	} else if (key === "PRE") {
		beginPreloadGesture(context, event);
	} else if (key === "CLR" && event.shiftKey) {
		context.toggleFixtureFreeze();
	} else if (key === "CLR") {
		context.clear();
	} else if (key === "UND") {
		context.undo();
	} else if (key === "ENT") {
		void context.execute();
	} else {
		applySoftwareEdit(context, key);
	}
}

function handleKeyDown(context: ShortcutContext, event: KeyboardEvent) {
	if (event.defaultPrevented || isExternalEditor(event.target)) return;
	if (event.ctrlKey && !event.metaKey && !event.altKey && event.key.toLowerCase() === "z") {
		event.preventDefault();
		context.undo();
		return;
	}
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
	if (!context.update.held.current) {
		if (context.update.mode.current === "update") context.armUpdateOrMenu();
		else context.toggleRecord();
	}
	context.update.held.current = false;
	context.update.mode.current = null;
}

function finishPreloadGesture(context: ShortcutContext) {
	if (context.preload.hold.current)
		window.clearTimeout(context.preload.hold.current);
	context.preload.hold.current = null;
	context.preload.active.current = false;
	if (!context.preload.held.current) context.advancePreload();
	context.preload.held.current = false;
}

function handleKeyUp(context: ShortcutContext, event: KeyboardEvent) {
	if (
		(event.key === "Shift" ||
			event.code === "ShiftLeft" ||
			event.code === "ShiftRight") &&
		context.keyboardShiftDown.current
	) {
		event.preventDefault();
		context.keyboardShiftDown.current = false;
		context.dispatch({ type: "SET_SHIFT_ARMED", value: false });
		return;
	}
	if (event.code === "PageUp" || event.code === "PageDown") {
		context.pageChord.up(event.code);
		return;
	}
	if (event.code === "End" && context.update.active.current) {
		finishUpdateGesture(context);
		return;
	}
	if (event.code === "Backquote" && context.preload.active.current) {
		finishPreloadGesture(context);
		return;
	}
	releasePlaybackSlot(context, event);
}

function releaseHeldControls(context: ShortcutContext) {
	if (context.update.hold.current)
		window.clearTimeout(context.update.hold.current);
	context.update.hold.current = null;
	context.update.active.current = false;
	context.update.mode.current = null;
	if (context.preload.hold.current)
		window.clearTimeout(context.preload.hold.current);
	context.preload.hold.current = null;
	context.preload.active.current = false;
	context.preload.held.current = false;
	context.heldActions.releaseAll();
	context.pageActions.invalidate();
	context.pageChord.reset();
	if (context.keyboardShiftDown.current) {
		context.keyboardShiftDown.current = false;
		context.dispatch({ type: "SET_SHIFT_ARMED", value: false });
	}
}

export function useCommandLineShortcuts(
	hardware: boolean,
	callbacks: ShortcutCallbacks,
) {
	const { state, dispatch } = useApp();
	const active = !hardware;
	const authority = usePlaybackShortcutAuthority(active);
	const runtimeActions = usePlaybackRuntimeActions();
	const topologyActions = usePlaybackTopologyActions();
	const setInteraction = useSetInteraction();
	const update: UpdateGesture = {
		hold: useRef<number | null>(null),
		active: useRef(false),
		held: useRef(false),
		mode: useRef(null),
	};
	const preload: PreloadGesture = {
		hold: useRef<number | null>(null),
		active: useRef(false),
		held: useRef(false),
	};
	const heldActions = useRef(new KeyboardHeldActions()).current;
	const pageActions = useRef(new KeyboardPageActions()).current;
	const pageChord = useRef(new PageKeyChord()).current;
	const keyboardShiftDown = useRef(false);
	const context = useRef<ShortcutContext | null>(null);
	context.current = {
		state,
		dispatch,
		authority,
		runtimeActions,
		update,
		preload,
		heldActions,
		pageActions,
		pageChord,
		keyboardShiftDown,
		regularNumberShortcuts: state.regularNumberShortcuts,
		setInteraction,
		...callbacks,
	};
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
		const pageStep = ((event: CustomEvent<number>) => {
			const shortcut = current();
			if (shortcut.authority.ready)
				stepPlaybackPage(shortcut, event.detail > 0 ? 1 : -1);
		}) as EventListener;
		const blur = () => {
			heldActions.releaseAll();
			if (keyboardShiftDown.current) {
				keyboardShiftDown.current = false;
				current().dispatch({ type: "SET_SHIFT_ARMED", value: false });
			}
		};
		window.addEventListener("keydown", keydown);
		window.addEventListener("keyup", keyup);
		window.addEventListener("light:playback-page-step", pageStep);
		window.addEventListener("blur", blur);
		return () => {
			window.removeEventListener("keydown", keydown);
			window.removeEventListener("keyup", keyup);
			window.removeEventListener("light:playback-page-step", pageStep);
			window.removeEventListener("blur", blur);
			releaseHeldControls(current());
		};
	}, [active, heldActions]);
}
