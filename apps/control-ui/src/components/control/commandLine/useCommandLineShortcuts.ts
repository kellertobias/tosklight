import { type Dispatch, useEffect, useRef } from "react";
import { useServer } from "../../../api/ServerContext";
import type { ServerContextValue } from "../../../features/server/ServerContextValue";
import { useApp } from "../../../state/AppContext";
import type { Action } from "../../../state/appReducer";
import type { AppState } from "../../../types";
import type { CommandTargetMode } from "../../../controlSurface/commandTarget";
import { canAdvancePlaybackPage } from "../PlaybackPageDialogs";
import {
	editTargetedCommandWithSoftwareKey,
	softwareKeyFromKeyboard,
} from "../softwareKeypad";
import { openUpdateSettings } from "../updateWorkflow";

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

interface ShortcutContext extends ShortcutCallbacks {
	state: AppState;
	dispatch: Dispatch<Action>;
	server: ServerContextValue;
	update: UpdateGesture;
	flashes: Map<string, number>;
}

function isExternalEditor(target: EventTarget | null) {
	if (!(target instanceof HTMLElement)) return false;
	return Boolean(
		target.closest("input,textarea,select,[contenteditable=true]") &&
			!target.closest(".command-input"),
	);
}

function triggerPlaybackButton(
	context: ShortcutContext,
	event: KeyboardEvent,
	slot: number,
) {
	const { server } = context;
	const page = server.playbacks?.pages.find(
		(candidate) => candidate.number === server.playbacks?.active_page,
	);
	const playbackNumber = page?.slots[String(slot)];
	const definition = server.playbacks?.pool.find(
		(candidate) => candidate.number === playbackNumber,
	);
	const action = definition?.buttons[0];
	if (!definition || !action || action === "none") return;
	if (action !== "flash") {
		void server.poolPlaybackAction(
			definition.number,
			action.replaceAll("_", "-") as Parameters<
				typeof server.poolPlaybackAction
			>[1],
		);
		return;
	}
	if (event.repeat) return;
	context.flashes.set(event.code, definition.number);
	void server.poolPlaybackAction(definition.number, "flash", { pressed: true });
}

function handleFunctionKey(context: ShortcutContext, event: KeyboardEvent) {
	if (!/^F(?:[1-9]|1[0-3])$/.test(event.key)) return false;
	event.preventDefault();
	const number = Number(event.key.slice(1));
	if (number <= 8) {
		triggerPlaybackButton(context, event, number);
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

function activatePlaybackPage(context: ShortcutContext, page: number) {
	context.dispatch({ type: "SET_PLAYBACK_PAGE", page: page - 1 });
	void context.server.setPlaybackPage(page);
}

function createPlaybackPage(context: ShortcutContext, page: number) {
	void context.server
		.savePlaybackPage({ number: page, name: `Page ${page}`, slots: {} })
		.then((saved) => {
			if (saved) activatePlaybackPage(context, page);
		});
}

function handlePageKey(context: ShortcutContext, event: KeyboardEvent) {
	if (event.code !== "PageUp" && event.code !== "PageDown") return false;
	event.preventDefault();
	const { server, state } = context;
	const current = server.playbacks?.active_page ?? state.playbackPage + 1;
	const pages = server.playbacks?.pages ?? [];
	const page = current + (event.code === "PageUp" ? 1 : -1);
	if (page < 1) return true;
	if (pages.some((item) => item.number === page)) {
		activatePlaybackPage(context, page);
	} else if (
		event.code === "PageUp" &&
		canAdvancePlaybackPage(pages, current)
	) {
		createPlaybackPage(context, page);
	}
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
	const playbackNumber = context.flashes.get(event.code);
	if (playbackNumber == null) return;
	context.flashes.delete(event.code);
	void context.server.poolPlaybackAction(playbackNumber, "flash", {
		pressed: false,
	});
}

function releaseHeldControls(context: ShortcutContext) {
	if (context.update.hold.current)
		window.clearTimeout(context.update.hold.current);
	context.update.hold.current = null;
	context.update.active.current = false;
	for (const playbackNumber of context.flashes.values()) {
		void context.server.poolPlaybackAction(playbackNumber, "flash", {
			pressed: false,
		});
	}
	context.flashes.clear();
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
	const server = useServer();
	const update: UpdateGesture = {
		hold: useRef<number | null>(null),
		active: useRef(false),
		held: useRef(false),
	};
	const flashes = useRef(new Map<string, number>());
	const context = useRef<ShortcutContext | null>(null);
	context.current = {
		state,
		dispatch,
		server,
		update,
		flashes: flashes.current,
		...callbacks,
	};
	useRunningMenuShortcut(hardware);
	useEffect(() => {
		if (hardware || !state.regularNumberShortcuts) return;
		const current = () => context.current as ShortcutContext;
		const keydown = (event: KeyboardEvent) => handleKeyDown(current(), event);
		const keyup = (event: KeyboardEvent) => handleKeyUp(current(), event);
		window.addEventListener("keydown", keydown);
		window.addEventListener("keyup", keyup);
		return () => {
			window.removeEventListener("keydown", keydown);
			window.removeEventListener("keyup", keyup);
			releaseHeldControls(current());
		};
	}, [hardware, state.regularNumberShortcuts]);
}
