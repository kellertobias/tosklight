import { Button } from "@tosklight/ui";
import { ControlSection } from "../../components/control/ControlSection";
import { VisibleEncoderCountProvider } from "../../components/control/parameterControls/VisibleEncoderCount";
import { useScreens } from "./ScreensContext";

export function ProgrammerControlSurfaceRegion({
	screenId = null,
}: {
	screenId?: string | null;
}) {
	const { screens, updateProgrammerControlSurface } = useScreens();
	const configuration = screens?.programmer_control_surface;
	if (!configuration) return null;
	if (configuration.owner_screen_id !== screenId) {
		const owner = screens.screens.find(
			(candidate) => candidate.id === configuration.owner_screen_id,
		);
		if (!owner || owner.desired_open) return null;
		return (
			<div className="programmer-control-owner-warning" role="alert">
				<span>{`Programmer controls unavailable — assigned to ${owner.name}`}</span>
				<Button
					variant="warning"
					onClick={() =>
						void updateProgrammerControlSurface(
							screenId
								? { owner_screen_id: screenId }
								: { assign_to_main: true },
						)
					}
				>
					Use controls on this screen
				</Button>
			</div>
		);
	}
	return (
		<VisibleEncoderCountProvider count={configuration.visible_encoders}>
			<ControlSection />
		</VisibleEncoderCountProvider>
	);
}
