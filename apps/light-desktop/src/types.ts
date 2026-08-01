import type { PresetFamily } from "./presetFamilies";

export const GRID_COLUMNS = 24;
export const GRID_ROWS = 18;

export type BuiltInWindow =
	| "stage"
	| "groups"
	| "fixtures"
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
	| "scheduler"
	| "channels"
	| "dmx"
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
export type StageView = "2d" | "3d";
export type StageRenderQuality =
	| "lines_only"
	| "lines_and_beams"
	| "beams"
	| "improved_beams";
export type DmxDotSize = "small" | "large";
export type ChannelDisplayMode = "intensity" | "all";
export type TextEditorMode = "plain" | "markdown" | "split";
export type VirtualPlaybackPageMode = "follow_main" | "pinned";

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
	showCueSidebar?: boolean;
	cueListSource?: "fixed" | "follow-selection";
	fixedCueListNumber?: number;
	stageView?: StageView;
	followPreload?: boolean;
	showBeamGuides?: boolean;
	stageRenderQuality?: StageRenderQuality;
	layoutGroupId?: string;
	presetFamily?: AppState["presetFamily"];
	presetPoolColors?: boolean;
	schedulerShowList?: boolean;
	schedulerShowCalendar?: boolean;
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
	stageShowBeamGuides: boolean;
	stageRenderQuality: StageRenderQuality;
	stageEnvironmentBrightness: number;
	layoutGroupId: string;
	dmxDotSize: DmxDotSize;
	fixtureSheetOrder: FixtureSheetOrder;
	fixtureSheetActiveOnly: boolean;
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
export type FixtureSheetColumn =
	| "id"
	| "icon"
	| "name"
	| "patch"
	| "dimmer"
	| "color"
	| "position"
	| "beam"
	| "focus";

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
	stageShowBeamGuides: boolean;
	stageRenderQuality: StageRenderQuality;
	stageEnvironmentBrightness: number;
	layoutGroupId: string;
	dmxDotSize: DmxDotSize;
	fixtureSheetOrder: FixtureSheetOrder;
	fixtureSheetActiveOnly: boolean;
	fixtureSheetCueListId: string;
	fixtureSheetColumns: FixtureSheetColumn[];
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
