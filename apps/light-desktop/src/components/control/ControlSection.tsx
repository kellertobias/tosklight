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
	const playbacks = (
		<PlaybackFaderBank
			playbackLayout={session?.desk.playback_layout}
			hardwareConnected={hardwareConnected}
		/>
	);
	/*
	 * With the encoders on another screen this surface has no programmer pane to show, so
	 * the Programmer/Playback toggle only swaps the right pane between the keypad and the
	 * Playback tools. The left pane stays on the Playbacks in both modes rather than
	 * leaving the operator in front of an empty half-surface.
	 */
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
						playbacks
					) : (
						<ParameterControls />
					)
				}
				playbacks={playbacks}
				programmerTools={<NumericPad />}
				playbackTools={<PlaybackTools />}
				hardwareTools={<HardwareControlSummary />}
			/>
		</VisibleEncoderCountProvider>
	);
}
