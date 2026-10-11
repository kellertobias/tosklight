import { usePlaybackDeskView } from "../../../features/playbackRuntime/PlaybackRuntimeView";
import { useScreensOptional } from "../../../features/screens/ScreensContext";

/** Show the persisted command authority even when its screen is closed. */
export function usePlaybackCommandContext(
	desktops: readonly {
		panes: readonly { surface_id?: string; title: string }[];
	}[] = [],
) {
	const desk = usePlaybackDeskView();
	const screens = useScreensOptional();
	const surfaceId = desk?.selected_surface_id;
	const surface = surfaceId
		? screens?.surfaces?.playback_surfaces[surfaceId]
		: null;
	const pane = desktops
		.flatMap((desktop) => desktop.panes)
		.find(
			(candidate) => Boolean(surfaceId) && candidate.surface_id === surfaceId,
		);
	const screen = screens?.screens?.screens.find(
		(candidate) => Boolean(surfaceId) && candidate.surface_id === surfaceId,
	);
	return {
		name:
			pane?.title ??
			screen?.name ??
			surface?.name ??
			(surfaceId === "main" ? "Main" : "Playback"),
		page: desk?.command_page ?? null,
	};
}
