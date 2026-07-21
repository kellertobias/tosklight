import type {
	AppState,
	BuiltInWindow,
	DevelopmentView,
	FixtureSheetColumn,
	GridRect,
	TextEditorMode,
	VirtualPlaybackExclusionZone,
	WindowSettings,
} from "../types";

export type Action =
	| { type: "SET_DOCK_MODE"; mode: AppState["dockMode"] }
	| { type: "OPEN_DESK"; id: string }
	| { type: "OPEN_BUILTIN"; kind: BuiltInWindow }
	| { type: "CLOSE_FILE_MANAGER" }
	| { type: "TOGGLE_CONTROL_MODE" }
	| { type: "SET_PANE_SETTINGS"; id: string | null }
	| { type: "SET_PANE_RECT"; id: string; rect: Partial<GridRect> }
	| { type: "SET_PANE_GROUP_SHORTCUTS"; id: string; value: boolean }
	| { type: "SET_PANE_CUE_SIDEBAR"; id: string; value: boolean }
	| {
			type: "SET_PANE_CUELIST";
			id: string;
			source?: "fixed" | "follow-selection";
			number?: number;
	  }
	| {
			type: "SET_PANE_STAGE_OPTION";
			id: string;
			option: "stageView" | "followPreload" | "showBeamGuides";
			value: AppState["stageView"] | boolean;
	  }
	| {
			type: "SET_PANE_PRESET_FAMILY";
			id: string;
			family: AppState["presetFamily"];
	  }
	| { type: "SET_PANE_PRESET_COLORS"; id: string; value: boolean }
	| { type: "SET_PANE_DEVELOPMENT_VIEW"; id: string; value: DevelopmentView }
	| {
			type: "SET_VIRTUAL_PLAYBACK_GRID";
			id: string;
			rows: number;
			columns: number;
	  }
	| {
			type: "SET_VIRTUAL_PLAYBACK_CELL";
			id: string;
			index: number;
			playbackNumber?: number | null;
			action?: "go" | "toggle";
	  }
	| {
			type: "SET_VIRTUAL_PLAYBACK_EXCLUSION_ZONES";
			id: string;
			zones: VirtualPlaybackExclusionZone[];
	  }
	| { type: "SET_FILE_MANAGER_SHOW_HIDDEN"; id: string; value: boolean }
	| { type: "SET_TEXT_EDITOR_FILE"; id: string; root: string; path: string }
	| {
			type: "SET_TEXT_EDITOR_SETTINGS";
			id: string;
			readOnly?: boolean;
			mode?: TextEditorMode;
	  }
	| {
			type: "SET_TEXT_EDITOR_VIEW";
			id: string;
			root: string;
			path: string;
			selectionStart: number;
			selectionEnd: number;
			scrollTop: number;
	  }
	| { type: "SET_STAGE_MODE"; value: AppState["stageMode"] }
	| { type: "SET_STAGE_VIEW"; value: AppState["stageView"] }
	| {
			type: "SET_STAGE_NAVIGATION";
			zoom?: number;
			panX?: number;
			panY?: number;
			orbitX?: number;
			orbitY?: number;
	  }
	| {
			type: "SET_STAGE_OPTIONS";
			groupsVisible?: boolean;
			showSelection?: boolean;
			showFloorGrid?: boolean;
			showBeamGuides?: boolean;
			environmentBrightness?: number;
	  }
	| { type: "SET_DMX_DOT_SIZE"; value: AppState["dmxDotSize"] }
	| {
			type: "SET_FIXTURE_SHEET_OPTIONS";
			order?: AppState["fixtureSheetOrder"];
			activeOnly?: boolean;
			cueListId?: string;
			columns?: FixtureSheetColumn[];
			showType?: boolean;
			includedHeads?: AppState["fixtureSheetIncludedHeads"];
	  }
	| {
			type: "SET_BUILTIN_GROUPS_VISIBLE";
			window: "fixtures" | "presets";
			value: boolean;
	  }
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
	| {
			type: "SET_MODAL";
			modal:
				| "setupOpen"
				| "specialDialogsOpen"
				| "systemControlsOpen"
				| "preloadStoreOpen"
				| "debugOpen"
				| "deskSettingsOpen"
				| "storeSettingsOpen";
			value: boolean;
	  }
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
	| {
			type: "HYDRATE_LAYOUT";
			desks: AppState["desks"];
			activeDeskId: string;
			windowSettings?: Partial<WindowSettings>;
	  };
