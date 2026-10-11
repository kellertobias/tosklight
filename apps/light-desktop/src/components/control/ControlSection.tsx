import { CommandSection } from "@tosklight/ui/command";
import { ScreenPlaybackFit } from "../../features/screens/ScreenPlaybackFit";
import { useEncoderPlacement } from "../../features/screens/encoderPlacement";
import {
	useHardwareConnected,
	useSessionSnapshot,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { useApp } from "../../state/AppContext";
import { PlaybackSectionSettingsHost } from "./PlaybackSectionSettingsHost";
import { CommandLineBar } from "./CommandLineBar";
import { useControlSurfacePolicy } from "./ControlSurfaceMode";
import { HardwareControlSummary } from "./HardwareControlSummary";
import { NumericPad } from "./NumericPad";
import { ParameterControls, useProgrammerEncoderContext } from "./ParameterControls";
import { CompactEncoderAssignments } from "./parameterControls/CompactEncoderAssignments";
import { useParameterController } from "./parameterControls/useParameterController";
import { programmerEncoderOwnerContext } from "./parameterControls/useEncoderNavigation";
import { PatchParameterControls } from "./PatchParameterControls";
import { PlaybackFaderBank } from "./PlaybackFaderBank";
import { PlaybackTools } from "./PlaybackTools";
import {
	resolveVisibleEncoderCount,
	useVisibleEncoderCount,
	VisibleEncoderCountProvider,
} from "./parameterControls/VisibleEncoderCount";

export function ControlSection() {
	const { state, dispatch } = useApp();
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
	const programmerContext = useProgrammerEncoderContext();
	const compactEnabled = hardware && Boolean(session?.desk.show_compact_encoders);
	const controller = useParameterController(
		encoderScreen ? compactEnabled : programmerContext && (mode === "programmer" || compactEnabled),
		{ bindHardware: !encoderScreen, ownerContext: programmerEncoderOwnerContext(programmerContext, state.builtIn) },
	);
	const compact = compactEnabled &&
		(mode === "playbacks" || Boolean(encoderScreen));
	const compactEncoders = compact ? <CompactEncoderAssignments controller={controller} /> : undefined;
	const playbacks = (
		<ScreenPlaybackFit>
			<PlaybackFaderBank
				surfaceId={session?.desk.surface_id ?? "main"}
				hardwareConnected={hardwareConnected}
			/>
		</ScreenPlaybackFit>
	);
	/*
	 * With the encoders on another screen this surface has no programmer pane to show, so
	 * the Programmer/Playback toggle only swaps the right pane between the keypad and the
	 * Playback tools. The left pane stays on the Playbacks in both modes rather than
	 * leaving the operator in front of an empty half-surface.
	 */
	return (
		<VisibleEncoderCountProvider count={visibleEncoderCount}>
			<PlaybackSectionSettingsHost surfaceId={session?.desk.surface_id ?? "main"}
				onShowGrid={() => dispatch({ type: "SET_CONTROL_MODE", value: "playbacks" })}>
			<CommandSection
				className="screen-bank-fit"
				mode={mode}
				hardware={hardware}
				commandLine={<CommandLineBar />}
				programmer={
					state.builtIn === "patch" ? (
						<PatchParameterControls hardwareConnected={hardware} />
					) : encoderScreen ? (
						playbacks
					) : (
						<ParameterControls controller={controller} />
					)
				}
				playbacks={playbacks}
				programmerTools={<NumericPad />}
				playbackTools={<PlaybackTools />}
				hardwareTools={<HardwareControlSummary encoders={compactEncoders} />}
			/>
			</PlaybackSectionSettingsHost>
		</VisibleEncoderCountProvider>
	);
}
