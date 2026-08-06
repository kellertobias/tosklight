import { Button } from "@tosklight/ui";
import { ControlSection } from "../../components/control/ControlSection";
import { ControlSurfaceModeProvider } from "../../components/control/ControlSurfaceMode";
import { VisibleEncoderCountProvider } from "../../components/control/parameterControls/VisibleEncoderCount";
import { useScreens } from "./ScreensContext";

/**
 * Encoder placement decides where the encoder section lives. The main screen keeps its
 * Playback controls either way; a surface that carries a single section pins its mode
 * so the Programmer/Playback toggle disappears.
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
	const encoderScreen = screens.screens.find(
		(candidate) => candidate.id === encoderScreenId,
	);
	if (!holdsEncoders && screenId !== null) {
		if (!encoderScreen || encoderScreen.desired_open) return null;
		return <EncoderRecovery name={encoderScreen.name} screenId={screenId} />;
	}
	const unreachable =
		!holdsEncoders && Boolean(encoderScreen) && !encoderScreen?.desired_open;
	return (
		<>
			{unreachable && (
				<EncoderRecovery name={encoderScreen?.name ?? ""} screenId={screenId} />
			)}
			<ControlSurfaceModeProvider
				mode={screenId === null ? (holdsEncoders ? null : "playbacks") : "programmer"}
				canToggle={screenId === null && holdsEncoders}
			>
				<VisibleEncoderCountProvider count={configuration.visible_encoders}>
					<ControlSection />
				</VisibleEncoderCountProvider>
			</ControlSurfaceModeProvider>
		</>
	);
}

/** A closed encoder screen would strand the encoders, so every surface can take them back. */
function EncoderRecovery({
	name,
	screenId,
}: {
	name: string;
	screenId: string | null;
}) {
	const { updateProgrammerControlSurface } = useScreens();
	return (
		<div className="programmer-control-owner-warning" role="alert">
			<span>{`Encoders unavailable — assigned to ${name}`}</span>
			<Button
				variant="warning"
				onClick={() =>
					void updateProgrammerControlSurface(
						screenId ? { owner_screen_id: screenId } : { assign_to_main: true },
					)
				}
			>
				Use encoders on this screen
			</Button>
		</div>
	);
}
