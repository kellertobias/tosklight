import { normalizePatchHiddenColumns } from "../reducerHelpers";
import type { AppState, PaneModel } from "../../types";
import type { Action } from "../appActions";
import { clamp } from "../reducerHelpers";

function updateActivePane(
	state: AppState,
	id: string,
	update: (pane: PaneModel) => PaneModel,
) {
	return {
		...state,
		desks: state.desks.map((desk) =>
			desk.id !== state.activeDeskId
				? desk
				: {
						...desk,
						panes: desk.panes.map((pane) =>
							pane.id === id ? update(pane) : pane,
						),
					},
		),
	};
}

export function reducePaneOptions(
	state: AppState,
	action: Action,
): AppState | undefined {
	switch (action.type) {
		case "SET_FILE_MANAGER_SHOW_HIDDEN":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				fileManagerShowHidden: action.value,
			}));
		case "SET_PANE_FIXTURE_OPTIONS":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				...(action.options.includedHeads === undefined
					? {}
					: { fixtureSheetIncludedHeads: action.options.includedHeads }),
				...(action.options.order === undefined
					? {}
					: { fixtureSheetOrder: action.options.order }),
				...(action.options.cueListId === undefined
					? {}
					: { fixtureSheetCueListId: action.options.cueListId }),
				...(action.options.columns === undefined
					? {}
					: { fixtureSheetColumns: action.options.columns }),
				...(action.options.showType === undefined
					? {}
					: { fixtureSheetShowType: action.options.showType }),
			}));
		case "SET_PANE_PATCH_HIDDEN_COLUMNS":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				patchHiddenColumns: normalizePatchHiddenColumns(action.columns),
			}));
		case "SET_PANE_POOL_COLUMNS":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				poolColumns: clamp(Math.trunc(action.value) || 1, 1, 24),
			}));
		case "SET_PANE_STAGE_OPTION":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				[action.option]: action.value,
			}));
		case "SET_PANE_FOG_VARIATION":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				[action.option]: clamp(action.value, 0, 1),
			}));
		case "SET_PANE_PRESET_FAMILY":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				presetFamily: action.family,
			}));
		case "SET_PANE_PRESET_COLORS":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				presetPoolColors: action.value,
			}));
		case "SET_PANE_CHANNEL_DISPLAY_MODE":
			return updateActivePane(state, action.id, (pane) =>
				Object.assign({}, pane, { channelDisplayMode: action.mode }),
			);
		case "SET_PANE_VISUALIZATION_ROWS":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				visualizationRows: action.rows,
			}));
		case "SET_PANE_SCHEDULER_LAYOUT":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				schedulerShowList: action.showList,
				schedulerShowCalendar: action.showCalendar,
			}));
		case "SET_PANE_MEDIA_STATE":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				mediaServerId: action.state.serverId,
				mediaLayerId: action.state.layerId,
				mediaBrowserMode: action.state.browserMode,
				mediaSourceFilter: action.state.sourceFilter,
				mediaControlSectionId: action.state.controlSectionId,
				mediaMainSectionId: action.state.mainSectionId,
				mediaRightPaneVisible: action.state.rightPaneVisible,
			}));
		case "SET_PANE_RUNNING_FILTER":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				runningFilter: action.filter,
			}));
		case "SET_PANE_PLAYBACK_CONTROLS":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				playbackShowControls: action.showControls,
			}));
		case "SET_PANE_PLAYBACK_VIEW":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				playbackViewMode: action.mode,
				playbackScreenSurfaceId: action.screenSurfaceId,
			}));
		case "SET_PANE_PLAYBACK_SURFACE":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				surface_id: action.surfaceId,
			}));
		default:
			return undefined;
	}
}
