import { EncoderSurfaces } from "./EncoderSurfaces";
import { ParameterFamilyTabs } from "./ParameterFamilyTabs";
import type { ParameterController } from "./useParameterController";

export function ParameterControlView({
	controller,
}: {
	controller: ParameterController;
}) {
	return (
		<div className="parameter-controls">
			<ParameterFamilyTabs controller={controller} />
			<div className="parameter-surfaces">
				<EncoderSurfaces controller={controller} />
			</div>
		</div>
	);
}
