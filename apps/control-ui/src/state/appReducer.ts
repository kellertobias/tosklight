import { GRID_COLUMNS, GRID_ROWS, type AppState, type BuiltInWindow, type GridRect, type WindowSettings } from "../types";
import { initialDesks } from "../data/mockData";

export type Action =
  | { type: "SET_DOCK_MODE"; mode: AppState["dockMode"] }
  | { type: "OPEN_DESK"; id: string }
  | { type: "OPEN_BUILTIN"; kind: BuiltInWindow }
  | { type: "TOGGLE_CONTROL_MODE" }
  | { type: "SET_PANE_SETTINGS"; id: string | null }
  | { type: "SET_PANE_RECT"; id: string; rect: Partial<GridRect> }
  | { type: "SET_PANE_GROUP_SHORTCUTS"; id: string; value: boolean }
  | { type: "SET_PANE_STAGE_OPTION"; id: string; option: "stageView" | "followPreload"; value: AppState["stageView"] | boolean }
  | { type: "SET_PANE_PRESET_FAMILY"; id: string; family: AppState["presetFamily"] }
  | { type: "SET_PANE_PRESET_COLORS"; id: string; value: boolean }
  | { type: "SET_STAGE_MODE"; value: AppState["stageMode"] }
  | { type: "SET_STAGE_VIEW"; value: AppState["stageView"] }
  | { type: "SET_STAGE_NAVIGATION"; zoom?: number; panX?: number; panY?: number; orbitX?: number; orbitY?: number }
  | { type: "SET_STAGE_OPTIONS"; groupsVisible?: boolean; showSelection?: boolean; environmentBrightness?: number }
  | { type: "SET_DMX_DOT_SIZE"; value: AppState["dmxDotSize"] }
  | { type: "SET_BUILTIN_GROUPS_VISIBLE"; window: "fixtures" | "presets"; value: boolean }
  | { type: "OPEN_GROUPS_FROM_STAGE"; origin?: "builtin" | "desk" }
  | { type: "RETURN_TO_STAGE" }
  | { type: "SET_BLACKOUT"; value: boolean }
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
  | { type: "SET_PRESET_POOL_COLORS"; value: boolean }
  | { type: "SET_PRESET_SET_ARMED"; value: boolean }
  | { type: "SET_MODAL"; modal: "setupOpen" | "specialDialogsOpen" | "systemControlsOpen" | "preloadStoreOpen" | "debugOpen" | "deskSettingsOpen" | "storeSettingsOpen"; value: boolean }
  | { type: "OPEN_SPECIAL_DIALOG"; family: AppState["specialDialogFamily"] }
  | { type: "TOGGLE_MIDI_PROFILE" }
  | { type: "TOGGLE_TOUCH_SCROLLBARS" }
  | { type: "SET_STORE_ARMED"; value: boolean }
  | { type: "SET_PATCH_ARMED"; value: boolean }
  | { type: "HYDRATE_LAYOUT"; desks: AppState["desks"]; activeDeskId: string; windowSettings?: Partial<WindowSettings> };

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
  presetPoolColors: true,
  presetSetArmed: false,
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
  stageMode: "select",
  stageView: "2d",
  stageZoom: 1,
  stagePanX: 0,
  stagePanY: 0,
  stageOrbitX: 0,
  stageOrbitY: 0,
  stageGroupsVisible: true,
  stageShowSelection: true,
  stageEnvironmentBrightness: 1,
  dmxDotSize: typeof globalThis.matchMedia === "function" && globalThis.matchMedia("(pointer: coarse)").matches ? "large" : "small",
  fixtureGroupsVisible: true,
  presetGroupsVisible: true,
  groupsReturnToStage: null,
  blackout: false,
};

const clamp = (value: number, minimum: number, maximum: number) => Math.max(minimum, Math.min(maximum, value));
const overlaps = (a: GridRect, b: GridRect) => a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y;

export function appReducer(state: AppState, action: Action): AppState {
  switch (action.type) {
    case "SET_DOCK_MODE": return action.mode === "desks"
      ? { ...state, dockMode: "desks", builtIn: null }
      : { ...state, dockMode: "builtins", builtIn: state.lastBuiltIn };
    case "OPEN_DESK": return { ...state, activeDeskId: action.id, builtIn: null, dockMode: "desks", savingDesk: false };
    case "OPEN_BUILTIN": return { ...state, builtIn: action.kind, lastBuiltIn: action.kind, dockMode: "builtins" };
    case "OPEN_GROUPS_FROM_STAGE": return { ...state, builtIn: "groups", lastBuiltIn: "groups", dockMode: "builtins", groupsReturnToStage: action.origin ?? "builtin" };
    case "RETURN_TO_STAGE": return state.groupsReturnToStage === "desk" ? { ...state, builtIn: null, dockMode: "desks", groupsReturnToStage: null } : { ...state, builtIn: "stage", lastBuiltIn: "stage", dockMode: "builtins", groupsReturnToStage: null };
    case "SET_BLACKOUT": return { ...state, blackout: action.value };
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
      ...action.windowSettings,
      desks: action.desks.map((desk) => ({ ...desk, panes: desk.panes.map((pane) => {
        if (pane.kind !== "presets") return pane;
        const legacyDefault = pane.id === "presets" && pane.title === "Color & Position Presets";
        return {
          ...pane,
          title: legacyDefault ? "All Presets" : pane.title,
          presetFamily: legacyDefault ? "All" : pane.presetFamily ?? state.presetFamily,
        };
      }) })),
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
      desks: state.desks.map((desk) => {
        if (desk.id !== state.activeDeskId) return desk;
        const pane = desk.panes.find((item) => item.id === action.id);
        if (!pane) return desk;
        const x = clamp(action.rect.x ?? pane.x, 1, GRID_COLUMNS);
        const y = clamp(action.rect.y ?? pane.y, 1, GRID_ROWS);
        const candidate = { ...pane, x, y, width: clamp(action.rect.width ?? pane.width, 1, GRID_COLUMNS - x + 1), height: clamp(action.rect.height ?? pane.height, 1, GRID_ROWS - y + 1) };
        if (desk.panes.some((item) => item.id !== pane.id && overlaps(candidate, item))) return desk;
        return { ...desk, panes: desk.panes.map((item) => item.id === pane.id ? candidate : item) };
      }),
    };
    case "SET_PANE_GROUP_SHORTCUTS": return { ...state, desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : { ...desk, panes: desk.panes.map((pane) => pane.id === action.id ? { ...pane, showGroupShortcuts: action.value } : pane) }) };
    case "SET_PANE_STAGE_OPTION": return { ...state, stageView: action.option === "stageView" ? action.value as AppState["stageView"] : state.stageView, desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : { ...desk, panes: desk.panes.map((pane) => pane.id === action.id ? { ...pane, [action.option]: action.value } : pane) }) };
    case "SET_PANE_PRESET_FAMILY": return { ...state, desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : { ...desk, panes: desk.panes.map((pane) => pane.id === action.id ? { ...pane, presetFamily: action.family } : pane) }) };
    case "SET_PANE_PRESET_COLORS": return { ...state, desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : { ...desk, panes: desk.panes.map((pane) => pane.id === action.id ? { ...pane, presetPoolColors: action.value } : pane) }) };
    case "SET_STAGE_MODE": return { ...state, stageMode: action.value };
    case "SET_STAGE_VIEW": return { ...state, stageView: action.value };
    case "SET_STAGE_NAVIGATION": return { ...state, stageZoom: action.zoom ?? state.stageZoom, stagePanX: action.panX ?? state.stagePanX, stagePanY: action.panY ?? state.stagePanY, stageOrbitX: action.orbitX ?? state.stageOrbitX, stageOrbitY: action.orbitY ?? state.stageOrbitY };
    case "SET_STAGE_OPTIONS": return { ...state, stageGroupsVisible: action.groupsVisible ?? state.stageGroupsVisible, stageShowSelection: action.showSelection ?? state.stageShowSelection, stageEnvironmentBrightness: clamp(action.environmentBrightness ?? state.stageEnvironmentBrightness, 0, 2) };
    case "SET_DMX_DOT_SIZE": return { ...state, dmxDotSize: action.value };
    case "SET_BUILTIN_GROUPS_VISIBLE": return action.window === "fixtures" ? { ...state, fixtureGroupsVisible: action.value } : { ...state, presetGroupsVisible: action.value };
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
      const pane = { id: `${action.kind}-${Date.now()}`, kind: action.kind, title: action.kind === "help" ? "Help" : action.kind[0].toUpperCase() + action.kind.slice(1), ...state.windowPicker };
      const activeDesk = state.desks.find((desk) => desk.id === state.activeDeskId);
      if (activeDesk?.panes.some((item) => overlaps(pane, item))) return { ...state, windowPicker: null };
      return { ...state, windowPicker: null, desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : { ...desk, panes: [...desk.panes, pane] }) };
    }
    case "ADVANCE_PRELOAD": return { ...state, preload: state.preload === "blind" ? "output" : "blind", preloadActive: state.preload === "blind" ? true : state.preloadActive };
    case "RELEASE_PRELOAD": return { ...state, preload: "idle", preloadActive: false };
    case "SET_SPEED_GROUP": return { ...state, speedGroup: action.value };
    case "SET_PLAYBACK_LAYOUT": return { ...state, playbackColumns: clamp(action.columns, 1, 32), playbackRows: clamp(action.rows, 1, 3) };
    case "SET_PLAYBACK_PAGE": return { ...state, playbackPage: clamp(action.page, 0, state.playbackPageNames.length - 1) };
    case "SET_PRESET_FAMILY": return { ...state, presetFamily: action.family };
    case "SET_PRESET_POOL_COLORS": return { ...state, presetPoolColors: action.value };
    case "SET_PRESET_SET_ARMED": return { ...state, presetSetArmed: action.value };
    case "SET_MODAL": return { ...state, [action.modal]: action.value };
    case "OPEN_SPECIAL_DIALOG": return { ...state, specialDialogFamily: action.family, specialDialogsOpen: true };
    case "TOGGLE_MIDI_PROFILE": return { ...state, midiProfile: !state.midiProfile };
    case "TOGGLE_TOUCH_SCROLLBARS": return { ...state, touchScrollbars: !state.touchScrollbars };
    case "SET_STORE_ARMED": return { ...state, storeArmed: action.value };
    case "SET_PATCH_ARMED": return { ...state, patchSetArmed: action.value };
    default: return state;
  }
}
