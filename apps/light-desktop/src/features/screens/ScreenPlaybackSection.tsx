import type { ScreenConfiguration } from "../../api/types";
import type { ReactNode } from "react";
import { HardwareControlSummary } from "../../components/control/HardwareControlSummary";
import { PlaybackFaderBank } from "../../components/control/PlaybackFaderBank";
import { screenShowsPageControls } from "./encoderPlacement";
import { useLowerSectionSwitch } from "./LowerSectionSwitch";
import { ScreenPageControls } from "./ScreenPageControls";
import { useScreens } from "./ScreensContext";
import { useScreenPlaybackPage } from "./useScreenPlaybackPage";

/**
 * Mounted only while a secondary screen shows Playbacks. Page controls address the page
 * of those Playbacks, so they follow them; the right column disappears with both of them
 * unless it still has to carry the Playback/Encoders switch.
 */
export function ScreenPlaybackSection({
	screen,
	encoders,
}: {
	screen: ScreenConfiguration;
	encoders?: ReactNode;
}) {
	const { screens, bootstrap } = useScreens();
	const sectionSwitch = useLowerSectionSwitch();
	const page = useScreenPlaybackPage(screen, screens);
	const pageControls = screenShowsPageControls(screen);
	const sideColumn = pageControls || Boolean(sectionSwitch) || Boolean(encoders);
	return (
		<section
			className={`screen-playbacks ${sideColumn ? "" : "playbacks-only"} ${encoders ? "with-compact-encoders" : ""}`}
		>
			{page == null ? (
				<div
					className="playback-fader-bank playback-authority-status"
					role="status"
				>
					Loading Playbacks…
				</div>
			) : (
				<PlaybackFaderBank
					pageNumber={page}
					firstSlot={screen.first_playback_slot}
					count={screen.playback_count}
					rows={screen.playback_rows}
					playbackLayout={screen.playback_layout}
					hardwareConnected={Boolean(bootstrap?.hardware_connected)}
				/>
			)}
			{/* The switch stays reachable even while the page is still resolving. */}
			{encoders ? (
				<aside className="screen-hardware-summary">
					{sectionSwitch}
					<HardwareControlSummary encoders={encoders} pageControl={page != null && pageControls ? <ScreenPageControls screen={screen} page={page} compact /> : null} />
				</aside>
			) : page != null && pageControls ? (
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
