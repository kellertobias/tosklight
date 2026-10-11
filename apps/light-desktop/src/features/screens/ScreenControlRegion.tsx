import { useState } from "react";
import type { ScreenConfiguration } from "../../api/types";
import { useApp } from "../../state/AppContext";
import { useHardwareConnected } from "../deskSnapshot/DeskSnapshotState";
import { useProgrammerEncoderContext } from "../../components/control/ParameterControls";
import { CompactEncoderAssignments } from "../../components/control/parameterControls/CompactEncoderAssignments";
import { useParameterController } from "../../components/control/parameterControls/useParameterController";
import { programmerEncoderOwnerContext } from "../../components/control/parameterControls/useEncoderNavigation";
import { VisibleEncoderCountProvider } from "../../components/control/parameterControls/VisibleEncoderCount";
import { useScreens } from "./ScreensContext";
import { screenShowsPlaybacks, useEncoderPlacement } from "./encoderPlacement";
import {
	LowerSectionSwitch,
	LowerSectionSwitchProvider,
	type LowerSectionView,
} from "./LowerSectionSwitch";
import { ProgrammerControlSurfaceRegion } from "./ProgrammerControlSurfaceRegion";
import { ScreenPlaybackFit } from "./ScreenPlaybackFit";
import { ScreenPlaybackSection } from "./ScreenPlaybackSection";
import { PlaybackSectionSettingsHost } from "../../components/control/PlaybackSectionSettingsHost";
import { SoloCommandEditor } from "../../components/control/CommandLineBar";
import { usePlaybackSoloEditor } from "../playbackSolo/PlaybackSoloEditorContext";

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
	const { screens } = useScreens();
	return (
		<VisibleEncoderCountProvider count={screens?.programmer_control_surface.visible_encoders ?? 6}>
			<ScreenControlRegionContent screen={screen} />
		</VisibleEncoderCountProvider>
	);
}

function ScreenControlRegionContent({ screen }: { screen: ScreenConfiguration }) {
	const placement = useEncoderPlacement(screen.id);
	const holdsEncoders = Boolean(placement?.holdsEncoders);
	const playbacks = screenShowsPlaybacks(screen);
	const [view, setView] = useState<LowerSectionView>("encoders");
	const { state } = useApp();
	const hardware = Boolean(useHardwareConnected() || state.midiProfile);
	const programmerContext = useProgrammerEncoderContext();
	const playbackVisible = playbacks && (!holdsEncoders || view === "playbacks");
	const compact = Boolean(screen.show_compact_encoders && hardware && playbackVisible);
	// One controller survives the lower-section switch. A mirror screen shares navigation
	// but never attaches another handler to the owner's hardware detents.
	const controller = useParameterController(holdsEncoders ? programmerContext && (!playbackVisible || compact) : compact, { bindHardware: holdsEncoders, ownerContext: programmerEncoderOwnerContext(programmerContext, state.builtIn) });
	const encoders = compact ? <CompactEncoderAssignments controller={controller} /> : undefined;
	if (!playbacks)
		return holdsEncoders
			? <ProgrammerControlSurfaceRegion screenId={screen.id} controller={controller} />
			: null;
	return (
		<PlaybackSectionSettingsHost
			surfaceId={screen.surface_id}
			onShowGrid={() => setView("playbacks")}
		>
			<LowerSectionSwitchProvider
				switchNode={holdsEncoders ? <LowerSectionSwitch view={view} onView={setView} /> : null}
			>
				{holdsEncoders && view === "encoders" ? (
					<ProgrammerControlSurfaceRegion screenId={screen.id} controller={controller} />
				) : (
					<ScreenPlaybackFit>
						<ScreenPlaybackDraftSection screen={screen} encoders={encoders} />
					</ScreenPlaybackFit>
				)}
			</LowerSectionSwitchProvider>
		</PlaybackSectionSettingsHost>
	);
}


/** An optional screen reserves an editor row only while its local Solo draft is open. */
function ScreenPlaybackDraftSection({ screen, encoders }: {
	screen: ScreenConfiguration;
	encoders?: React.ReactNode;
}) {
	const editor = usePlaybackSoloEditor(screen.surface_id);
	const grid = <ScreenPlaybackSection key="grid" screen={screen} encoders={encoders} />;
	return (
		<div style={{ display: "grid", gridTemplateRows: "auto minmax(0, 1fr)", height: "100%", minHeight: 0 }}>
			<div>{editor?.selecting && <SoloCommandEditor editor={editor} />}</div>
			{grid}
		</div>
	);
}

/** True while the screen shell must reserve room for a lower control region. */
export function screenHasControlRegion(
	screen: ScreenConfiguration,
	holdsEncoders: boolean,
) {
	return holdsEncoders || screenShowsPlaybacks(screen);
}
