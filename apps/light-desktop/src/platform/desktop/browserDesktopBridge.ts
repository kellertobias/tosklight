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
	openStageViewWindow: noAction,
	// A browser has no second process and no window to draw underneath, so the Stage is always
	// the web renderer here. Saying so plainly is what keeps the pane from being asked for.
	stagePaneAvailable: async () => false,
	openStagePane: noAction,
	setStagePane: noAction,
	closeStagePane: noAction,
	sendStagePaneInput: noAction,
	setStagePanePicture: noAction,
	stagePaneStatus: async () => [null, null],
	packagedStageBenchmarkConfig: async () => null,
	packagedStageBenchmarkPrepared: async () => false,
	focusPackagedStageBenchmarkWindow: noAction,
	appendPackagedStageBenchmarkSample: noAction,
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
