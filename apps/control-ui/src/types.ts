export const GRID_COLUMNS = 24;
export const GRID_ROWS = 18;

export type BuiltInWindow =
  | "stage"
  | "groups"
  | "fixtures"
  | "presets"
  | "playback"
  | "dynamics"
  | "channels"
  | "dmx"
  | "setup";

export type ControlMode = "programmer" | "playbacks";
export type DockMode = "desks" | "builtins";
export type ValueSource = "programmer" | "playback" | "default";

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
}

export interface DeskModel {
  id: string;
  name: string;
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
  controlMode: ControlMode;
  paneSettingsId: string | null;
  maximizedPaneId: string | null;
  windowPicker: GridRect | null;
  savingDesk: boolean;
  preload: "idle" | "blind" | "output";
  preloadActive: boolean;
  programmerFade: number;
  sequenceMaster: number;
  speedGroup: "A" | "B" | "C" | "D";
  setupOpen: boolean;
  specialDialogsOpen: boolean;
  specialDialogFamily: "Color" | "Position" | "Beam" | "Control" | "Dynamics";
  systemControlsOpen: boolean;
  preloadStoreOpen: boolean;
  midiProfile: boolean;
}
