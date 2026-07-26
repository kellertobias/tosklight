import { Button } from "../common/controls/foundation";
import { ModalTitleBar } from "../common/ModalTitleBar";
import { ModalLayer } from "../modals/ModalStack";

export interface UnsavedInputCloseConfirmationProps {
	ariaLabel: string;
	onDiscard(): void;
	onSave(): void;
	onStay(): void;
}

export function UnsavedInputCloseConfirmation({
	ariaLabel,
	onDiscard,
	onSave,
	onStay,
}: UnsavedInputCloseConfirmationProps) {
	return (
		<ModalLayer
			role="dialog"
			ariaLabel={ariaLabel}
			dialogClassName="ui-input-unsaved-modal"
			onClose={onStay}
		>
			<ModalTitleBar
				title="Unsaved changes"
				closeLabel="Stay in modal"
				onClose={onStay}
			/>
			<div className="ui-input-unsaved-content">
				<p>Do you want to save your changes before closing?</p>
				<div className="ui-input-unsaved-actions">
					<Button variant="danger" onClick={onDiscard}>
						Discard changes
					</Button>
					<Button variant="primary" onClick={onSave}>
						Save changes
					</Button>
					<Button autoFocus onClick={onStay}>
						Stay in modal
					</Button>
				</div>
			</div>
		</ModalLayer>
	);
}
