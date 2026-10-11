import type { ScreenConfiguration, ScreenSnapshot } from "../../api/types";

/** Uses the server-resolved page for this registered surface. */
export function useScreenPlaybackPage(
	screen: ScreenConfiguration,
	screens: ScreenSnapshot | null,
): number | null {
	return screens?.surfaces?.resolved_pages[screen.surface_id] ?? null;
}
