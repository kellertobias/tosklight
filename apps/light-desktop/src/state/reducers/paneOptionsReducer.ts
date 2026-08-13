import {
	MAX_PLAYBACK_PAGE,
	VIRTUAL_PLAYBACKS_PER_PAGE,
} from "../../api/virtualPlaybackAddress";
import type { AppState, PaneModel } from "../../types";
import type { Action } from "../appActions";
import { clamp } from "../reducerHelpers";

export const MAX_VIRTUAL_PLAYBACK_CELLS = VIRTUAL_PLAYBACKS_PER_PAGE;
export { MAX_PLAYBACK_PAGE };

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
				mediaMainSectionId: action.state.mainSectionId,
				mediaRightPaneVisible: action.state.rightPaneVisible,
			}));
		case "SET_PANE_RUNNING_FILTER":
			return updateActivePane(state, action.id, (pane) => ({
				...pane,
				runningFilter: action.filter,
			}));
		case "SET_VIRTUAL_PLAYBACK_GRID": {
			const grid = normalizeVirtualPlaybackGrid(
				action.rows,
				action.columns,
				action.changed,
			);
			return {
				...state,
				desks: state.desks.map((desk) =>
					desk.id !== state.activeDeskId
						? desk
						: {
								...desk,
								panes: desk.panes.map((pane) =>
									pane.id === action.id
										? {
												...pane,
												virtualPlaybackRows: grid.rows,
												virtualPlaybackColumns: grid.columns,
											}
										: pane,
								),
							},
				),
			};
		}
		case "SET_VIRTUAL_PLAYBACK_PAGE_MODE":
			return {
				...state,
				desks: state.desks.map((desk) =>
					desk.id !== state.activeDeskId
						? desk
						: {
								...desk,
								panes: desk.panes.map((pane) =>
									pane.id === action.id
										? {
												...pane,
												virtualPlaybackPageMode: action.mode,
												virtualPlaybackPinnedPage: normalizePlaybackPage(
													action.pinnedPage ??
														pane.virtualPlaybackPinnedPage ??
														1,
												),
											}
										: pane,
								),
							},
				),
			};
		default:
			return undefined;
	}
}

export function normalizeVirtualPlaybackGrid(
	rows: number,
	columns: number,
	changed: "rows" | "columns" = "columns",
) {
	const normalizedRows = normalizedGridDimension(rows);
	const normalizedColumns = normalizedGridDimension(columns);
	if (normalizedRows * normalizedColumns <= MAX_VIRTUAL_PLAYBACK_CELLS)
		return { rows: normalizedRows, columns: normalizedColumns };
	if (changed === "rows")
		return {
			rows: Math.max(
				1,
				Math.floor(MAX_VIRTUAL_PLAYBACK_CELLS / normalizedColumns),
			),
			columns: normalizedColumns,
		};
	return {
		rows: normalizedRows,
		columns: Math.max(
			1,
			Math.floor(MAX_VIRTUAL_PLAYBACK_CELLS / normalizedRows),
		),
	};
}

function normalizedGridDimension(value: number) {
	if (!Number.isFinite(value)) return 1;
	return clamp(Math.trunc(value), 1, MAX_VIRTUAL_PLAYBACK_CELLS);
}

function normalizePlaybackPage(value: number) {
	if (!Number.isFinite(value)) return 1;
	return clamp(Math.trunc(value), 1, MAX_PLAYBACK_PAGE);
}
