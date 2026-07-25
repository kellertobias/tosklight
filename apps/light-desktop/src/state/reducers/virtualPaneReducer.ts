import type { AppState } from "../../types";
import type { Action } from "../appActions";

export function reduceVirtualPane(
	state: AppState,
	action: Action,
): AppState | undefined {
	switch (action.type) {
		case "SET_VIRTUAL_PLAYBACK_CELL":
			return {
				...state,
				desks: state.desks.map((desk) =>
					desk.id !== state.activeDeskId
						? desk
						: {
								...desk,
								panes: desk.panes.map((pane) => {
									if (pane.id !== action.id) return pane;
									const cells = [...(pane.virtualPlaybackCells ?? [])];
									const current = cells[action.index] ?? {
										playbackNumber: null,
										action: "go" as const,
									};
									cells[action.index] = {
										playbackNumber:
											action.playbackNumber === undefined
												? current.playbackNumber
												: action.playbackNumber,
										action: action.action ?? current.action,
									};
									return { ...pane, virtualPlaybackCells: cells };
								}),
							},
				),
			};
		case "SET_VIRTUAL_PLAYBACK_EXCLUSION_ZONES":
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
												virtualPlaybackExclusionZones: action.zones.map(
													(zone) => ({ ...zone, slots: [...zone.slots] }),
												),
											}
										: pane,
								),
							},
				),
			};
		case "SET_FILE_MANAGER_SHOW_HIDDEN":
			return {
				...state,
				desks: state.desks.map((desk) => ({
					...desk,
					panes: desk.panes.map((pane) =>
						pane.id === action.id
							? { ...pane, fileManagerShowHidden: action.value }
							: pane,
					),
				})),
			};
		default:
			return undefined;
	}
}
