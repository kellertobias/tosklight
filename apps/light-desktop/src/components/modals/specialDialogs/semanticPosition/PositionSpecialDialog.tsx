import { PositionDialog } from "../intention/PositionDialog";
import type { SemanticSpecialDialogProps } from "../registry/specialDialogRegistry";
import { usePositionSpecialDialog } from "./usePositionSpecialDialog";

/**
 * The production Position Special Dialog under the semantic programming contract (TL-549).
 *
 * Modal only: the unwrapped multi-turn Pan circle with −90°/Reset/+90° above the Tilt fader on
 * the left and the square rate joystick with Return Home on the right. Point and X/Y/Z stay on
 * Position encoder page 2. Opening, focusing and closing the dialog send nothing; Return Home sends
 * the home Angles (Pan 0°, Tilt 0°) as one gesture.
 */
export function PositionSpecialDialog({
	selectedFixtureIds,
	close,
}: SemanticSpecialDialogProps) {
	const props = usePositionSpecialDialog(selectedFixtureIds);
	return <PositionDialog {...props} onClose={close} />;
}
