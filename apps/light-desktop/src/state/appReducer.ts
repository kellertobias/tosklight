import type { AppState } from "../types";
import type { Action } from "./appActions";
import { reduceControls } from "./reducers/controlReducer";
import { reduceHydration } from "./reducers/hydrationReducer";
import { reduceNavigation } from "./reducers/navigationReducer";
import { reducePaneGeometry } from "./reducers/paneGeometryReducer";
import { reducePaneOptions } from "./reducers/paneOptionsReducer";
import { reduceVirtualPane } from "./reducers/virtualPaneReducer";
import { reduceWorkspace } from "./reducers/workspaceReducer";

export type { Action } from "./appActions";
export { initialState } from "./initialState";

const reducers = [
	reduceNavigation,
	reduceHydration,
	reducePaneGeometry,
	reducePaneOptions,
	reduceVirtualPane,
	reduceWorkspace,
	reduceControls,
];

export function appReducer(state: AppState, action: Action): AppState {
	for (const reducer of reducers) {
		const next = reducer(state, action);
		if (next) return next;
	}
	return state;
}
