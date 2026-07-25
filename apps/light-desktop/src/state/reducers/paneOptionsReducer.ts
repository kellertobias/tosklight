import type { AppState } from "../../types";
import type { Action } from "../appActions";
import { clamp } from "../reducerHelpers";

export function reducePaneOptions(
	state: AppState,
	action: Action,
): AppState | undefined {
	switch (action.type) {
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
		case "SET_PANE_DEVELOPMENT_VIEW":
			return {
				...state,
				desks: state.desks.map((desk) =>
					desk.id !== state.activeDeskId
						? desk
						: {
								...desk,
								panes: desk.panes.map((pane) =>
									pane.id === action.id
										? { ...pane, developmentView: action.value }
										: pane,
								),
							},
				),
			};
		case "SET_VIRTUAL_PLAYBACK_GRID":
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
												virtualPlaybackRows: clamp(action.rows, 1, 12),
												virtualPlaybackColumns: clamp(action.columns, 1, 12),
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
