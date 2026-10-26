import { StagePaneCommandQueue } from "./stagePaneCommandQueue";
import type {
	ConsoleScreenWindow,
	DesktopBridge,
	DesktopDisplay,
	DesktopUnsubscribe,
	DesktopWindowState,
	StagePaneBenchmarkObservations,
	StageStillEvent,
} from "./types";

const stagePaneCommands = new StagePaneCommandQueue();
let liveStagePaneId: string | null = null;

const coreApi = () => import("@tauri-apps/api/core");
const eventApi = () => import("@tauri-apps/api/event");
const windowApi = () => import("@tauri-apps/api/window");
const dialogApi = () => import("@tauri-apps/plugin-dialog");

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
		currentWindowFullscreen(),
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
	const api = await coreApi();
	return api.invoke<boolean>("current_window_fullscreen");
}

async function setCurrentWindowFullscreen(fullscreen: boolean) {
	await invoke("set_current_window_fullscreen", { fullscreen });
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
		attachment: screen.attachment,
	};
}

export const tauriDesktopBridge: DesktopBridge = {
	available: true,
	selectFolder: async () => {
		const api = await dialogApi();
		const selected = await api.open({
			directory: true,
			multiple: false,
			title: "Select media library",
		});
		return typeof selected === "string" ? selected : null;
	},
	frontendReady: () => invoke("frontend_ready"),
	exitApplication: () => invoke("exit_desktop_app"),
	cancelQuit: () => invoke("cancel_quit"),
	onQuitRequested: (handler) => listen("quit-requested", handler),
	onQuitCancelled: (handler) => listen("quit-cancelled", handler),
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
	openVisualizer: () => invoke("open_visualizer"),
	setStagePaneSelection: (paneId, fixtures) => {
		const snapshot = [...fixtures];
		return stagePaneCommands.run(paneId, () => invoke("set_stage_pane_selection", { paneId, fixtures: snapshot }));
	},
	stagePaneAvailable: async () => {
		const api = await coreApi();
		return api.invoke<boolean>("stage_pane_available");
	},
	openStagePane: (paneId, live3d, geometry) =>
		stagePaneCommands.run(paneId, async () => {
			await invoke("open_stage_pane", { paneId, live3d, geometry });
			if (live3d) liveStagePaneId = paneId;
		}),
	setStagePane: (paneId, geometry) =>
		invoke("set_stage_pane", { paneId, geometry }),
	openStagePaneCapture: (paneId, reveal) => invoke("open_stage_pane_capture", { paneId, reveal }),
	startStagePaneCapture: (paneId, requestId, picture) => {
		const snapshot = structuredClone(picture);
		return stagePaneCommands.run(paneId, () => invoke("start_stage_pane_capture", { paneId, requestId, picture: snapshot }));
	},
	cancelStagePaneCapture: (paneId, jobId) => invoke("cancel_stage_pane_capture", { paneId, jobId }),
	onStageStill: async (handler) => {
		const api = await eventApi();
		return api.listen<StageStillEvent>("stage-still", (event) => handler(event.payload));
	},
	setStagePaneVisibility: (paneId, visible) =>
		invoke("set_stage_pane_visibility", { paneId, visible }),
	closeStagePane: (paneId) => stagePaneCommands.close(paneId, async () => {
		await invoke("close_stage_pane", { paneId });
		if (liveStagePaneId === paneId) liveStagePaneId = null;
	}),
	sendStagePaneInput: (gesture, x, y, paneId) => {
		const owner = paneId ?? liveStagePaneId;
		const operation = () => invoke("stage_pane_input", { paneId: owner ?? undefined, gesture, x, y });
		return owner ? stagePaneCommands.run(owner, operation) : operation();
	},
	setStagePanePicture: (paneId, picture) => {
		const snapshot = structuredClone(picture);
		return stagePaneCommands.picture(paneId, () => invoke("set_stage_pane_picture", { paneId, picture: snapshot }));
	},
	stagePaneCamera: async () => {
		const api = await coreApi();
		return api.invoke<[number, number, number, number, number, number] | null>(
			"stage_pane_camera",
		);
	},
	placeStagePaneCamera: (place) => {
		const snapshot = structuredClone(place);
		const owner = liveStagePaneId;
		const operation = () => invoke("place_stage_pane_camera", { ...snapshot, paneId: owner ?? undefined });
		return owner ? stagePaneCommands.run(owner, operation) : operation();
	},
	takeStagePanePicks: async (paneId) => {
		const api = await coreApi();
		return api.invoke<Array<[string | null, boolean]>>(
			"take_stage_pane_picks",
			{
				paneId,
			},
		);
	},
	stagePaneStatus: async (paneId) => {
		const api = await coreApi();
		return api.invoke<[string | null, string | null]>("stage_pane_status", {
			paneId,
		});
	},
	takeStagePaneBenchmarkSamples: async () => {
		const api = await coreApi();
		return api.invoke<StagePaneBenchmarkObservations>("take_stage_pane_benchmark_samples");
	},
	packagedStageBenchmarkConfig: async () => {
		const api = await coreApi();
		return api.invoke("packaged_stage_benchmark_config");
	},
	packagedStageBenchmarkPrepared: async () => {
		const api = await coreApi();
		return api.invoke("packaged_stage_benchmark_prepared");
	},
	exercisePackagedStageBenchmarkVisibility: async () => {
		const api = await coreApi();
		return api.invoke("exercise_packaged_stage_benchmark_visibility");
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
