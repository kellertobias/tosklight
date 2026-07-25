import { type AppState, GRID_COLUMNS, GRID_ROWS } from "../../types";
import type { Action } from "../appActions";
import { clamp, overlaps } from "../reducerHelpers";

export function reducePaneGeometry(
	state: AppState,
	action: Action,
): AppState | undefined {
	switch (action.type) {
		case "SET_PANE_RECT":
			return {
				...state,
				desks: state.desks.map((desk) => {
					if (desk.id !== state.activeDeskId) return desk;
					const pane = desk.panes.find((item) => item.id === action.id);
					if (!pane) return desk;
					const x = clamp(action.rect.x ?? pane.x, 1, GRID_COLUMNS);
					const y = clamp(action.rect.y ?? pane.y, 1, GRID_ROWS);
					const candidate = {
						...pane,
						x,
						y,
						width: clamp(
							action.rect.width ?? pane.width,
							1,
							GRID_COLUMNS - x + 1,
						),
						height: clamp(
							action.rect.height ?? pane.height,
							1,
							GRID_ROWS - y + 1,
						),
					};
					if (
						desk.panes.some(
							(item) => item.id !== pane.id && overlaps(candidate, item),
						)
					)
						return desk;
					return {
						...desk,
						panes: desk.panes.map((item) =>
							item.id === pane.id ? candidate : item,
						),
					};
				}),
			};
		case "SET_PANE_GROUP_SHORTCUTS":
			return {
				...state,
				desks: state.desks.map((desk) =>
					desk.id !== state.activeDeskId
						? desk
						: {
								...desk,
								panes: desk.panes.map((pane) =>
									pane.id === action.id
										? { ...pane, showGroupShortcuts: action.value }
										: pane,
								),
							},
				),
			};
		case "SET_PANE_CUE_SIDEBAR":
			return {
				...state,
				desks: state.desks.map((desk) =>
					desk.id !== state.activeDeskId
						? desk
						: {
								...desk,
								panes: desk.panes.map((pane) =>
									pane.id === action.id
										? { ...pane, showCueSidebar: action.value }
										: pane,
								),
							},
				),
			};
		case "SET_PANE_CUELIST":
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
												cueListSource:
													action.source ?? pane.cueListSource ?? "fixed",
												...(action.number != null
													? { fixedCueListNumber: action.number }
													: pane.fixedCueListNumber != null
														? { fixedCueListNumber: pane.fixedCueListNumber }
														: {}),
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
