import type { Action } from "../../state/appActions";

export const OPEN_MEDIA_PATCH_ACTION = {
	type: "OPEN_BUILTIN",
	kind: "patch",
	patchView: "media",
} as const satisfies Action;
