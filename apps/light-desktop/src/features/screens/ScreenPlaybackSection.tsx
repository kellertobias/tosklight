import type { ScreenConfiguration } from "../../api/types";
import { PlaybackFaderBank } from "../../components/control/PlaybackFaderBank";
import { ScreenPageControls } from "./ScreenPageControls";
import { useScreens } from "./ScreensContext";
import { useScreenPlaybackPage } from "./useScreenPlaybackPage";

/**
 * Mounted only while a secondary screen shows Playbacks, so a screen without
 * them opens no Playback or Page authority at all.
 */
export function ScreenPlaybackSection({
	screen,
}: {
	screen: ScreenConfiguration;
}) {
	const { screens, bootstrap } = useScreens();
	const page = useScreenPlaybackPage(screen, screens);
	return (
		<section className="screen-playbacks">
			{page == null ? (
				<div className="playback-fader-bank playback-authority-status" role="status">
					Loading Playbacks…
				</div>
			) : (
				<>
					<PlaybackFaderBank
						pageNumber={page}
						firstSlot={screen.first_playback_slot}
						count={screen.playback_count}
						rows={screen.playback_rows}
						playbackLayout={screen.playback_layout}
						hardwareConnected={Boolean(bootstrap?.hardware_connected)}
					/>
					{screen.show_page_controls && (
						<ScreenPageControls screen={screen} page={page} />
					)}
				</>
			)}
		</section>
	);
}
