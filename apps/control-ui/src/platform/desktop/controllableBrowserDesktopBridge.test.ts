import { describe, expect, it, vi } from "vitest";
import {
	controllableBrowserDesktopBridge,
	type ControllableDesktopAction,
	type ControllableDesktopEvent,
	type ControllableDesktopPort,
} from "./controllableBrowserDesktopBridge";

describe("controllable browser desktop bridge", () => {
	it("maps only explicit desktop capabilities", async () => {
		const actions: ControllableDesktopAction[] = [];
		const listeners = new Map<ControllableDesktopEvent, () => void>();
		const port: ControllableDesktopPort = {
			perform: (action) => {
				actions.push(action);
			},
			listDisplays: () => [{ id: "display-a", name: "Preview" }],
			currentWindowState: () => ({
				displayId: "display-a",
				bounds: { x: 1, y: 2, width: 800, height: 600 },
				fullscreen: false,
			}),
			subscribe: (event, handler) => {
				listeners.set(event, handler);
				return () => listeners.delete(event);
			},
		};
		const bridge = controllableBrowserDesktopBridge(port);
		await bridge.frontendReady();
		await bridge.openConsoleScreen({
			screenId: "stage",
			title: "Stage",
			displayId: "display-a",
			bounds: null,
			fullscreen: true,
		});
		await bridge.setCurrentWindowFullscreen(true);
		expect(actions).toEqual([
			{ type: "frontend_ready" },
			{
				type: "open_console_screen",
				screen: {
					screenId: "stage",
					title: "Stage",
					displayId: "display-a",
					bounds: null,
					fullscreen: true,
				},
			},
			{ type: "set_fullscreen", fullscreen: true },
		]);
		await expect(bridge.listDisplays()).resolves.toEqual([
			{ id: "display-a", name: "Preview" },
		]);
	});

	it("owns typed event subscription and disposal", async () => {
		let closeRequested: (() => void) | undefined;
		const unsubscribe = vi.fn();
		const bridge = controllableBrowserDesktopBridge({
			perform: () => undefined,
			listDisplays: () => [],
			currentWindowState: () => ({
				displayId: null,
				bounds: { x: 0, y: 0, width: 0, height: 0 },
				fullscreen: false,
			}),
			subscribe: (event, handler) => {
				if (event === "current_window_close_requested")
					closeRequested = handler;
				return unsubscribe;
			},
		});
		const handler = vi.fn();
		expect(await bridge.onCurrentWindowCloseRequested(handler)).toBe(
			unsubscribe,
		);
		closeRequested?.();
		expect(handler).toHaveBeenCalledOnce();
	});

	it("rejects malformed controlled state", async () => {
		const bridge = controllableBrowserDesktopBridge({
			perform: () => undefined,
			listDisplays: () => [],
			currentWindowState: () =>
				({ fullscreen: "yes" } as unknown as ReturnType<
					ControllableDesktopPort["currentWindowState"]
				>),
			subscribe: () => () => undefined,
		});
		await expect(bridge.currentWindowState()).rejects.toThrow(
			"Invalid controllable display identity",
		);
		await expect(
			controllableBrowserDesktopBridge({
				perform: () => undefined,
				listDisplays: () => [{ id: "", name: "Preview" }],
				currentWindowState: () => ({
					displayId: null,
					bounds: { x: 0, y: 0, width: 10, height: 10 },
					fullscreen: false,
				}),
				subscribe: () => () => undefined,
			}).listDisplays(),
		).rejects.toThrow("Invalid controllable display id");
	});
});
