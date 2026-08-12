import type {
	ConsoleScreenWindow,
	DesktopBridge,
	DesktopDisplay,
	DesktopUnsubscribe,
	DesktopWindowState,
} from "./types";

const coreApi = () => import("@tauri-apps/api/core");
const eventApi = () => import("@tauri-apps/api/event");
const windowApi = () => import("@tauri-apps/api/window");

async function invoke(command: string, args?: Record<string, unknown>) {
	const api = await coreApi();
	await api.invoke(command, args);
}

async function listen(
	event: string,
	handler: () => void,
): Promise<DesktopUnsubscribe> {
	const api = await eventApi();
	return api.listen(event, handler);
}

interface MonitorIdentity {
	name: string | null;
	position: { x: number; y: number };
	size: { width: number; height: number };
}

function displayId(monitor: MonitorIdentity | null) {
	if (!monitor) return null;
	const { position, size } = monitor;
	return `${monitor.name ?? "Display"}|${position.x},${position.y}|${size.width}x${size.height}`;
}

async function currentWindowState(): Promise<DesktopWindowState> {
	const api = await windowApi();
	const current = api.getCurrentWindow();
	const [position, size, scale, fullscreen, monitor] = await Promise.all([
		current.outerPosition(),
		current.outerSize(),
		current.scaleFactor(),
		current.isFullscreen(),
		api.currentMonitor(),
	]);
	return {
		displayId: displayId(monitor),
		bounds: {
			x: position.x / scale,
			y: position.y / scale,
			width: size.width / scale,
			height: size.height / scale,
		},
		fullscreen,
	};
}

async function currentWindowFullscreen() {
	const api = await windowApi();
	return api.getCurrentWindow().isFullscreen();
}

async function setCurrentWindowFullscreen(fullscreen: boolean) {
	const api = await windowApi();
	await api.getCurrentWindow().setFullscreen(fullscreen);
}

async function closeCurrentWindow() {
	const api = await windowApi();
	await api.getCurrentWindow().close();
}

async function destroyCurrentWindow() {
	const api = await windowApi();
	await api.getCurrentWindow().destroy();
}

async function startCurrentWindowDrag() {
	const api = await windowApi();
	await api.getCurrentWindow().startDragging();
}

async function onMoved(handler: () => void) {
	const api = await windowApi();
	return api.getCurrentWindow().onMoved(handler);
}

async function onResized(handler: () => void) {
	const api = await windowApi();
	return api.getCurrentWindow().onResized(handler);
}

async function onCloseRequested(handler: () => void | Promise<void>) {
	const api = await windowApi();
	return api.getCurrentWindow().onCloseRequested((event) => {
		event.preventDefault();
		void handler();
	});
}

function screenArguments(screen: ConsoleScreenWindow) {
	return {
		screenId: screen.screenId,
		title: screen.title,
		displayId: screen.displayId,
		bounds: screen.bounds,
		fullscreen: screen.fullscreen,
	};
}

export const tauriDesktopBridge: DesktopBridge = {
	available: true,
	frontendReady: () => invoke("frontend_ready"),
	exitApplication: () => invoke("exit_desktop_app"),
	cancelQuit: () => invoke("cancel_quit"),
	onQuitRequested: (handler) => listen("quit-requested", handler),
	onApplicationShuttingDown: (handler) => listen("app-shutting-down", handler),
	listDisplays: async () => {
		const api = await coreApi();
		return api.invoke<DesktopDisplay[]>("list_console_displays");
	},
	openConsoleScreen: (screen) =>
		invoke("open_console_screen", screenArguments(screen)),
	hideConsoleScreen: (screenId) => invoke("hide_console_screen", { screenId }),
	closeConsoleScreen: (screenId) =>
		invoke("close_console_screen", { screenId }),
	setStagePaneSelection: (paneId, fixtures) =>
		invoke("set_stage_pane_selection", { paneId, fixtures }),
	stagePaneAvailable: async () => {
		const api = await coreApi();
		return api.invoke<boolean>("stage_pane_available");
	},
	openStagePane: (paneId, live3d, geometry, user) =>
		invoke("open_stage_pane", { paneId, live3d, geometry, user }),
	setStagePane: (paneId, geometry) =>
		invoke("set_stage_pane", { paneId, geometry }),
	closeStagePane: (paneId) => invoke("close_stage_pane", { paneId }),
	sendStagePaneInput: (gesture, x, y, paneId) =>
		invoke("stage_pane_input", { paneId, gesture, x, y }),
	setStagePanePicture: (paneId, picture) =>
		invoke("set_stage_pane_picture", { paneId, picture }),
	stagePaneCamera: async () => {
		const api = await coreApi();
		return api.invoke<[number, number, number, number, number, number] | null>(
			"stage_pane_camera",
		);
	},
	placeStagePaneCamera: (place) => invoke("place_stage_pane_camera", place),
	takeStagePanePicks: async (paneId) => {
		const api = await coreApi();
		return api.invoke<Array<[string | null, boolean]>>("take_stage_pane_picks", {
			paneId,
		});
	},
	stagePaneStatus: async (paneId) => {
		const api = await coreApi();
		return api.invoke<[string | null, string | null]>("stage_pane_status", {
			paneId,
		});
	},
	packagedStageBenchmarkConfig: async () => {
		const api = await coreApi();
		return api.invoke("packaged_stage_benchmark_config");
	},
	packagedStageBenchmarkPrepared: async () => {
		const api = await coreApi();
		return api.invoke("packaged_stage_benchmark_prepared");
	},
	focusPackagedStageBenchmarkWindow: () =>
		invoke("focus_packaged_stage_benchmark_window"),
	appendPackagedStageBenchmarkSample: async (sample) => {
		const api = await coreApi();
		await api.invoke("append_packaged_stage_benchmark_sample", { sample });
	},
	currentWindowState,
	currentWindowFullscreen,
	setCurrentWindowFullscreen,
	closeCurrentWindow,
	destroyCurrentWindow,
	startCurrentWindowDrag,
	onCurrentWindowMoved: onMoved,
	onCurrentWindowResized: onResized,
	onCurrentWindowCloseRequested: onCloseRequested,
};
