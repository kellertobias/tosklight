import { Button, NumberField } from "../../components/common";
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
				<Button
					className="modal-close"
					aria-label="Close Renumber Cues"
					onClick={close}
				>
					×
				</Button>
				<h2>Renumber Cues</h2>
				<NumberField
					label="Start Cue"
					allowDecimal
					step="1"
					value={startCue}
					onChange={(event) => setStartCue(event.target.value)}
				/>
				{renumberError && (
					<p className="ui-field-error" role="alert">
						{renumberError}
					</p>
				)}
				<div className="modal-actions">
					<Button type="button" onClick={close}>
						Cancel
					</Button>
					<Button type="submit">Renumber</Button>
				</div>
			</form>
		</div>
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
	return (
		<div className="modal-backdrop">
			<section
				className="modal-card cuelist-settings-close-confirm"
				role="dialog"
				aria-label="Unsaved Cuelist Settings"
			>
				<h2>Unsaved Cuelist Settings</h2>
				<p>
					Save the Cuelist changes, discard them, or stay in Cuelist Settings.
				</p>
				<div className="modal-actions three">
					<Button onClick={() => void controller.submit()}>Save changes</Button>
					<Button className="danger" onClick={discard}>
						Discard changes
					</Button>
					<Button onClick={() => controller.setCloseConfirm(false)}>
						Stay
					</Button>
				</div>
			</section>
		</div>
	);
}
