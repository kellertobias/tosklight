import type { PresetFamily } from "./presetFamilies";

export const GRID_COLUMNS = 24;
export const GRID_ROWS = 18;

export type BuiltInWindow =
	| "stage"
	| "groups"
	| "fixtures"
	// Persisted only so pre-retirement desk layouts can be decoded and discarded safely.
	| "layout"
	| "presets"
	| "cuelists"
	| "cuelist_pool"
	| "cues"
	// Persisted layout aliases from before the Cuelist terminology migration.
	| "qlists"
	| "qlist_pool"
	| "qs"
	| "playback"
	| "playback_pool"
	| "cue_list"
	| "dynamics"
	| "macros"
	| "media"
	| "running"
	| "timecode"
	| "scheduler"
	| "channels"
	| "dmx"
	| "visualization"
	| "patch"
	| "setup"
	| "help"
	| "virtual_playbacks"
	| "file_manager"
	| "text_editor";

export type ControlMode = "programmer" | "playbacks";
export type DockMode = "desks" | "builtins";
export type ValueSource = "programmer" | "playback" | "default";
export type StageMode = "select" | "navigate";
/**
 * Which renderer draws the Stage, chosen by the operator rather than by what is available.
 *
 * All three are drawn by the ToskLight renderer, in its own process: `2d` is its plan of the rig
 * from a chosen side, `3d` its outline view, `3d-viz` the full picture. The desk draws none of
 * them, so a screen the renderer cannot reach shows that rather than a second drawing of its own.
 */
export type StageView = "2d" | "3d" | "3d-viz";
/**
 * Which side a 2D Stage is the plan from.
 *
 * A 2D Stage is the renderer's own orthographic view of the rig, so the choice is where the
 * operator is standing to look at it: over it, in the house, upstage, or in either wing.
 */
export type Stage2dSide = "top" | "front" | "back" | "left" | "right";
/**
 * How much of a beam the Stage used to draw, when the desk drew it itself.
 *
 * Nothing reads this any more: the renderer draws every Stage, and what is in a beam is decided by
 * the view and the render quality. It remains only so a saved layout that carries one decodes
 * without complaint.
 */
export type StageRenderQuality =
	| "none"
	| "lines_only"
	| "lines_and_beams"
	| "beams"
	| "improved_beams";
export type DmxDotSize = "small" | "large";
export type ChannelDisplayMode = "intensity" | "all";
export type TextEditorMode = "plain" | "markdown" | "split";
export type VirtualPlaybackPageMode = "follow_main" | "pinned";

export type VisualizationWidgetSource =
	| { kind: "raw_dmx"; universe: number; address: number }
	| { kind: "fixture_attribute"; fixtureId: string; attribute: string };

export type VisualizationWidgetType = "text" | "graph" | "bar" | "number";

export interface VisualizationWidget {
	id: string;
	title: string;
	type: VisualizationWidgetType;
	source: VisualizationWidgetSource;
	operation: "multiply" | "divide";
	factor: number;
	displayScale: "percent" | "dmx";
	minimum: number;
	maximum: number;
	graph: {
		timeWindowSeconds: number;
		yScale: "linear" | "logarithmic";
		filled: boolean;
		lineLowColor: string;
		lineHighColor: string;
		fillLowColor: string;
		fillHighColor: string;
		yAxisName: string;
	};
	bar: { orientation: "horizontal" | "vertical" };
	number: {
		decimalPlaces: number;
		unit: string;
		lowColor: string;
		highColor: string;
	};
}

export interface VisualizationRow {
	id: string;
	widgets: VisualizationWidget[];
}

export interface GridRect {
	x: number;
	y: number;
	width: number;
	height: number;
}

export interface VirtualPlaybackExclusionZone {
	id: string;
	name: string;
	/** Stable show-owned Virtual Playback numbers. */
	playbackNumbers: number[];
}

export interface VirtualPlaybackZoneEdit {
	zoneId: string;
	name: string;
	playbackNumbers: number[];
}

export interface PaneModel extends GridRect {
	id: string;
	kind: BuiltInWindow;
	title: string;
	showGroupShortcuts?: boolean;
	fixtureSheetActiveOnly?: boolean;
	fixtureSheetCompactMode?: FixtureSheetCompactMode;
	fixtureSheetIncludedHeads?: FixtureSheetIncludedHeads;
	fixtureSheetOrder?: FixtureSheetOrder;
	fixtureSheetCueListId?: string;
	fixtureSheetColumns?: FixtureSheetColumn[];
	fixtureSheetShowType?: boolean;
	showCueSidebar?: boolean;
	cueListCompactRows?: boolean;
	cueInformationBlock?: "off" | "current" | "next";
	cueListSource?: "fixed" | "follow-selection";
	fixedCueListNumber?: number;
	stageView?: StageView;
	stage2dSide?: Stage2dSide;
	followPreload?: boolean;
	/** Ultra-only fog character, stored with this portable Visualizer pane. */
	lampFogCloudiness?: number;
	lampFogTurbulence?: number;
	laserFogCloudiness?: number;
	laserFogTurbulence?: number;
	/** Legacy Stage-pane fields retained only for tolerant persisted-layout decoding. */
	showBeamGuides?: boolean;
	stageRenderQuality?: StageRenderQuality;
	/** Legacy Layout-pane field retained only for tolerant persisted-layout decoding. */
	layoutGroupId?: string;
	presetFamily?: AppState["presetFamily"];
	presetPoolColors?: boolean;
	poolColumns?: number;
	schedulerShowList?: boolean;
	schedulerShowCalendar?: boolean;
	mediaServerId?: string;
	mediaLayerId?: string;
	mediaBrowserMode?: "media" | "mask";
	mediaMainSectionId?: string;
	mediaRightPaneVisible?: boolean;
	runningFilter?: "all" | "cue_list" | "dynamic" | "timecode" | "macro";
	virtualPlaybackRows?: number;
	virtualPlaybackColumns?: number;
	virtualPlaybackPageMode?: VirtualPlaybackPageMode;
	virtualPlaybackPinnedPage?: number;
	virtualPlaybackCells?: Array<{
		playbackNumber: number | null;
		action: "go" | "toggle";
	}>;
	virtualPlaybackExclusionZones?: VirtualPlaybackExclusionZone[];
	fileManagerShowHidden?: boolean;
	textFileRoot?: string;
	textFilePath?: string;
	textEditorReadOnly?: boolean;
	textEditorMode?: TextEditorMode;
	channelDisplayMode?: ChannelDisplayMode;
	visualizationRows?: VisualizationRow[];
	textEditorView?: {
		root: string;
		path: string;
		selectionStart: number;
		selectionEnd: number;
		scrollTop: number;
	};
}

export interface DeskModel {
	id: string;
	name: string;
	icon?: string;
	panes: PaneModel[];
}

export interface FixtureRow {
	id: number;
	name: string;
	type: string;
	dimmer: number;
	color: string;
	colorLabel: string;
	pan: number;
	tilt: number;
	positionLabel?: string;
	beam: string;
	focus: string;
	sources: Record<
		"dimmer" | "color" | "position" | "beam" | "focus",
		ValueSource
	>;
}

export interface PresetModel {
	id: number;
	name?: string;
	family?: string;
	color?: string;
	icon?: string;
	fixtures?: number;
}

export interface GroupModel {
	id: number;
	name: string;
	fixtures: number;
}

export interface AppState {
	dockMode: DockMode;
	activeDeskId: string;
	desks: DeskModel[];
	builtIn: BuiltInWindow | null;
	lastBuiltIn: BuiltInWindow;
	fileManagerReturn: {
		dockMode: DockMode;
		activeDeskId: string;
		builtIn: BuiltInWindow | null;
	} | null;
	controlMode: ControlMode;
	paneSettingsId: string | null;
	virtualPlaybackZoneEdit: VirtualPlaybackZoneEdit | null;
	maximizedPaneId: string | null;
	windowPicker: GridRect | null;
	savingDesk: boolean;
	speedGroup: "A" | "B" | "C" | "D" | "E";
	playbackColumns: number;
	playbackRows: number;
	playbackPage: number;
	playbackPageNames: string[];
	presetFamily: PresetFamily;
	presetPoolColors: boolean;
	presetSetArmed: boolean;
	cuelistBuiltInView: "pool" | "cues";
	cuelistBuiltInNumber: number | null;
	patchBuiltInView: "fixtures" | "media";
	cueListSetArmed: boolean;
	cueListSetTarget: number | null;
	playbackSetArmed: boolean;
	setupOpen: boolean;
	specialDialogsOpen: boolean;
	specialDialogFamily:
		| "Color"
		| "Position"
		| "Beam"
		| "Shapers"
		| "Control"
		| "Dynamics";
	systemControlsOpen: boolean;
	preloadStoreOpen: boolean;
	storeArmed: boolean;
	updateArmed: boolean;
	shiftArmed: boolean;
	storeSettingsOpen: boolean;
	patchSetArmed: boolean;
	midiProfile: boolean;
	debugOpen: boolean;
	touchScrollbars: boolean;
	showSectionNames: boolean;
	regularNumberShortcuts: boolean;
	deskSettingsOpen: boolean;
	deskSettingsId: string | null;
	stageMode: StageMode;
	stageView: StageView;
	stageZoom: number;
	stagePanX: number;
	stagePanY: number;
	stageOrbitX: number;
	stageOrbitY: number;
	stageGroupsVisible: boolean;
	stageShowSelection: boolean;
	stageShowFloorGrid: boolean;
	/** Which side a 2D Stage is the plan from. */
	stage2dSide: Stage2dSide;
	stageEnvironmentBrightness: number;
	/** The colour behind the rig in every renderer-drawn Stage, as `#rrggbb`. */
	stageVizBackground: string;
	/** Haze the 3D Viz renderer draws its beams through, `0..=1`. */
	stageVizAtmosphere: number;
	/** How much the 3D Viz renderer is asked to do per frame. */
	stageVizQuality: "draft" | "standard" | "high" | "ultra";
	stageVizExposure: number;
	stageVizLaserBrightness: number;
	stageVizShowLabels: boolean;
	layoutMigrationNotice: boolean;
	dmxDotSize: DmxDotSize;
	fixtureSheetOrder: FixtureSheetOrder;
	fixtureSheetActiveOnly: boolean;
	fixtureSheetCompactMode: FixtureSheetCompactMode;
	fixtureSheetCueListId: string;
	fixtureSheetColumns: FixtureSheetColumn[];
	fixtureSheetShowType: boolean;
	fixtureSheetIncludedHeads: FixtureSheetIncludedHeads;
	fixtureGroupsVisible: boolean;
	presetGroupsVisible: boolean;
	groupsReturnToStage: "builtin" | "desk" | null;
}

export type FixtureSheetOrder = "fixture-id" | "active";
export type FixtureSheetIncludedHeads =
	| "all"
	| "no-sub-heads"
	| "no-master-heads";
export type FixtureSheetCompactMode = "off" | "icon-only" | "text-only";
export type FixtureSheetColumn =
	| "id"
	| "icon"
	| "name"
	| "patch"
	| "intensity"
	| "color"
	| "position"
	| "beam"
	| "shapers"
	| "focus"
	| "control"
	| "media";

export interface WindowSettings {
	dockMode: DockMode;
	builtIn: BuiltInWindow | null;
	lastBuiltIn: BuiltInWindow;
	presetFamily: AppState["presetFamily"];
	presetPoolColors: boolean;
	playbackColumns: number;
	playbackRows: number;
	playbackPage: number;
	stageMode: StageMode;
	stageView: StageView;
	stageZoom: number;
	stagePanX: number;
	stagePanY: number;
	stageOrbitX: number;
	stageOrbitY: number;
	stageGroupsVisible: boolean;
	stageShowSelection: boolean;
	stageShowFloorGrid: boolean;
	/** Which side a 2D Stage is the plan from. */
	stage2dSide: Stage2dSide;
	stageEnvironmentBrightness: number;
	/** The colour behind the rig in every renderer-drawn Stage, as `#rrggbb`. */
	stageVizBackground: string;
	/** Haze the 3D Viz renderer draws its beams through, `0..=1`. */
	stageVizAtmosphere: number;
	/** How much the 3D Viz renderer is asked to do per frame. */
	stageVizQuality: "draft" | "standard" | "high" | "ultra";
	stageVizExposure: number;
	stageVizLaserBrightness: number;
	stageVizShowLabels: boolean;
	/** Legacy fields retained only for tolerant persisted-layout decoding. */
	stageShowBeamGuides?: boolean;
	stageRenderQuality?: StageRenderQuality;
	layoutGroupId?: string;
	dmxDotSize: DmxDotSize;
	fixtureSheetOrder: FixtureSheetOrder;
	fixtureSheetActiveOnly: boolean;
	fixtureSheetCompactMode: FixtureSheetCompactMode;
	fixtureSheetCueListId: string;
	fixtureSheetColumns: Array<FixtureSheetColumn | "dimmer">;
	fixtureSheetShowType: boolean;
	fixtureSheetIncludedHeads: FixtureSheetIncludedHeads;
	/** Legacy layout field retained only for migration to the Patch column. */
	fixtureSheetShowPatch?: boolean;
	/** Legacy layout fields retained only for migration. */
	fixtureSheetShowSubheads?: boolean;
	fixtureSheetShowMasterHeads?: boolean;
	fixtureGroupsVisible: boolean;
	presetGroupsVisible: boolean;
}
