import { beforeEach, describe, expect, it, vi } from "vitest";
import desktopCapability from "../../../src-tauri/capabilities/default.json";

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
		mocks.invoke.mockResolvedValue(undefined);
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
		await tauriDesktopBridge.openConsoleScreen({
			screenId: "stage",
			title: "Stage",
			displayId: "display-1",
			bounds: { x: 10, y: 20, width: 800, height: 600 },
			fullscreen: false,
		});
		expect(mocks.invoke).toHaveBeenCalledWith("open_console_screen", {
			screenId: "stage",
			title: "Stage",
			displayId: "display-1",
			bounds: { x: 10, y: 20, width: 800, height: 600 },
			fullscreen: false,
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
