import type {
	ConsoleScreenWindow,
	DesktopBridge,
	DesktopDisplay,
	DesktopUnsubscribe,
	DesktopWindowState,
} from "./types";

export const DESKTOP_TEST_CONTROL = "__lightDesktopTestControl";

export type ControllableDesktopAction =
	| { type: "frontend_ready" }
	| { type: "exit_application" }
	| { type: "cancel_quit" }
	| { type: "open_console_screen"; screen: ConsoleScreenWindow }
	| { type: "hide_console_screen"; screen_id: string }
	| { type: "close_console_screen"; screen_id: string }
	| { type: "set_fullscreen"; fullscreen: boolean }
	| { type: "close_current_window" }
	| { type: "destroy_current_window" }
	| { type: "start_current_window_drag" };

export type ControllableDesktopEvent =
	| "quit_requested"
	| "application_shutting_down"
	| "current_window_moved"
	| "current_window_resized"
	| "current_window_close_requested";

export interface ControllableDesktopPort {
	perform(action: ControllableDesktopAction): void | Promise<void>;
	listDisplays(): DesktopDisplay[] | Promise<DesktopDisplay[]>;
	currentWindowState(): DesktopWindowState | Promise<DesktopWindowState>;
	subscribe(
		event: ControllableDesktopEvent,
		handler: () => void,
	): DesktopUnsubscribe;
}

export type ControllableDesktopWindow = Window & {
	[DESKTOP_TEST_CONTROL]?: ControllableDesktopPort;
};

export function controllableBrowserDesktopBridge(
	port: ControllableDesktopPort,
): DesktopBridge {
	const perform = async (action: ControllableDesktopAction) => {
		await port.perform(action);
	};
	const subscribe = async (
		event: ControllableDesktopEvent,
		handler: () => void,
	) => {
		const unsubscribe = port.subscribe(event, handler);
		if (typeof unsubscribe !== "function")
			throw new Error("Invalid controllable desktop subscription");
		return unsubscribe;
	};
	return {
		available: true,
		frontendReady: () => perform({ type: "frontend_ready" }),
		exitApplication: () => perform({ type: "exit_application" }),
		cancelQuit: () => perform({ type: "cancel_quit" }),
		onQuitRequested: (handler) => subscribe("quit_requested", handler),
		onApplicationShuttingDown: (handler) =>
			subscribe("application_shutting_down", handler),
		listDisplays: async () => decodeDisplays(await port.listDisplays()),
		openConsoleScreen: (screen) =>
			perform({ type: "open_console_screen", screen }),
		hideConsoleScreen: (screenId) =>
			perform({ type: "hide_console_screen", screen_id: screenId }),
		closeConsoleScreen: (screenId) =>
			perform({ type: "close_console_screen", screen_id: screenId }),
		currentWindowState: async () => decodeWindowState(await port.currentWindowState()),
		currentWindowFullscreen: async () =>
			decodeWindowState(await port.currentWindowState()).fullscreen,
		setCurrentWindowFullscreen: (fullscreen) =>
			perform({ type: "set_fullscreen", fullscreen }),
		closeCurrentWindow: () => perform({ type: "close_current_window" }),
		destroyCurrentWindow: () => perform({ type: "destroy_current_window" }),
		startCurrentWindowDrag: () =>
			perform({ type: "start_current_window_drag" }),
		onCurrentWindowMoved: (handler) =>
			subscribe("current_window_moved", handler),
		onCurrentWindowResized: (handler) =>
			subscribe("current_window_resized", handler),
		onCurrentWindowCloseRequested: (handler) =>
			subscribe("current_window_close_requested", () => void handler()),
	};
}

function decodeDisplays(value: unknown): DesktopDisplay[] {
	if (!Array.isArray(value)) throw new Error("Invalid controllable display list");
	return value.map((item) => {
		const display = record(item, "controllable display");
		return {
			id: requiredString(display.id, "controllable display id"),
			name: requiredString(display.name, "controllable display name"),
		};
	});
}

function decodeWindowState(value: unknown): DesktopWindowState {
	const state = record(value, "controllable window state");
	const displayId = state.displayId;
	if (displayId !== null && (typeof displayId !== "string" || !displayId))
		throw new Error("Invalid controllable display identity");
	if (typeof state.fullscreen !== "boolean")
		throw new Error("Invalid controllable fullscreen state");
	return {
		displayId,
		bounds: decodeBounds(state.bounds),
		fullscreen: state.fullscreen,
	};
}

function decodeBounds(value: unknown): DesktopWindowState["bounds"] {
	const bounds = record(value, "controllable window bounds");
	const decoded = {
		x: finiteNumber(bounds.x, "bounds x"),
		y: finiteNumber(bounds.y, "bounds y"),
		width: finiteNumber(bounds.width, "bounds width"),
		height: finiteNumber(bounds.height, "bounds height"),
	};
	if (decoded.width < 0 || decoded.height < 0)
		throw new Error("Invalid controllable window dimensions");
	return decoded;
}

function record(value: unknown, label: string): Record<string, unknown> {
	if (!value || typeof value !== "object" || Array.isArray(value))
		throw new Error(`Invalid ${label}`);
	return value as Record<string, unknown>;
}

function requiredString(value: unknown, label: string): string {
	if (typeof value !== "string" || !value) throw new Error(`Invalid ${label}`);
	return value;
}

function finiteNumber(value: unknown, label: string): number {
	if (typeof value !== "number" || !Number.isFinite(value))
		throw new Error(`Invalid ${label}`);
	return value;
}

export function injectedDesktopPort(
	runtime: ControllableDesktopWindow,
): ControllableDesktopPort | null {
	const port = runtime[DESKTOP_TEST_CONTROL];
	if (
		!port ||
		typeof port.perform !== "function" ||
		typeof port.listDisplays !== "function" ||
		typeof port.currentWindowState !== "function" ||
		typeof port.subscribe !== "function"
	)
		return null;
	return port;
}
