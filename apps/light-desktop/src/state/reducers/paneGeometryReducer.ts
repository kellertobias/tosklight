import { type AppState, GRID_COLUMNS, GRID_ROWS } from "../../types";
import type { Action } from "../appActions";
import { clamp, overlaps } from "../reducerHelpers";

function updatePane(
	state: AppState,
	id: string,
	patch: Record<string, unknown>,
): AppState {
	return {
		...state,
		desks: state.desks.map((desk) =>
			desk.id !== state.activeDeskId
				? desk
				: {
						...desk,
						panes: desk.panes.map((pane) =>
							pane.id === id ? { ...pane, ...patch } : pane,
						),
					},
		),
	};
}

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
			return updatePane(state, action.id, { showGroupShortcuts: action.value });
		case "SET_PANE_FIXTURE_ACTIVE_ONLY":
			return updatePane(state, action.id, {
				fixtureSheetActiveOnly: action.value,
			});
		case "SET_PANE_FIXTURE_COMPACT_MODE":
			return updatePane(state, action.id, {
				fixtureSheetCompactMode: action.mode,
			});
		case "SET_PANE_CUE_SIDEBAR":
			return updatePane(state, action.id, { showCueSidebar: action.value });
		case "SET_PANE_CUELIST_COMPACT_ROWS":
			return updatePane(state, action.id, {
				cueListCompactRows: action.value,
			});
		case "SET_PANE_CUE_INFORMATION_BLOCK":
			return updatePane(state, action.id, {
				cueInformationBlock: action.value,
			});
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
