import type { Action } from "../../../state/appActions";

/** Show Patch › Points, as the Point encoder's **Manage Points** opens it (TL-651). */
export const OPEN_POINTS_PATCH_ACTION = {
	type: "OPEN_BUILTIN",
	kind: "patch",
	patchView: "points",
} as const satisfies Action;

/** Show Patch › Points with one new aim Point created there (the Point encoder's Create Point). */
export const CREATE_POINT_ACTION = {
	type: "OPEN_BUILTIN",
	kind: "patch",
	patchView: "points",
	patchRequest: "create_point",
} as const satisfies Action;
