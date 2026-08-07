import type { ScreenConfiguration } from "../../api/types";
import { PlaybackFaderBank } from "../../components/control/PlaybackFaderBank";
import { useLowerSectionSwitch } from "./LowerSectionSwitch";
import { ScreenPageControls } from "./ScreenPageControls";
import { useScreens } from "./ScreensContext";
import { useScreenPlaybackPage } from "./useScreenPlaybackPage";

/**
 * Mounted only while a secondary screen shows Playbacks or Page Controls.
 * Either surface remains independently configurable.
 */
export function ScreenPlaybackSection({
	screen,
}: {
	screen: ScreenConfiguration;
}) {
	const { screens, bootstrap } = useScreens();
	const sectionSwitch = useLowerSectionSwitch();
	const page = useScreenPlaybackPage(screen, screens);
	return (
		<section
			className={`screen-playbacks ${screen.show_playbacks ? "" : "page-controls-only"}`}
		>
			{page == null ? (
				<div
					className="playback-fader-bank playback-authority-status"
					role="status"
				>
					Loading Playbacks…
				</div>
			) : (
				screen.show_playbacks && (
					<PlaybackFaderBank
						pageNumber={page}
						firstSlot={screen.first_playback_slot}
						count={screen.playback_count}
						rows={screen.playback_rows}
						playbackLayout={screen.playback_layout}
						hardwareConnected={Boolean(bootstrap?.hardware_connected)}
					/>
				)
			)}
			{/* The switch stays reachable even while the page is still resolving. */}
			{page != null && screen.show_page_controls ? (
				<ScreenPageControls screen={screen} page={page} />
			) : (
				sectionSwitch && (
					<div className="screen-page-controls switch-only">
						{sectionSwitch}
					</div>
				)
			)}
		</section>
	);
}
