import { GRID_COLUMNS, GRID_ROWS, type AppState, type BuiltInWindow, type DevelopmentView, type GridRect, type TextEditorMode, type VirtualPlaybackExclusionZone, type WindowSettings } from "../types";
import { initialDesks } from "../data/mockData";

export type Action =
  | { type: "SET_DOCK_MODE"; mode: AppState["dockMode"] }
  | { type: "OPEN_DESK"; id: string }
  | { type: "OPEN_BUILTIN"; kind: BuiltInWindow }
  | { type: "CLOSE_FILE_MANAGER" }
  | { type: "TOGGLE_CONTROL_MODE" }
  | { type: "SET_PANE_SETTINGS"; id: string | null }
  | { type: "SET_PANE_RECT"; id: string; rect: Partial<GridRect> }
  | { type: "SET_PANE_GROUP_SHORTCUTS"; id: string; value: boolean }
  | { type: "SET_PANE_STAGE_OPTION"; id: string; option: "stageView" | "followPreload"; value: AppState["stageView"] | boolean }
  | { type: "SET_PANE_PRESET_FAMILY"; id: string; family: AppState["presetFamily"] }
  | { type: "SET_PANE_PRESET_COLORS"; id: string; value: boolean }
  | { type: "SET_PANE_DEVELOPMENT_VIEW"; id: string; value: DevelopmentView }
  | { type: "SET_VIRTUAL_PLAYBACK_GRID"; id: string; rows: number; columns: number }
  | { type: "SET_VIRTUAL_PLAYBACK_CELL"; id: string; index: number; playbackNumber?: number | null; action?: "go" | "toggle" }
  | { type: "SET_VIRTUAL_PLAYBACK_EXCLUSION_ZONES"; id: string; zones: VirtualPlaybackExclusionZone[] }
  | { type: "SET_FILE_MANAGER_SHOW_HIDDEN"; id: string; value: boolean }
  | { type: "SET_TEXT_EDITOR_FILE"; id: string; root: string; path: string }
  | { type: "SET_TEXT_EDITOR_SETTINGS"; id: string; readOnly?: boolean; mode?: TextEditorMode }
  | { type: "SET_TEXT_EDITOR_VIEW"; id: string; root: string; path: string; selectionStart: number; selectionEnd: number; scrollTop: number }
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
  | { type: "OPEN_BUILTIN_CUELIST"; number: number }
  | { type: "SET_BUILTIN_CUELIST_VIEW"; value: "pool" | "cues" }
  | { type: "SET_CUELIST_SET_ARMED"; value: boolean }
  | { type: "SET_CUELIST_SET_TARGET"; value: number | null }
  | { type: "SET_PLAYBACK_SET_ARMED"; value: boolean }
  | { type: "SET_MODAL"; modal: "setupOpen" | "specialDialogsOpen" | "systemControlsOpen" | "preloadStoreOpen" | "debugOpen" | "deskSettingsOpen" | "storeSettingsOpen"; value: boolean }
  | { type: "OPEN_SPECIAL_DIALOG"; family: AppState["specialDialogFamily"] }
  | { type: "TOGGLE_MIDI_PROFILE" }
  | { type: "SET_MIDI_PROFILE"; value: boolean }
  | { type: "TOGGLE_TOUCH_SCROLLBARS" }
  | { type: "TOGGLE_SECTION_NAMES" }
  | { type: "SET_REGULAR_NUMBER_SHORTCUTS"; value: boolean }
  | { type: "SET_STORE_ARMED"; value: boolean }
  | { type: "SET_UPDATE_ARMED"; value: boolean }
  | { type: "SET_SHIFT_ARMED"; value: boolean }
  | { type: "SET_PATCH_ARMED"; value: boolean }
  | { type: "HYDRATE_LAYOUT"; desks: AppState["desks"]; activeDeskId: string; windowSettings?: Partial<WindowSettings> };

export const initialState: AppState = {
  dockMode: "desks",
  activeDeskId: "programming",
  desks: initialDesks,
  builtIn: null,
  lastBuiltIn: "stage",
  fileManagerReturn: null,
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
  cuelistBuiltInView: "pool",
  cuelistBuiltInNumber: null,
  cueListSetArmed: false,
  cueListSetTarget: null,
  playbackSetArmed: false,
  setupOpen: false,
  specialDialogsOpen: false,
  specialDialogFamily: "Position",
  systemControlsOpen: false,
  preloadStoreOpen: false,
  storeArmed: false,
  updateArmed: false,
  shiftArmed: false,
  storeSettingsOpen: false,
  patchSetArmed: false,
  midiProfile: false,
  debugOpen: false,
  touchScrollbars: false,
  showSectionNames: false,
  regularNumberShortcuts: true,
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
const cueListWindowKind = (kind: BuiltInWindow): BuiltInWindow => kind === "playback" || kind === "qlists" ? "cuelists" : kind === "playback_pool" || kind === "qlist_pool" ? "cuelist_pool" : kind === "cue_list" || kind === "qs" ? "cues" : kind;
const cueListWindowTitle = (title: string, kind: BuiltInWindow) => {
  if (kind === "cuelists") return "Cuelists";
  if (kind === "cuelist_pool") return "Cuelist Pool";
  if (kind !== "cues") return title;
  if (/^(cue list|sequence)$/i.test(title)) return "Cues · Cuelist";
  return title.replace(/^Qs\s*·\s*/i, "Cues · ").replace(/QList/g, "Cuelist");
};

export function appReducer(state: AppState, action: Action): AppState {
  switch (action.type) {
    case "SET_DOCK_MODE": return action.mode === "desks"
      ? { ...state, dockMode: "desks", builtIn: null, fileManagerReturn: null }
      : { ...state, dockMode: "builtins", builtIn: state.lastBuiltIn, fileManagerReturn: null };
    case "OPEN_DESK": return { ...state, activeDeskId: action.id, builtIn: null, dockMode: "desks", savingDesk: false, fileManagerReturn: null };
    case "OPEN_BUILTIN": {
      const kind = cueListWindowKind(action.kind);
      if (kind === "cuelists" && state.builtIn === "cuelists" && state.cuelistBuiltInView === "cues") return { ...state, cuelistBuiltInView: "pool", dockMode: "builtins" };
      if (kind === "file_manager") return {
        ...state,
        builtIn: kind,
        dockMode: "builtins",
        fileManagerReturn: state.builtIn === "file_manager" && state.fileManagerReturn
          ? state.fileManagerReturn
          : { dockMode: state.dockMode, activeDeskId: state.activeDeskId, builtIn: state.builtIn },
      };
      return { ...state, builtIn: kind, lastBuiltIn: kind, dockMode: "builtins", fileManagerReturn: null };
    }
    case "CLOSE_FILE_MANAGER": {
      if (state.builtIn !== "file_manager") return state;
      const destination = state.fileManagerReturn;
      return destination
        ? { ...state, ...destination, fileManagerReturn: null }
        : { ...state, builtIn: null, dockMode: "desks", fileManagerReturn: null };
    }
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
      builtIn: action.windowSettings?.builtIn == null ? action.windowSettings?.builtIn ?? state.builtIn : cueListWindowKind(action.windowSettings.builtIn),
      lastBuiltIn: cueListWindowKind(action.windowSettings?.lastBuiltIn ?? state.lastBuiltIn),
      desks: action.desks.map((desk) => ({ ...desk, panes: desk.panes.map((pane) => {
        const kind = cueListWindowKind(pane.kind);
        const migrated = { ...pane, kind, title: cueListWindowTitle(pane.title, kind) };
        if (pane.kind !== "presets") return migrated;
        const legacyDefault = pane.id === "presets" && pane.title === "Color & Position Presets";
        return {
          ...migrated,
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
      return { ...state, desks: [...state.desks, { id, name: `Desktop ${state.desks.length + 1}`, panes }], activeDeskId: id, builtIn: null, savingDesk: false };
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
    case "SET_PANE_DEVELOPMENT_VIEW": return { ...state, desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : { ...desk, panes: desk.panes.map((pane) => pane.id === action.id ? { ...pane, developmentView: action.value } : pane) }) };
    case "SET_VIRTUAL_PLAYBACK_GRID": return { ...state, desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : { ...desk, panes: desk.panes.map((pane) => pane.id === action.id ? { ...pane, virtualPlaybackRows: clamp(action.rows, 1, 12), virtualPlaybackColumns: clamp(action.columns, 1, 12) } : pane) }) };
    case "SET_VIRTUAL_PLAYBACK_CELL": return { ...state, desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : { ...desk, panes: desk.panes.map((pane) => { if (pane.id !== action.id) return pane; const cells = [...(pane.virtualPlaybackCells ?? [])]; const current = cells[action.index] ?? { playbackNumber: null, action: "go" as const }; cells[action.index] = { playbackNumber: action.playbackNumber === undefined ? current.playbackNumber : action.playbackNumber, action: action.action ?? current.action }; return { ...pane, virtualPlaybackCells: cells }; }) }) };
    case "SET_VIRTUAL_PLAYBACK_EXCLUSION_ZONES": return { ...state, desks: state.desks.map((desk) => desk.id !== state.activeDeskId ? desk : { ...desk, panes: desk.panes.map((pane) => pane.id === action.id ? { ...pane, virtualPlaybackExclusionZones: action.zones.map((zone) => ({ ...zone, slots: [...zone.slots] })) } : pane) }) };
    case "SET_FILE_MANAGER_SHOW_HIDDEN": return { ...state, desks: state.desks.map((desk) => ({ ...desk, panes: desk.panes.map((pane) => pane.id === action.id ? { ...pane, fileManagerShowHidden: action.value } : pane) })) };
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
      const kind = cueListWindowKind(action.kind);
      const pane = { id: `${kind}-${Date.now()}`, kind, title: kind === "help" ? "Help" : kind === "development" ? "Development" : kind === "virtual_playbacks" ? "Virtual Playbacks" : kind === "file_manager" ? "File Manager" : kind === "text_editor" ? "Text Editor" : cueListWindowTitle(kind[0].toUpperCase() + kind.slice(1), kind), ...(kind === "virtual_playbacks" ? { virtualPlaybackRows: 2, virtualPlaybackColumns: 2, virtualPlaybackCells: [], virtualPlaybackExclusionZones: [] } : {}), ...(kind === "file_manager" ? { fileManagerShowHidden: false } : {}), ...(kind === "text_editor" ? { textEditorReadOnly: false, textEditorMode: "plain" as const } : {}), ...state.windowPicker };
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
    case "OPEN_BUILTIN_CUELIST": return { ...state, cuelistBuiltInView: "cues", cuelistBuiltInNumber: action.number };
    case "SET_BUILTIN_CUELIST_VIEW": return { ...state, cuelistBuiltInView: action.value };
    case "SET_CUELIST_SET_ARMED": return { ...state, cueListSetArmed: action.value, cueListSetTarget: action.value ? state.cueListSetTarget : null };
    case "SET_CUELIST_SET_TARGET": return { ...state, cueListSetArmed: action.value != null, cueListSetTarget: action.value };
    case "SET_PLAYBACK_SET_ARMED": return { ...state, playbackSetArmed: action.value };
    case "SET_MODAL": return { ...state, [action.modal]: action.value };
    case "OPEN_SPECIAL_DIALOG": return { ...state, specialDialogFamily: action.family, specialDialogsOpen: true };
    case "TOGGLE_MIDI_PROFILE": return { ...state, midiProfile: !state.midiProfile };
    case "SET_MIDI_PROFILE": return { ...state, midiProfile: action.value };
    case "TOGGLE_TOUCH_SCROLLBARS": return { ...state, touchScrollbars: !state.touchScrollbars };
    case "TOGGLE_SECTION_NAMES": return { ...state, showSectionNames: !state.showSectionNames };
    case "SET_REGULAR_NUMBER_SHORTCUTS": return { ...state, regularNumberShortcuts: action.value };
    case "SET_TEXT_EDITOR_FILE": return { ...state, desks: state.desks.map((desk) => ({ ...desk, panes: desk.panes.map((pane) => pane.id === action.id ? { ...pane, textFileRoot: action.root, textFilePath: action.path } : pane) })) };
    case "SET_TEXT_EDITOR_SETTINGS": return { ...state, desks: state.desks.map((desk) => ({ ...desk, panes: desk.panes.map((pane) => pane.id === action.id ? { ...pane, textEditorReadOnly: action.readOnly ?? pane.textEditorReadOnly ?? false, textEditorMode: action.mode ?? pane.textEditorMode ?? "plain" } : pane) })) };
    case "SET_TEXT_EDITOR_VIEW": return { ...state, desks: state.desks.map((desk) => ({ ...desk, panes: desk.panes.map((pane) => pane.id === action.id ? { ...pane, textEditorView: { root: action.root, path: action.path, selectionStart: action.selectionStart, selectionEnd: action.selectionEnd, scrollTop: action.scrollTop } } : pane) })) };
    case "SET_STORE_ARMED": return { ...state, storeArmed: action.value, updateArmed: action.value ? false : state.updateArmed };
    case "SET_UPDATE_ARMED": return { ...state, updateArmed: action.value, storeArmed: action.value ? false : state.storeArmed };
    case "SET_SHIFT_ARMED": return { ...state, shiftArmed: action.value };
    case "SET_PATCH_ARMED": return { ...state, patchSetArmed: action.value };
    default: return state;
  }
}
