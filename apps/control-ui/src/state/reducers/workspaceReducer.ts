import type { AppState } from "../../types";
import type { Action } from "../appActions";
import {
	clamp,
	cueListWindowKind,
	cueListWindowTitle,
	normalizeFixtureSheetColumns,
	overlaps,
} from "../reducerHelpers";

export function reduceWorkspace(
	state: AppState,
	action: Action,
): AppState | undefined {
	switch (action.type) {
		case "SET_STAGE_MODE":
			return { ...state, stageMode: action.value };
		case "SET_STAGE_VIEW":
			return { ...state, stageView: action.value };
		case "SET_STAGE_NAVIGATION":
			return {
				...state,
				stageZoom: action.zoom ?? state.stageZoom,
				stagePanX: action.panX ?? state.stagePanX,
				stagePanY: action.panY ?? state.stagePanY,
				stageOrbitX: action.orbitX ?? state.stageOrbitX,
				stageOrbitY: action.orbitY ?? state.stageOrbitY,
			};
		case "SET_STAGE_OPTIONS":
			return {
				...state,
				stageGroupsVisible: action.groupsVisible ?? state.stageGroupsVisible,
				stageShowSelection: action.showSelection ?? state.stageShowSelection,
				stageShowFloorGrid: action.showFloorGrid ?? state.stageShowFloorGrid,
				stageShowBeamGuides: action.showBeamGuides ?? state.stageShowBeamGuides,
				stageEnvironmentBrightness: clamp(
					action.environmentBrightness ?? state.stageEnvironmentBrightness,
					0,
					2,
				),
			};
		case "SET_DMX_DOT_SIZE":
			return { ...state, dmxDotSize: action.value };
		case "SET_FIXTURE_SHEET_OPTIONS":
			return {
				...state,
				fixtureSheetOrder: action.order ?? state.fixtureSheetOrder,
				fixtureSheetActiveOnly:
					action.activeOnly ?? state.fixtureSheetActiveOnly,
				fixtureSheetCueListId: action.cueListId ?? state.fixtureSheetCueListId,
				fixtureSheetColumns: normalizeFixtureSheetColumns(
					action.columns,
					state.fixtureSheetColumns,
				),
				fixtureSheetShowType: action.showType ?? state.fixtureSheetShowType,
				fixtureSheetIncludedHeads:
					action.includedHeads ?? state.fixtureSheetIncludedHeads,
			};
		case "SET_BUILTIN_GROUPS_VISIBLE":
			return action.window === "fixtures"
				? { ...state, fixtureGroupsVisible: action.value }
				: { ...state, presetGroupsVisible: action.value };
		case "REMOVE_PANE":
			return {
				...state,
				paneSettingsId: null,
				desks: state.desks.map((desk) =>
					desk.id !== state.activeDeskId
						? desk
						: {
								...desk,
								panes: desk.panes.filter((pane) => pane.id !== action.id),
							},
				),
			};
		case "OPEN_DESK_SETTINGS":
			return {
				...state,
				deskSettingsOpen: Boolean(action.id),
				deskSettingsId: action.id,
			};
		case "UPDATE_DESK":
			return {
				...state,
				desks: state.desks.map((desk) =>
					desk.id === action.id
						? {
								...desk,
								name: action.name ?? desk.name,
								icon: action.icon ?? desk.icon,
							}
						: desk,
				),
			};
		case "DELETE_DESK": {
			if (state.desks.length <= 1) return state;
			const desks = state.desks.filter((desk) => desk.id !== action.id);
			return {
				...state,
				desks,
				activeDeskId:
					state.activeDeskId === action.id ? desks[0].id : state.activeDeskId,
				deskSettingsOpen: false,
				deskSettingsId: null,
			};
		}
		case "ADD_WINDOW": {
			if (!state.windowPicker) return state;
			const kind = cueListWindowKind(action.kind);
			const pane = {
				id: `${kind}-${Date.now()}`,
				kind,
				title:
					kind === "help"
						? "Help"
						: kind === "development"
							? "Development"
							: kind === "virtual_playbacks"
								? "Virtual Playbacks"
								: kind === "file_manager"
									? "File Manager"
									: kind === "text_editor"
										? "Text Editor"
										: cueListWindowTitle(
												kind[0].toUpperCase() + kind.slice(1),
												kind,
											),
				...(kind === "virtual_playbacks"
					? {
							virtualPlaybackRows: 2,
							virtualPlaybackColumns: 2,
							virtualPlaybackCells: [],
							virtualPlaybackExclusionZones: [],
						}
					: {}),
				...(kind === "file_manager" ? { fileManagerShowHidden: false } : {}),
				...(kind === "text_editor"
					? { textEditorReadOnly: false, textEditorMode: "plain" as const }
					: {}),
				...state.windowPicker,
			};
			const activeDesk = state.desks.find(
				(desk) => desk.id === state.activeDeskId,
			);
			if (activeDesk?.panes.some((item) => overlaps(pane, item)))
				return { ...state, windowPicker: null };
			return {
				...state,
				windowPicker: null,
				desks: state.desks.map((desk) =>
					desk.id !== state.activeDeskId
						? desk
						: { ...desk, panes: [...desk.panes, pane] },
				),
			};
		}
		default:
			return undefined;
	}
}
