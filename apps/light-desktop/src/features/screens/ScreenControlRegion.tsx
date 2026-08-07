import { useState } from "react";
import type { ScreenConfiguration } from "../../api/types";
import { screenShowsPlaybacks, useEncoderPlacement } from "./encoderPlacement";
import {
	LowerSectionSwitch,
	LowerSectionSwitchProvider,
	type LowerSectionView,
} from "./LowerSectionSwitch";
import { ProgrammerControlSurfaceRegion } from "./ProgrammerControlSurfaceRegion";
import { ScreenPlaybackSection } from "./ScreenPlaybackSection";

/**
 * Lower region of a secondary screen. A screen configured for both sections switches
 * between them; otherwise it renders whichever single section it carries. The switch
 * itself is placed by the visible section rather than by a row of its own.
 */
export function ScreenControlRegion({
	screen,
}: {
	screen: ScreenConfiguration;
}) {
	const placement = useEncoderPlacement(screen.id);
	const holdsEncoders = Boolean(placement?.holdsEncoders);
	const playbacks = screenShowsPlaybacks(screen);
	const [view, setView] = useState<LowerSectionView>("encoders");
	if (!holdsEncoders)
		return playbacks ? <ScreenPlaybackSection screen={screen} /> : null;
	if (!playbacks)
		return <ProgrammerControlSurfaceRegion screenId={screen.id} />;
	return (
		<LowerSectionSwitchProvider
			switchNode={<LowerSectionSwitch view={view} onView={setView} />}
		>
			{view === "encoders" ? (
				<ProgrammerControlSurfaceRegion screenId={screen.id} />
			) : (
				<ScreenPlaybackSection screen={screen} />
			)}
		</LowerSectionSwitchProvider>
	);
}

/** True while the screen shell must reserve room for a lower control region. */
export function screenHasControlRegion(
	screen: ScreenConfiguration,
	holdsEncoders: boolean,
) {
	return holdsEncoders || screenShowsPlaybacks(screen);
}
