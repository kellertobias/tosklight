import type { DesktopBridge, DesktopUnsubscribe } from "./types";

const noSubscription = async (): Promise<DesktopUnsubscribe> => () => undefined;
const noAction = async (): Promise<void> => undefined;

export const browserDesktopBridge: DesktopBridge = {
	available: false,
	frontendReady: noAction,
	exitApplication: noAction,
	cancelQuit: noAction,
	onQuitRequested: noSubscription,
	onApplicationShuttingDown: noSubscription,
	listDisplays: async () => [],
	openConsoleScreen: noAction,
	hideConsoleScreen: noAction,
	closeConsoleScreen: noAction,
	currentWindowState: async () => ({
		displayId: null,
		bounds: { x: 0, y: 0, width: 0, height: 0 },
		fullscreen: false,
	}),
	currentWindowFullscreen: async () => false,
	setCurrentWindowFullscreen: noAction,
	closeCurrentWindow: noAction,
	destroyCurrentWindow: noAction,
	startCurrentWindowDrag: noAction,
	onCurrentWindowMoved: noSubscription,
	onCurrentWindowResized: noSubscription,
	onCurrentWindowCloseRequested: noSubscription,
};
