import { ControlSection } from "../../components/control/ControlSection";
import { ControlSurfaceModeProvider } from "../../components/control/ControlSurfaceMode";
import { ParameterControls } from "../../components/control/ParameterControls";
import {
	resolveVisibleEncoderCount,
	useVisibleEncoderCount,
	VisibleEncoderCountProvider,
} from "../../components/control/parameterControls/VisibleEncoderCount";
import { useHardwareConnected } from "../deskSnapshot/DeskSnapshotState";
import { useApp } from "../../state/AppContext";
import { useScreens } from "./ScreensContext";

/**
 * Encoder placement decides where the encoder section lives. The main screen keeps its
 * Playback controls either way; a surface that carries a single section pins its mode
 * so the Programmer/Playback toggle disappears. A surface that does not hold the encoders
 * stays silent about the placement — Setup → Screens owns that state and its recovery.
 */
export function ProgrammerControlSurfaceRegion({
	screenId = null,
}: {
	screenId?: string | null;
}) {
	const { screens } = useScreens();
	const configuration = screens?.programmer_control_surface;
	if (!configuration) return null;
	const encoderScreenId = configuration.owner_screen_id ?? null;
	const holdsEncoders = encoderScreenId === screenId;
	if (!holdsEncoders && screenId !== null) return null;
	const screen = screenId
		? screens.screens.find((candidate) => candidate.id === screenId)
		: null;
	if (screen && !screen.show_programmer)
		return (
			<VisibleEncoderCountProvider count={configuration.visible_encoders}>
				<ScreenEncoderSection />
			</VisibleEncoderCountProvider>
		);
	return (
		<ControlSurfaceModeProvider
			mode={screenId === null ? (holdsEncoders ? null : "playbacks") : "programmer"}
			canToggle={screenId === null && holdsEncoders}
		>
			<VisibleEncoderCountProvider count={configuration.visible_encoders}>
				<ControlSection />
			</VisibleEncoderCountProvider>
		</ControlSurfaceModeProvider>
	);
}

/**
 * Encoders-only surface. A screen with the programmer switched off carries the encoder
 * group tabs with their Align, Special Dialog and Dynamics controls and the encoders —
 * no command line and no programmer tool pane beside them.
 */
function ScreenEncoderSection() {
	const { state } = useApp();
	const hardware = Boolean(useHardwareConnected() || state.midiProfile);
	const visibleEncoderCount = resolveVisibleEncoderCount(
		useVisibleEncoderCount(),
		hardware,
	);
	return (
		<section
			className="control-section screen-encoders-only"
			data-control-mode="programmer"
		>
			<VisibleEncoderCountProvider count={visibleEncoderCount}>
				<ParameterControls />
			</VisibleEncoderCountProvider>
		</section>
	);
}
