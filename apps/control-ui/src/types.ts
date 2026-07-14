export const GRID_COLUMNS = 24;
export const GRID_ROWS = 18;

export type BuiltInWindow =
  | "stage"
  | "groups"
  | "fixtures"
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
  | "channels"
  | "dmx"
  | "patch"
  | "setup"
  | "help"
  | "development";

export type ControlMode = "programmer" | "playbacks";
export type DockMode = "desks" | "builtins";
export type ValueSource = "programmer" | "playback" | "default";
export type StageMode = "select" | "setup" | "navigate";
export type StageView = "2d" | "3d";
export type DmxDotSize = "small" | "large";
export type DevelopmentView = "forms" | "faders" | "buttons";

export interface GridRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PaneModel extends GridRect {
  id: string;
  kind: BuiltInWindow;
  title: string;
  showGroupShortcuts?: boolean;
  stageView?: StageView;
  followPreload?: boolean;
  presetFamily?: AppState["presetFamily"];
  presetPoolColors?: boolean;
  developmentView?: DevelopmentView;
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
  sources: Record<"dimmer" | "color" | "position" | "beam" | "focus", ValueSource>;
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
  controlMode: ControlMode;
  paneSettingsId: string | null;
  maximizedPaneId: string | null;
  windowPicker: GridRect | null;
  savingDesk: boolean;
  preload: "idle" | "blind" | "output";
  preloadActive: boolean;
  speedGroup: "A" | "B" | "C" | "D" | "E";
  playbackColumns: number;
  playbackRows: number;
  playbackPage: number;
  playbackPageNames: string[];
  presetFamily: "All" | "Intensity" | "Color" | "Position" | "Beam";
  presetPoolColors: boolean;
  presetSetArmed: boolean;
  cuelistBuiltInView: "pool" | "cues";
  cuelistBuiltInNumber: number | null;
  cueListSetArmed: boolean;
  cueListSetTarget: number | null;
  setupOpen: boolean;
  specialDialogsOpen: boolean;
  specialDialogFamily: "Color" | "Position" | "Beam" | "Shapers" | "Control" | "Dynamics";
  systemControlsOpen: boolean;
  preloadStoreOpen: boolean;
  storeArmed: boolean;
  storeSettingsOpen: boolean;
  patchSetArmed: boolean;
  midiProfile: boolean;
  debugOpen: boolean;
  touchScrollbars: boolean;
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
  stageEnvironmentBrightness: number;
  dmxDotSize: DmxDotSize;
  fixtureGroupsVisible: boolean;
  presetGroupsVisible: boolean;
  groupsReturnToStage: "builtin" | "desk" | null;
  blackout: boolean;
}

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
  stageEnvironmentBrightness: number;
  dmxDotSize: DmxDotSize;
  fixtureGroupsVisible: boolean;
  presetGroupsVisible: boolean;
}
