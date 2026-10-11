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
	const { screens, surfaces, bootstrap } = useScreens();
	const sectionSwitch = useLowerSectionSwitch();
	const page = useScreenPlaybackPage(screen, screens);
	const layout = surfaces?.playback_surfaces[screen.surface_id]?.layout;
	const pageControls = screenShowsPageControls(screen);
	const sideColumn = pageControls || Boolean(sectionSwitch) || Boolean(encoders);
	return (
		<section
			className={`screen-playbacks ${sideColumn ? "" : "playbacks-only"} ${encoders ? "with-compact-encoders" : ""}`}
		>
			{page == null || !layout ? (
				<div
					className="playback-fader-bank playback-authority-status"
					role="status"
				>
					Loading Playbacks…
				</div>
			) : (
				<PlaybackFaderBank
					pageNumber={page}
					surfaceId={screen.surface_id}
					playbackLayout={layout}
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
