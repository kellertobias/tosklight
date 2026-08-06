import { CommandSection } from "@tosklight/ui/command";
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
