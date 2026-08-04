import { ControlSection } from "../../components/control/ControlSection";
import { VisibleEncoderCountProvider } from "../../components/control/parameterControls/VisibleEncoderCount";
import { useScreens } from "./ScreensContext";

export function ProgrammerControlSurfaceRegion({
	screenId = null,
}: {
	screenId?: string | null;
}) {
	const { screens } = useScreens();
	const configuration = screens?.programmer_control_surface;
	if (!configuration || configuration.owner_screen_id !== screenId) return null;
	return (
		<VisibleEncoderCountProvider count={configuration.visible_encoders}>
			<ControlSection />
		</VisibleEncoderCountProvider>
	);
}
