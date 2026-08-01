export const PATCH_OBJECT_CHANGED_EVENT = "light:patch-object-changed";

export function publishPatchObjectChanged(showId: string): void {
	window.dispatchEvent(
		new CustomEvent(PATCH_OBJECT_CHANGED_EVENT, { detail: { showId } }),
	);
}
