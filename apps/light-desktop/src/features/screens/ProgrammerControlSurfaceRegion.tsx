import { CommandLineBar } from "../../components/control/CommandLineBar";
import { ControlSection } from "../../components/control/ControlSection";
import { ControlSurfaceModeProvider } from "../../components/control/ControlSurfaceMode";
import { ParameterControls } from "../../components/control/ParameterControls";
import {
	resolveVisibleEncoderCount,
	useVisibleEncoderCount,
	VisibleEncoderCountProvider,
} from "../../components/control/parameterControls/VisibleEncoderCount";
import { useApp } from "../../state/AppContext";
import { useHardwareConnected } from "../deskSnapshot/DeskSnapshotState";
import {
	LowerSectionSwitchProvider,
	useLowerSectionSwitch,
} from "./LowerSectionSwitch";
import { useScreens } from "./ScreensContext";

/**
 * Encoder placement decides where the encoder section lives. The main screen keeps its
 * whole programmer either way and loses only the encoders; an optional screen carries the
 * encoders alone, with the command line where that screen enables it. A surface that does
 * not hold the encoders stays silent about the placement — Setup → Screens owns that state
 * and its recovery.
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
	if (screenId !== null)
		return (
			<VisibleEncoderCountProvider count={configuration.visible_encoders}>
				<ScreenEncoderSection
					showCommandLine={Boolean(screen?.show_programmer)}
				/>
			</VisibleEncoderCountProvider>
		);
	/*
	 * The main screen keeps its whole programmer — command line, keypad and the
	 * Delete/Move tools — wherever the encoders sit. Moving them to an optional screen
	 * takes the encoders off this surface and nothing else, so the Programmer/Playback
	 * toggle stays available either way.
	 */
	return (
		<ControlSurfaceModeProvider mode={null} canToggle>
			<VisibleEncoderCountProvider count={configuration.visible_encoders}>
				<ControlSection />
			</VisibleEncoderCountProvider>
		</ControlSurfaceModeProvider>
	);
}

/**
 * Encoder surface of an optional screen. It carries the encoder group tabs with their
 * Align, Special Dialog and Dynamics controls and the encoders — never the keypad, the
 * programmer fader or the Delete/Move tools, which stay on the main screen. The command
 * line joins it only when the screen is configured for it, and then it also takes the
 * Playback/Encoders switch into its top right instead of the Dynamics row.
 */
function ScreenEncoderSection({
	showCommandLine,
}: {
	showCommandLine: boolean;
}) {
	const { state } = useApp();
	const hardware = Boolean(useHardwareConnected() || state.midiProfile);
	const visibleEncoderCount = resolveVisibleEncoderCount(
		useVisibleEncoderCount(),
		hardware,
	);
	const sectionSwitch = useLowerSectionSwitch();
	const encoders = (
		<VisibleEncoderCountProvider count={visibleEncoderCount}>
			<ParameterControls />
		</VisibleEncoderCountProvider>
	);
	return (
		<section
			className={`control-section screen-encoders-only ${showCommandLine ? "with-command-line" : ""}`}
			data-control-mode="programmer"
		>
			{showCommandLine ? (
				<>
					<div className="screen-command-row">
						<CommandLineBar />
						{sectionSwitch}
					</div>
					{/* The tab row must not repeat the switch the command row already shows. */}
					<LowerSectionSwitchProvider switchNode={null}>
						{encoders}
					</LowerSectionSwitchProvider>
				</>
			) : (
				encoders
			)}
		</section>
	);
}
