import type { ScreenConfiguration, ScreenSnapshot } from "../../api/types";
import { usePlaybackDeskView } from "../playbackRuntime/PlaybackRuntimeView";

/**
 * Resolves the single page authority a secondary screen surface may display.
 *
 * `follow_main` reads only the exact desk projection and never owns a screen
 * page. `independent` reads only its own stored screen page. Either mode
 * returns `null` while its authority is missing so the caller can gate instead
 * of borrowing the other mode's page.
 */
export function useScreenPlaybackPage(
	screen: ScreenConfiguration,
	screens: ScreenSnapshot | null,
): number | null {
	const followsMain = screen.page_mode === "follow_main";
	const desk = usePlaybackDeskView(followsMain);
	if (followsMain) return desk?.active_page ?? null;
	return screens?.active_pages[screen.id] ?? null;
}
