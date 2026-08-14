import {
	Button,
	ModalRegistration,
	ModalTitleBar,
	NumberField,
} from "@tosklight/ui";
import type { CuelistSettingsController } from "./useCuelistSettings";

export function RenumberCuesDialog({
	controller,
}: {
	controller: CuelistSettingsController;
}) {
	const {
		renumberOpen,
		setRenumberOpen,
		setRenumberError,
		startCue,
		setStartCue,
		renumberError,
		renumber,
	} = controller;
	if (!renumberOpen) return null;
	const close = () => {
		setRenumberOpen(false);
		setRenumberError("");
	};
	return (
		<ModalRegistration onClose={close}>
			<div
				className="modal-backdrop"
				onPointerDown={(event) => {
					if (event.target === event.currentTarget) close();
				}}
			>
				<form
					className="modal-card"
					role="dialog"
					aria-modal="true"
					aria-label="Renumber Cues"
					onSubmit={(event) => {
						event.preventDefault();
						void renumber();
					}}
				>
					<ModalTitleBar
						title="Renumber Cues"
						accept={{
							id: "renumber",
							label: "Renumber",
							variant: "primary",
							type: "submit",
						}}
						onClose={close}
						closeLabel="Cancel renumbering"
					/>
					<NumberField
						label="First new Cue number"
						description="The first Cue receives this number; every later Cue follows in its existing order."
						step="1"
						value={startCue}
						onChange={(event) => setStartCue(event.target.value)}
					/>
					{renumberError && (
						<p className="ui-field-error" role="alert">
							{renumberError}
						</p>
					)}
				</form>
			</div>
		</ModalRegistration>
	);
}

export function UnsavedSettingsDialog({
	controller,
	discard,
}: {
	controller: CuelistSettingsController;
	discard: () => void;
}) {
	if (!controller.closeConfirm) return null;
	const stay = () => controller.setCloseConfirm(false);
	return (
		<ModalRegistration onClose={stay}>
			<div className="modal-backdrop">
				<section
					className="modal-card cuelist-settings-close-confirm"
					role="dialog"
					aria-label="Unsaved Cuelist Settings"
				>
					<ModalTitleBar title="Unsaved Cuelist Settings" onClose={stay} />
					<p>
						Save the Cuelist changes, discard them, or stay in Cuelist Settings.
					</p>
					<div className="modal-actions three">
						<Button onClick={() => void controller.submit()}>
							Save changes
						</Button>
						<Button className="danger" onClick={discard}>
							Discard changes
						</Button>
						<Button onClick={stay}>Stay</Button>
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}
