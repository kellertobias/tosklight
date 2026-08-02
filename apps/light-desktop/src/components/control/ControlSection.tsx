import { CommandSection } from "@tosklight/ui/command";
import {
	useHardwareConnected,
	useSessionSnapshot,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { useApp } from "../../state/AppContext";
import { CommandLineBar } from "./CommandLineBar";
import { HardwareControlSummary } from "./HardwareControlSummary";
import { NumericPad } from "./NumericPad";
import { ParameterControls } from "./ParameterControls";
import { PatchParameterControls } from "./PatchParameterControls";
import { PlaybackFaderBank } from "./PlaybackFaderBank";
import { PlaybackTools } from "./PlaybackTools";

export function ControlSection() {
	const { state } = useApp();
	const hardwareConnected = useHardwareConnected();
	const session = useSessionSnapshot();
	const hardware = Boolean(hardwareConnected || state.midiProfile);
	return (
		<CommandSection
			mode={state.controlMode}
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
	);
}
