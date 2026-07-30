import {
	MAX_PLAYBACK_PAGE,
	VIRTUAL_PLAYBACKS_PER_PAGE,
} from "../../api/virtualPlaybackAddress";
import type { AppState } from "../../types";
import type { Action } from "../appActions";
import { clamp } from "../reducerHelpers";

export const MAX_VIRTUAL_PLAYBACK_CELLS = VIRTUAL_PLAYBACKS_PER_PAGE;
export { MAX_PLAYBACK_PAGE };

export function reducePaneOptions(
	state: AppState,
	action: Action,
): AppState | undefined {
	switch (action.type) {
		case "SET_PANE_LAYOUT_GROUP":
			return {
				...state,
				desks: state.desks.map((desk) =>
					desk.id !== state.activeDeskId
						? desk
						: {
								...desk,
								panes: desk.panes.map((pane) =>
									pane.id === action.id
										? { ...pane, layoutGroupId: action.groupId }
										: pane,
								),
							},
				),
			};
		case "SET_PANE_STAGE_OPTION":
			return {
				...state,
				stageView:
					action.option === "stageView"
						? (action.value as AppState["stageView"])
						: state.stageView,
				desks: state.desks.map((desk) =>
					desk.id !== state.activeDeskId
						? desk
						: {
								...desk,
								panes: desk.panes.map((pane) =>
									pane.id === action.id
										? { ...pane, [action.option]: action.value }
										: pane,
								),
							},
				),
			};
		case "SET_PANE_PRESET_FAMILY":
			return {
				...state,
				desks: state.desks.map((desk) =>
					desk.id !== state.activeDeskId
						? desk
						: {
								...desk,
								panes: desk.panes.map((pane) =>
									pane.id === action.id
										? { ...pane, presetFamily: action.family }
										: pane,
								),
							},
				),
			};
		case "SET_PANE_PRESET_COLORS":
			return {
				...state,
				desks: state.desks.map((desk) =>
					desk.id !== state.activeDeskId
						? desk
						: {
								...desk,
								panes: desk.panes.map((pane) =>
									pane.id === action.id
										? { ...pane, presetPoolColors: action.value }
										: pane,
								),
							},
				),
			};
		case "SET_PANE_SCHEDULER_LAYOUT":
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
												schedulerShowList: action.showList,
												schedulerShowCalendar: action.showCalendar,
											}
										: pane,
								),
							},
				),
			};
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
