import { DirectProgrammerPicker } from "./DirectProgrammerPicker";
import { EncoderSurfaces } from "./EncoderSurfaces";
import { ParameterFamilyTabs } from "./ParameterFamilyTabs";
import type { ParameterController } from "./useParameterController";

export function ParameterControlView({
	controller,
}: {
	controller: ParameterController;
}) {
	let surface = <EncoderSurfaces controller={controller} />;
	if (controller.directMode && !controller.hardwareConnected)
		surface = <DirectProgrammerPicker controller={controller} />;
	return (
		<div className="parameter-controls">
			<ParameterFamilyTabs controller={controller} />
			<div className="parameter-surfaces">{surface}</div>
		</div>
	);
}
