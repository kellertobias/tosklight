import { capturePicture } from "../../windows/stageWindow/stageCaptureTestFixture";
import { beforeEach, describe, expect, it, vi } from "vitest";
import desktopCapability from "../../../src-tauri/capabilities/default.json";
import type { ScreenAttachment } from "../../api/client/screenAttachment";

const mocks = vi.hoisted(() => ({
	invoke: vi.fn(),
	listen: vi.fn(),
	currentMonitor: vi.fn(),
	currentWindow: {
		outerPosition: vi.fn(),
		outerSize: vi.fn(),
		scaleFactor: vi.fn(),
		isFullscreen: vi.fn(),
		setFullscreen: vi.fn(),
		close: vi.fn(),
		destroy: vi.fn(),
		startDragging: vi.fn(),
		onMoved: vi.fn(),
		onResized: vi.fn(),
		onCloseRequested: vi.fn(),
	},
	openDialog: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("@tauri-apps/api/window", () => ({
	currentMonitor: mocks.currentMonitor,
	getCurrentWindow: () => mocks.currentWindow,
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: mocks.openDialog }));

import { tauriDesktopBridge } from "./tauriDesktopBridge";

describe("Tauri desktop bridge", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		mocks.invoke.mockImplementation(async (command: string) =>
			command === "current_window_fullscreen" ? true : undefined,
		);
		mocks.currentWindow.outerPosition.mockResolvedValue({ x: 200, y: 100 });
		mocks.currentWindow.outerSize.mockResolvedValue({
			width: 1600,
			height: 900,
		});
		mocks.currentWindow.scaleFactor.mockResolvedValue(2);
		mocks.currentWindow.isFullscreen.mockResolvedValue(true);
		mocks.currentMonitor.mockResolvedValue({
			name: "Desk display",
			position: { x: 0, y: 0 },
			size: { width: 1920, height: 1080 },
		});
	});

	it("maps typed screen operations to desktop commands", async () => {
		const attachment = {
			server_url: "http://127.0.0.1:5000",
			session: {
				role: "operator",
				session_id: "session-1",
				client_id: "client-1",
				token: "token-1",
				desk: { id: "desk-1" },
			},
			desk_token: null,
		} as unknown as ScreenAttachment;
		await tauriDesktopBridge.openConsoleScreen({
			screenId: "stage",
			title: "Stage",
			displayId: "display-1",
			bounds: { x: 10, y: 20, width: 800, height: 600 },
			fullscreen: false,
			attachment,
		});
		expect(mocks.invoke).toHaveBeenCalledWith("open_console_screen", {
			screenId: "stage",
			title: "Stage",
			displayId: "display-1",
			bounds: { x: 10, y: 20, width: 800, height: 600 },
			fullscreen: false,
			attachment,
		});
	});

	it("opens the dedicated Stage renderer through the desktop command", async () => {
		await tauriDesktopBridge.openVisualizer();
		expect(mocks.invoke).toHaveBeenCalledWith("open_visualizer", undefined);
	});

	it("chooses one native media-library folder and treats cancel as neutral", async () => {
		mocks.openDialog.mockResolvedValueOnce("/Volumes/Show/Audio");
		await expect(tauriDesktopBridge.selectFolder()).resolves.toBe(
			"/Volumes/Show/Audio",
		);
		expect(mocks.openDialog).toHaveBeenCalledWith({
			directory: true,
			multiple: false,
			title: "Select media library",
		});
		mocks.openDialog.mockResolvedValueOnce(null);
		await expect(tauriDesktopBridge.selectFolder()).resolves.toBeNull();
	});

	it("normalizes native geometry into logical screen coordinates", async () => {
		await expect(tauriDesktopBridge.currentWindowState()).resolves.toEqual({
			displayId: "Desk display|0,0|1920x1080",
			bounds: { x: 100, y: 50, width: 800, height: 450 },
			fullscreen: true,
		});
		expect(mocks.invoke).toHaveBeenCalledWith("current_window_fullscreen");
	});

	it("uses the native window command for both fullscreen transitions", async () => {
		await expect(tauriDesktopBridge.currentWindowFullscreen()).resolves.toBe(
			true,
		);
		await tauriDesktopBridge.setCurrentWindowFullscreen(true);
		await tauriDesktopBridge.setCurrentWindowFullscreen(false);
		expect(mocks.invoke.mock.calls).toEqual([
			["current_window_fullscreen"],
			["set_current_window_fullscreen", { fullscreen: true }],
			["set_current_window_fullscreen", { fullscreen: false }],
		]);
		expect(mocks.currentWindow.setFullscreen).not.toHaveBeenCalled();
	});

	it("owns native close prevention before notifying the application", async () => {
		let nativeHandler:
			| ((event: { preventDefault: () => void }) => void)
			| undefined;
		const unsubscribe = vi.fn();
		mocks.currentWindow.onCloseRequested.mockImplementation(async (handler) => {
			nativeHandler = handler;
			return unsubscribe;
		});
		const requested = vi.fn();
		expect(
			await tauriDesktopBridge.onCurrentWindowCloseRequested(requested),
		).toBe(unsubscribe);
		const preventDefault = vi.fn();
		nativeHandler?.({ preventDefault });
		expect(preventDefault).toHaveBeenCalledOnce();
		expect(requested).toHaveBeenCalledOnce();
	});

	it("routes frozen image commands and pushed native status without polling or frontend paths", async () => {
		const handler = vi.fn();
		await tauriDesktopBridge.startStagePaneCapture?.("preload", "request", capturePicture());
		await tauriDesktopBridge.chooseStagePaneCapture?.("preload", "job", "ultimate");
		await tauriDesktopBridge.cancelStagePaneCapture?.("preload", "job");
		await tauriDesktopBridge.openStagePaneCapture?.("preload", true);
		await tauriDesktopBridge.onStageStill?.(handler);
		expect(mocks.invoke.mock.calls).toEqual([
			["start_stage_pane_capture", { paneId: "preload", requestId: "request", picture: capturePicture() }],
			["choose_stage_pane_capture", { paneId: "preload", jobId: "job", engine: "ultimate" }],
			["cancel_stage_pane_capture", { paneId: "preload", jobId: "job" }],
			["open_stage_pane_capture", { paneId: "preload", reveal: true }],
		]);
		expect(mocks.listen).toHaveBeenCalledWith("stage-still", expect.any(Function));
		const payload = { paneId: "preload", requestId: "request", status: null, error: "helper closed" };
		mocks.listen.mock.calls.at(-1)?.[1]({ payload });
		expect(handler).toHaveBeenCalledWith(payload);
	});

	// Every native window command the bridge issues is rejected at runtime unless the desktop
	// capability grants it, and a rejected destroy leaves a screen window that cannot be closed.
	it("runs on capabilities that grant every native window command it issues", () => {
		expect(desktopCapability.permissions).toEqual(
			expect.arrayContaining([
				"core:window:allow-close",
				"core:window:allow-destroy",
				"core:window:allow-is-fullscreen",
				"core:window:allow-set-fullscreen",
				"core:window:allow-start-dragging",
				"dialog:default",
			]),
		);
		expect(desktopCapability.windows).toEqual(
			expect.arrayContaining(["main", "screen-*"]),
		);
	});
});


it("reports resident Stage visibility through its scoped native command", async () => {
	await tauriDesktopBridge.setStagePaneVisibility("pane-7", false);
	expect(mocks.invoke).toHaveBeenLastCalledWith("set_stage_pane_visibility", { paneId: "pane-7", visible: false });
});

it("pins a queued numeric camera placement to its invocation owner", async () => {
	Reflect.set(window, "__TAURI_INTERNALS__", { invoke: mocks.invoke });
	const geometry = { x: 0, y: 0, width: 640, height: 480, scale: 1, surfaceWidth: 640, surfaceHeight: 480 };
	let release!: () => void;
	const blocked = new Promise<void>((resolve) => { release = resolve; });
	mocks.invoke.mockImplementation(async (command: string) => {
		if (command === "set_stage_pane_picture") await blocked;
	});
	await tauriDesktopBridge.openStagePane("old-owner", true, geometry);
	const pending = tauriDesktopBridge.setStagePanePicture("old-owner", capturePicture());
	const placement = tauriDesktopBridge.placeStagePaneCamera({ pan: 25 });
	// Simulate a different live owner before the delayed old-owner operation executes.
	await tauriDesktopBridge.openStagePane("new-owner", true, geometry);
	release(); await pending; await placement;
	expect(mocks.invoke).toHaveBeenCalledWith("place_stage_pane_camera", { pan: 25, paneId: "old-owner" });
	await tauriDesktopBridge.closeStagePane("old-owner");
	await tauriDesktopBridge.closeStagePane("new-owner");
	await tauriDesktopBridge.placeStagePaneCamera({ tilt: 10 });
	expect(mocks.invoke).toHaveBeenLastCalledWith("place_stage_pane_camera", { tilt: 10, paneId: undefined });
	Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
