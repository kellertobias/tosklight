import type { AppState } from "../../types";
import type { Action } from "../appActions";
import { cueListWindowKind } from "../reducerHelpers";

export function reduceNavigation(
	state: AppState,
	action: Action,
): AppState | undefined {
	switch (action.type) {
		case "SET_DOCK_MODE":
			return action.mode === "desks"
				? {
						...state,
						dockMode: "desks",
						builtIn: null,
						fileManagerReturn: null,
					}
				: {
						...state,
						dockMode: "builtins",
						builtIn: state.lastBuiltIn,
						fileManagerReturn: null,
					};
		case "OPEN_DESK":
			return {
				...state,
				activeDeskId: action.id,
				builtIn: null,
				dockMode: "desks",
				savingDesk: false,
				fileManagerReturn: null,
			};
		case "OPEN_BUILTIN": {
			const kind = cueListWindowKind(action.kind);
			if (
				kind === "cuelists" &&
				state.builtIn === "cuelists" &&
				state.cuelistBuiltInView === "cues"
			)
				return { ...state, cuelistBuiltInView: "pool", dockMode: "builtins" };
			if (kind === "file_manager")
				return {
					...state,
					builtIn: kind,
					dockMode: "builtins",
					fileManagerReturn:
						state.builtIn === "file_manager" && state.fileManagerReturn
							? state.fileManagerReturn
							: {
									dockMode: state.dockMode,
									activeDeskId: state.activeDeskId,
									builtIn: state.builtIn,
								},
				};
			return {
				...state,
				builtIn: kind,
				lastBuiltIn: kind,
				dockMode: "builtins",
				fileManagerReturn: null,
			};
		}
		case "CLOSE_FILE_MANAGER": {
			if (state.builtIn !== "file_manager") return state;
			const destination = state.fileManagerReturn;
			return destination
				? { ...state, ...destination, fileManagerReturn: null }
				: {
						...state,
						builtIn: null,
						dockMode: "desks",
						fileManagerReturn: null,
					};
		}
		case "OPEN_GROUPS_FROM_STAGE":
			return {
				...state,
				builtIn: "groups",
				lastBuiltIn: "groups",
				dockMode: "builtins",
				groupsReturnToStage: action.origin ?? "builtin",
			};
		case "RETURN_TO_STAGE":
			return state.groupsReturnToStage === "desk"
				? {
						...state,
						builtIn: null,
						dockMode: "desks",
						groupsReturnToStage: null,
					}
				: {
						...state,
						builtIn: "stage",
						lastBuiltIn: "stage",
						dockMode: "builtins",
						groupsReturnToStage: null,
					};
		case "TOGGLE_CONTROL_MODE":
			return {
				...state,
				controlMode:
					state.controlMode === "programmer" ? "playbacks" : "programmer",
			};
		case "SET_PANE_SETTINGS":
			return { ...state, paneSettingsId: action.id };
		case "TOGGLE_MAXIMIZE":
			return {
				...state,
				maximizedPaneId: state.maximizedPaneId === action.id ? null : action.id,
			};
		case "OPEN_WINDOW_PICKER":
			return { ...state, windowPicker: action.rect };
		case "START_SAVE_DESK":
			return { ...state, savingDesk: true };
		case "SAVE_DESK_TO": {
			const source = state.desks.find((desk) => desk.id === state.activeDeskId);
			return {
				...state,
				savingDesk: false,
				activeDeskId: action.id,
				desks: state.desks.map((desk) =>
					desk.id !== action.id || !source
						? desk
						: {
								...desk,
								panes: source.panes.map((pane, index) => ({
									...pane,
									id: `${desk.id}-${pane.kind}-${index + 1}`,
								})),
							},
				),
			};
		}
		default:
			return undefined;
	}
}
