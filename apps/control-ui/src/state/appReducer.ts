import { GRID_COLUMNS, GRID_ROWS, type AppState, type BuiltInWindow, type GridRect } from "../types";
import { initialDesks } from "../data/mockData";

export type Action =
  | { type: "SET_DOCK_MODE"; mode: AppState["dockMode"] }
  | { type: "OPEN_DESK"; id: string }
  | { type: "OPEN_BUILTIN"; kind: BuiltInWindow }
  | { type: "TOGGLE_CONTROL_MODE" }
  | { type: "SET_PANE_SETTINGS"; id: string | null }
  | { type: "SET_PANE_RECT"; id: string; rect: Partial<GridRect> }
  | { type: "SET_PANE_GROUP_SHORTCUTS"; id: string; value: boolean }
  | { type: "TOGGLE_MAXIMIZE"; id: string }
  | { type: "REMOVE_PANE"; id: string }
  | { type: "OPEN_DESK_SETTINGS"; id: string | null }
  | { type: "UPDATE_DESK"; id: string; name?: string; icon?: string }
  | { type: "DELETE_DESK"; id: string }
  | { type: "NEW_DESK" }
  | { type: "START_SAVE_DESK" }
  | { type: "SAVE_DESK_TO"; id: string }
  | { type: "OPEN_WINDOW_PICKER"; rect: GridRect | null }
  | { type: "ADD_WINDOW"; kind: BuiltInWindow }
  | { type: "ADVANCE_PRELOAD" }
  | { type: "RELEASE_PRELOAD" }
  | { type: "SET_SPEED_GROUP"; value: AppState["speedGroup"] }
  | { type: "SET_PLAYBACK_LAYOUT"; columns: number; rows: number }
  | { type: "SET_PLAYBACK_PAGE"; page: number }
  | { type: "SET_PRESET_FAMILY"; family: AppState["presetFamily"] }
  | { type: "SET_MODAL"; modal: "setupOpen" | "specialDialogsOpen" | "systemControlsOpen" | "preloadStoreOpen" | "debugOpen" | "deskSettingsOpen" | "storeSettingsOpen"; value: boolean }
  | { type: "OPEN_SPECIAL_DIALOG"; family: AppState["specialDialogFamily"] }
  | { type: "TOGGLE_MIDI_PROFILE" }
  | { type: "TOGGLE_TOUCH_SCROLLBARS" }
  | { type: "SET_STORE_ARMED"; value: boolean }
  | { type: "SET_PATCH_ARMED"; value: boolean }
  | { type: "HYDRATE_LAYOUT"; desks: AppState["desks"]; activeDeskId: string };

export const initialState: AppState = {
  dockMode: "desks",
  activeDeskId: "programming",
  desks: initialDesks,
  builtIn: null,
  lastBuiltIn: "stage",
  controlMode: "programmer",
  paneSettingsId: null,
  maximizedPaneId: null,
  windowPicker: null,
  savingDesk: false,
  preload: "idle",
  preloadActive: false,
  speedGroup: "A",
  playbackColumns: 8,
  playbackRows: 1,
  playbackPage: 0,
  playbackPageNames: Array.from({ length: 127 }, (_, index) => index === 0 ? "Main" : `Page ${index + 1}`),
  presetFamily: "All",
  setupOpen: false,
  specialDialogsOpen: false,
  specialDialogFamily: "Position",
  systemControlsOpen: false,
  preloadStoreOpen: false,
  storeArmed: false,
  storeSettingsOpen: false,
  patchSetArmed: false,
  midiProfile: false,
  debugOpen: false,
  touchScrollbars: false,
  deskSettingsOpen: false,
  deskSettingsId: null,
};

const clamp = (value: number, minimum: number, maximum: number) => Math.max(minimum, Math.min(maximum, value));

export function appReducer(state: AppState, action: Action): AppState {
  switch (action.type) {
    case "SET_DOCK_MODE": return action.mode === "desks"
      ? { ...state, dockMode: "desks", builtIn: null }
      : { ...state, dockMode: "builtins", builtIn: state.lastBuiltIn };
    case "OPEN_DESK": return { ...state, activeDeskId: action.id, builtIn: null, dockMode: "desks", savingDesk: false };
    case "OPEN_BUILTIN": return { ...state, builtIn: action.kind, lastBuiltIn: action.kind, dockMode: "builtins" };
    case "TOGGLE_CONTROL_MODE": return { ...state, controlMode: state.controlMode === "programmer" ? "playbacks" : "programmer" };
    case "SET_PANE_SETTINGS": return { ...state, paneSettingsId: action.id };
    case "TOGGLE_MAXIMIZE": return { ...state, maximizedPaneId: state.maximizedPaneId === action.id ? null : action.id };
    case "OPEN_WINDOW_PICKER": return { ...state, windowPicker: action.rect };
    case "START_SAVE_DESK": return { ...state, savingDesk: true };
    case "SAVE_DESK_TO": {
      const source = state.desks.find((desk) => desk.id === state.activeDeskId);
      return {
        ...state,
        savingDesk: false,
        activeDeskId: action.id,
        desks: state.desks.map((desk) => desk.id !== action.id || !source ? desk : {
          ...desk,
          panes: source.panes.map((pane, index) => ({ ...pane, id: `${desk.id}-${pane.kind}-${index + 1}` })),
        }),
      };
    }
    case "HYDRATE_LAYOUT": return {
      ...state,
      desks: action.desks,
      activeDeskId: action.desks.some((desk) => desk.id === action.activeDeskId) ? action.activeDeskId : action.desks[0]?.id ?? state.activeDeskId,
      savingDesk: false,
    };
    case "NEW_DESK": {
      const id = `desk-${state.desks.length + 1}`;
      const source = state.desks.find((desk) => desk.id === state.activeDeskId);
      const panes = state.savingDesk && source
        ? source.panes.map((pane, index) => ({ ...pane, id: `${id}-${pane.kind}-${index + 1}` }))
        : [];
      return { ...state, desks: [...state.desks, { id, name: `Desk ${state.desks.length + 1}`, panes }], activeDeskId: id, builtIn: null, savingDesk: false };
    }
    case "SET_PANE_RECT": return {
      ...state,
      desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : {
        ...desk,
        panes: desk.panes.map((pane) => pane.id !== action.id ? pane : {
          ...pane,
          x: clamp(action.rect.x ?? pane.x, 1, GRID_COLUMNS),
          y: clamp(action.rect.y ?? pane.y, 1, GRID_ROWS),
          width: clamp(action.rect.width ?? pane.width, 1, GRID_COLUMNS - clamp(action.rect.x ?? pane.x, 1, GRID_COLUMNS) + 1),
          height: clamp(action.rect.height ?? pane.height, 1, GRID_ROWS - clamp(action.rect.y ?? pane.y, 1, GRID_ROWS) + 1),
        }),
      }),
    };
    case "SET_PANE_GROUP_SHORTCUTS": return { ...state, desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : { ...desk, panes: desk.panes.map((pane) => pane.id === action.id ? { ...pane, showGroupShortcuts: action.value } : pane) }) };
    case "REMOVE_PANE": return { ...state, paneSettingsId: null, desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : { ...desk, panes: desk.panes.filter((pane) => pane.id !== action.id) }) };
    case "OPEN_DESK_SETTINGS": return { ...state, deskSettingsOpen: Boolean(action.id), deskSettingsId: action.id };
    case "UPDATE_DESK": return { ...state, desks: state.desks.map((desk) => desk.id === action.id ? { ...desk, name: action.name ?? desk.name, icon: action.icon ?? desk.icon } : desk) };
    case "DELETE_DESK": {
      if (state.desks.length <= 1) return state;
      const desks = state.desks.filter((desk) => desk.id !== action.id);
      return { ...state, desks, activeDeskId: state.activeDeskId === action.id ? desks[0].id : state.activeDeskId, deskSettingsOpen: false, deskSettingsId: null };
    }
    case "ADD_WINDOW": {
      if (!state.windowPicker) return state;
      const pane = { id: `${action.kind}-${Date.now()}`, kind: action.kind, title: action.kind[0].toUpperCase() + action.kind.slice(1), ...state.windowPicker };
      return { ...state, windowPicker: null, desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : { ...desk, panes: [...desk.panes, pane] }) };
    }
    case "ADVANCE_PRELOAD": return { ...state, preload: state.preload === "blind" ? "output" : "blind", preloadActive: state.preload === "blind" ? true : state.preloadActive };
    case "RELEASE_PRELOAD": return { ...state, preload: "idle", preloadActive: false };
    case "SET_SPEED_GROUP": return { ...state, speedGroup: action.value };
    case "SET_PLAYBACK_LAYOUT": return { ...state, playbackColumns: clamp(action.columns, 1, 32), playbackRows: clamp(action.rows, 1, 3) };
    case "SET_PLAYBACK_PAGE": return { ...state, playbackPage: clamp(action.page, 0, state.playbackPageNames.length - 1) };
    case "SET_PRESET_FAMILY": return { ...state, presetFamily: action.family };
    case "SET_MODAL": return { ...state, [action.modal]: action.value };
    case "OPEN_SPECIAL_DIALOG": return { ...state, specialDialogFamily: action.family, specialDialogsOpen: true };
    case "TOGGLE_MIDI_PROFILE": return { ...state, midiProfile: !state.midiProfile };
    case "TOGGLE_TOUCH_SCROLLBARS": return { ...state, touchScrollbars: !state.touchScrollbars };
    case "SET_STORE_ARMED": return { ...state, storeArmed: action.value };
    case "SET_PATCH_ARMED": return { ...state, patchSetArmed: action.value };
    default: return state;
  }
}
