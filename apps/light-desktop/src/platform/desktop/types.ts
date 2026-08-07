import type { ScreenConfiguration } from "../../api/types";

export type DesktopUnsubscribe = () => void;

export interface DesktopDisplay {
	id: string;
	name: string;
}

export interface DesktopWindowState {
	displayId: string | null;
	bounds: NonNullable<ScreenConfiguration["bounds"]>;
	fullscreen: boolean;
}

export interface ConsoleScreenWindow {
	screenId: string;
	title: string;
	displayId: string | null;
	bounds: ScreenConfiguration["bounds"];
	fullscreen: boolean;
}

export interface PackagedStageBenchmarkConfig {
	durationSeconds: number;
	controlDurationSeconds: number;
	profile: string;
	additionalStageWindow: boolean;
	fixtureSheet: boolean;
}

/** Where the Stage pane is, in the points the web layout works in. */
export interface StagePaneGeometry {
	x: number;
	y: number;
	width: number;
	height: number;
	scale: number;
	surfaceWidth: number;
	surfaceHeight: number;
}

/** The camera addressed by number rather than by dragging. */
export interface StagePaneCameraPlacement {
	x: number;
	y: number;
	z: number;
	pan: number;
	tilt: number;
	distance: number;
}

export type StagePaneGesture =
	| "orbit"
	| "pan"
	| "truck"
	| "fly"
	| "zoom"
	| "pick"
	| "pick-add";

/** Everything the renderer draws the pane with, as the operator set it. */
export interface StagePanePicture {
	atmosphere: number;
	ambient: number;
	quality: "draft" | "standard" | "high" | "ultra";
	exposure: number;
	laserBrightness: number;
	showLabels: boolean;
}

export interface DesktopBridge {
	readonly available: boolean;
	frontendReady(): Promise<void>;
	exitApplication(): Promise<void>;
	cancelQuit(): Promise<void>;
	onQuitRequested(handler: () => void): Promise<DesktopUnsubscribe>;
	onApplicationShuttingDown(handler: () => void): Promise<DesktopUnsubscribe>;
	listDisplays(): Promise<DesktopDisplay[]>;
	openConsoleScreen(screen: ConsoleScreenWindow): Promise<void>;
	hideConsoleScreen(screenId: string): Promise<void>;
	closeConsoleScreen(screenId: string): Promise<void>;
	openStageViewWindow(): Promise<void>;
	/**
	 * Whether the desk can draw the Stage itself, with the native renderer, into a rectangle of
	 * its own window. False keeps the web renderer, which is not a failure — it is what a browser,
	 * a platform without a shared surface, and an installation missing its renderer all get.
	 */
	stagePaneAvailable(): Promise<boolean>;
	openStagePane(geometry: StagePaneGeometry, user: string): Promise<void>;
	setStagePane(geometry: StagePaneGeometry): Promise<void>;
	closeStagePane(): Promise<void>;
	sendStagePaneInput(gesture: StagePaneGesture, x: number, y: number): Promise<void>;
	/** The picture settings, which belong to the renderer drawing the pane rather than the desk. */
	setStagePanePicture(picture: StagePanePicture): Promise<void>;
	/** What is drawing the pane, and whatever last went wrong with it. */
	stagePaneStatus(): Promise<[string | null, string | null]>;
	/**
	 * What the operator pointed at in the pane since this was last asked, as `[fixtureId, additive]`.
	 * A null fixture is a click on nothing, which clears the selection.
	 */
	takeStagePanePicks(): Promise<Array<[string | null, boolean]>>;
	/** Where the renderer's camera is, as `[x, y, z, pan, tilt, distance]`. */
	stagePaneCamera(): Promise<[number, number, number, number, number, number] | null>;
	/** Put the camera at numbers. Anything omitted is left where it is. */
	placeStagePaneCamera(place: Partial<StagePaneCameraPlacement>): Promise<void>;
	packagedStageBenchmarkConfig(): Promise<PackagedStageBenchmarkConfig | null>;
	packagedStageBenchmarkPrepared(): Promise<boolean>;
	focusPackagedStageBenchmarkWindow(): Promise<void>;
	appendPackagedStageBenchmarkSample(sample: unknown): Promise<void>;
	currentWindowState(): Promise<DesktopWindowState>;
	currentWindowFullscreen(): Promise<boolean>;
	setCurrentWindowFullscreen(fullscreen: boolean): Promise<void>;
	closeCurrentWindow(): Promise<void>;
	destroyCurrentWindow(): Promise<void>;
	startCurrentWindowDrag(): Promise<void>;
	onCurrentWindowMoved(handler: () => void): Promise<DesktopUnsubscribe>;
	onCurrentWindowResized(handler: () => void): Promise<DesktopUnsubscribe>;
	onCurrentWindowCloseRequested(
		handler: () => void | Promise<void>,
	): Promise<DesktopUnsubscribe>;
}
