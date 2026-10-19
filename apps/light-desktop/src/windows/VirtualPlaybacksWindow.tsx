import { ScreenPlaybackFit } from "../features/screens/ScreenPlaybackFit";
import { PlaybackPaneTitleControls } from "./PlaybackPaneTitleControls";
import { Button, ErrorAlert } from "@tosklight/ui";
import { gridPlaybackLayout } from "@tosklight/ui/playback";
import { useState } from "react";
import { PlaybackFaderBank } from "../components/control/PlaybackFaderBank";
import { useScreens } from "../features/screens/ScreensContext";
import { useApp } from "../state/AppContext";
import type { PaneModel } from "../types";
import type { WindowProps } from "./windowTypes";

/** A pane renders the same sparse Playback addresses as every other surface. */
export function VirtualPlaybacksWindow({ paneId, active = true }: WindowProps) {
	const { state, dispatch } = useApp();
	const screens = useScreens();
	const [error, setError] = useState<string | null>(null);
	const [saving, setSaving] = useState(false);
	const pane = state.desks
		.flatMap((desk) => desk.panes)
		.find((candidate) => candidate.id === paneId);
	const surfaceId = pane?.surface_id ?? `pane:${paneId ?? "builtin"}`;
	if (!active) return null;
	const registry = screens.surfaces?.playback_surfaces;
	const create = async () => {
		if (!registry) return;
		setSaving(true);
		setError(null);
		try {
			await screens.updatePlaybackSurface(surfaceId, {
				name: pane?.title ?? "Playback pane",
				layout: gridPlaybackLayout(401, 20, 14),
				page_source: { type: "follow", surface_id: "main" },
			});
			if (pane)
				dispatch({ type: "SET_PANE_PLAYBACK_SURFACE", id: pane.id, surfaceId });
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setSaving(false);
		}
	};
	return (
		<section className="playback-pane" aria-label="Playbacks">
			{registry?.[surfaceId] ? (
				<>
				<PlaybackPaneTitleControls surfaceId={surfaceId} />
				<PlaybackPaneBank pane={pane} surfaceId={surfaceId} />
				</>
			) : (
				<>
					<p>
						Configure this playback surface to use positions 401–680 in a 20 ×
						14 layout.
					</p>
					<Button disabled={!registry || saving} onClick={() => void create()}>
						{saving ? "Creating surface…" : "Create playback surface"}
					</Button>
				</>
			)}
			{error && <ErrorAlert role="alert">{error}</ErrorAlert>}
		</section>
	);
}

function PlaybackPaneBank({ pane, surfaceId }: { pane?: PaneModel; surfaceId: string }) {
	const { surfaces, bootstrap } = useScreens();
	if (pane?.playbackViewMode !== "screen")
		return <ScreenPlaybackFit className="screen-bank-fit"><PlaybackFaderBank surfaceId={surfaceId} tilePresentation={!pane?.playbackShowControls} /></ScreenPlaybackFit>;
	const screen = pane.playbackScreenSurfaceId ?? "main";
	if (!surfaces?.playback_surfaces[screen])
		return <ErrorAlert role="alert">The selected screen layout is unavailable. Choose another screen in Pane Settings.</ErrorAlert>;
	return <ScreenPlaybackFit className="screen-bank-fit"><PlaybackFaderBank surfaceId={surfaceId} tilePresentation={!pane.playbackShowControls} layoutSourceSurfaceId={screen} hardwareConnected={Boolean(bootstrap?.hardware_connected)} /></ScreenPlaybackFit>;
}
