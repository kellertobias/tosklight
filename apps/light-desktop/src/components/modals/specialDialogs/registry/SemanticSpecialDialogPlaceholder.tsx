import { ModalFrame } from "@tosklight/ui";
import type { SemanticSpecialDialogProps } from "./specialDialogRegistry";

/**
 * Stand-in for a family's semantic Special Dialog until its family agent mounts the production
 * dialog (TL-549 `PositionDialog`, TL-550 `ColorDialogLayout`, TL-551 `FocusZoomDialog`). It
 * opens and closes like the real dialog and sends nothing.
 */
export function SemanticSpecialDialogPlaceholder({
	family,
	selectedFixtureIds,
	close,
}: SemanticSpecialDialogProps) {
	return (
		<ModalFrame
			title={family}
			ariaLabel={`${family} Special Dialog`}
			onClose={close}
			dialogClassName="semantic-special-dialog-placeholder"
		>
			<p data-testid="semantic-special-dialog-placeholder">
				{selectedFixtureIds.length} fixtures selected
			</p>
		</ModalFrame>
	);
}
