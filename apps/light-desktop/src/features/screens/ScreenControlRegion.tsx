import { Button } from "@tosklight/ui";
import { useState } from "react";
import type { ScreenConfiguration } from "../../api/types";
import { screenShowsPlaybacks, useEncoderPlacement } from "./encoderPlacement";
import { ProgrammerControlSurfaceRegion } from "./ProgrammerControlSurfaceRegion";
import { ScreenPlaybackSection } from "./ScreenPlaybackSection";

/**
 * Lower region of a secondary screen. A screen configured for both sections switches
 * between them; otherwise it renders whichever single section it carries.
 */
export function ScreenControlRegion({
	screen,
}: {
	screen: ScreenConfiguration;
}) {
	const placement = useEncoderPlacement(screen.id);
	const holdsEncoders = Boolean(placement?.holdsEncoders);
	const playbacks = screenShowsPlaybacks(screen);
	const [view, setView] = useState<"encoders" | "playbacks">("encoders");
	if (!holdsEncoders)
		return playbacks ? (
			<ScreenPlaybackSection screen={screen} />
		) : (
			<ProgrammerControlSurfaceRegion screenId={screen.id} />
		);
	if (!playbacks)
		return <ProgrammerControlSurfaceRegion screenId={screen.id} />;
	return (
		<>
			<div className="screen-section-switch" role="group" aria-label="Lower section">
				<Button
					active={view === "playbacks"}
					aria-pressed={view === "playbacks"}
					onClick={() => setView("playbacks")}
				>
					Playback
				</Button>
				<Button
					active={view === "encoders"}
					aria-pressed={view === "encoders"}
					onClick={() => setView("encoders")}
				>
					Encoders
				</Button>
			</div>
			{view === "encoders" ? (
				<ProgrammerControlSurfaceRegion screenId={screen.id} />
			) : (
				<ScreenPlaybackSection screen={screen} />
			)}
		</>
	);
}

/** True while the screen shell must reserve room for a lower control region. */
export function screenHasControlRegion(
	screen: ScreenConfiguration,
	holdsEncoders: boolean,
) {
	return holdsEncoders || screenShowsPlaybacks(screen);
}
