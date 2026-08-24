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
	expectedFixtureRecords: number | null;
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
	| "frame"
	| "pick"
	| "pick-add";

/** Everything the renderer draws the pane with, as the operator set it. */
export interface StagePanePicture {
	atmosphere: number;
	ambient: number;
	quality: "draft" | "standard" | "high" | "ultra";
	exposure: number;
	laserBrightness: number;
	lampFogCloudiness: number;
	lampFogTurbulence: number;
	laserFogCloudiness: number;
	laserFogTurbulence: number;
	showLabels: boolean;
	/** Lay the reference grid on the ground plane. */
	floorGrid: boolean;
	/** Draw dashed direction guides for unlit fixtures in the 3D diagram. */
	showBeamGuides: boolean;
	/** The colour behind the rig, as linear RGB. */
	background: [number, number, number];
	/** Which way the Stage is being looked at. */
	mode: string;
	/** Draw what the operator has preloaded, over what is currently lit. */
	followPreload: boolean;
}

export interface StagePaneBenchmarkSample {
	paneId: string;
	sequence: number;
	sourceFrame: number;
	sourceInputEpochMicros: number;
	presentedEpochMicros: number;
	cpuMicros: number;
	acquireMicros: number;
	gpuMicros: number | null;
	instances: number;
	drawCalls: number;
	degraded: boolean;
	renderer: string;
	quality: "draft" | "standard" | "high" | "ultra";
	followPreload: boolean;
	width: number;
	height: number;
}

export interface DesktopBridge {
	readonly available: boolean;
	/** Choose one machine-local folder without exposing native APIs to feature code. */
	selectFolder(): Promise<string | null>;
	frontendReady(): Promise<void>;
	exitApplication(): Promise<void>;
	cancelQuit(): Promise<void>;
	onQuitRequested(handler: () => void): Promise<DesktopUnsubscribe>;
	onApplicationShuttingDown(handler: () => void): Promise<DesktopUnsubscribe>;
	listDisplays(): Promise<DesktopDisplay[]>;
	openConsoleScreen(screen: ConsoleScreenWindow): Promise<void>;
	hideConsoleScreen(screenId: string): Promise<void>;
	closeConsoleScreen(screenId: string): Promise<void>;
	/** Open the desk-owned Stage renderer as a dedicated native window. */
	openVisualizer(): Promise<void>;
	/**
	 * Whether the renderer can draw the Stage into a rectangle of the desk's own window.
	 *
	 * False is what a browser, a platform without a shared surface, and an installation missing
	 * its renderer all get. There is no second drawing behind it any more, so the Stage says it
	 * cannot be drawn here rather than quietly showing a different picture of the same rig.
	 */
	stagePaneAvailable(): Promise<boolean>;
	openStagePane(
		paneId: string,
		live3d: boolean,
		geometry: StagePaneGeometry,
	): Promise<void>;
	setStagePane(paneId: string, geometry: StagePaneGeometry): Promise<void>;
	closeStagePane(paneId: string): Promise<void>;
	sendStagePaneInput(
		gesture: StagePaneGesture,
		x: number,
		y: number,
		paneId?: string,
	): Promise<void>;
	/** The picture settings, which belong to the renderer drawing the pane rather than the desk. */
	setStagePanePicture(paneId: string, picture: StagePanePicture): Promise<void>;
	/** What the operator has selected, which the renderer draws and never decides. */
	setStagePaneSelection(paneId: string, fixtures: string[]): Promise<void>;
	/** What is drawing the pane, and whatever last went wrong with it. */
	stagePaneStatus(paneId?: string): Promise<[string | null, string | null]>;
	takeStagePaneBenchmarkSamples(): Promise<StagePaneBenchmarkSample[]>;
	/**
	 * What the operator pointed at in the pane since this was last asked, as `[fixtureId, additive]`.
	 * A null fixture is a click on nothing, which clears the selection.
	 */
	takeStagePanePicks(paneId: string): Promise<Array<[string | null, boolean]>>;
	/** Where the renderer's camera is, as `[x, y, z, pan, tilt, distance]`. */
	stagePaneCamera(): Promise<
		[number, number, number, number, number, number] | null
	>;
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
