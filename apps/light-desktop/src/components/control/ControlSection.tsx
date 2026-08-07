import { CommandSection } from "@tosklight/ui/command";
import { useEncoderPlacement } from "../../features/screens/encoderPlacement";
import {
	useHardwareConnected,
	useSessionSnapshot,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { useApp } from "../../state/AppContext";
import { CommandLineBar } from "./CommandLineBar";
import { useControlSurfacePolicy } from "./ControlSurfaceMode";
import { HardwareControlSummary } from "./HardwareControlSummary";
import { NumericPad } from "./NumericPad";
import { ParameterControls } from "./ParameterControls";
import { PatchParameterControls } from "./PatchParameterControls";
import { PlaybackFaderBank } from "./PlaybackFaderBank";
import { PlaybackTools } from "./PlaybackTools";
import {
	resolveVisibleEncoderCount,
	useVisibleEncoderCount,
	VisibleEncoderCountProvider,
} from "./parameterControls/VisibleEncoderCount";

/**
 * The main screen keeps its command line, keypad and programmer tools while the encoders
 * sit on an optional screen. Only the encoder pane itself moves, and it says where it went
 * rather than leaving the operator in front of an empty half-surface.
 */
function EncodersOnAnotherScreen({ name }: { name: string }) {
	return (
		<div className="parameter-empty encoders-elsewhere" role="status">
			<b>Encoders on {name}</b>
			<small>
				Move them back in Setup → Screens → Encoder placement to control them here.
			</small>
		</div>
	);
}

export function ControlSection() {
	const { state } = useApp();
	const placement = useEncoderPlacement(null);
	const encoderScreen =
		placement && !placement.holdsEncoders ? placement.encoderScreen : null;
	const policy = useControlSurfacePolicy();
	const mode = policy?.mode ?? state.controlMode;
	const hardwareConnected = useHardwareConnected();
	const session = useSessionSnapshot();
	const hardware = Boolean(hardwareConnected || state.midiProfile);
	const configuredEncoderCount = useVisibleEncoderCount();
	const visibleEncoderCount = resolveVisibleEncoderCount(
		configuredEncoderCount,
		hardware,
	);
	return (
		<VisibleEncoderCountProvider count={visibleEncoderCount}>
			<CommandSection
				mode={mode}
				hardware={hardware}
				commandLine={<CommandLineBar />}
				programmer={
					state.builtIn === "patch" ? (
						<PatchParameterControls hardwareConnected={hardware} />
					) : encoderScreen ? (
						<EncodersOnAnotherScreen name={encoderScreen.name} />
					) : (
						<ParameterControls />
					)
				}
				playbacks={
					<PlaybackFaderBank
						playbackLayout={session?.desk.playback_layout}
						hardwareConnected={hardwareConnected}
					/>
				}
				programmerTools={<NumericPad />}
				playbackTools={<PlaybackTools />}
				hardwareTools={<HardwareControlSummary />}
			/>
		</VisibleEncoderCountProvider>
	);
}
