import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { createPortal } from "react-dom";
import type { PlaybackBankController } from "./controller";

export function CueRecordChoiceModal({
	controller,
}: {
	controller: PlaybackBankController;
}) {
	const choice = controller.cueRecordChoice;
	if (!choice) return null;
	const close = () => controller.resolveCueRecordChoice(null);
	return createPortal(
		<ModalRegistration onClose={close}>
			<div className="stacked-modal-layer cue-record-choice-layer">
				<section
					className="nested-modal cue-record-choice-modal"
					role="dialog"
					aria-modal="true"
					aria-label="Record Cue choice"
				>
					<ModalTitleBar title={`Record Cue ${choice.cueNumber}`} />
					<p>This Cuelist contains one Cue. Choose how to record it.</p>
					<div className="command-choice-actions">
						<Button
							variant="primary"
							onClick={() => controller.resolveCueRecordChoice("add")}
						>
							Add Cue
						</Button>
						<Button onClick={() => controller.resolveCueRecordChoice("merge")}>
							Merge Cue
						</Button>
						<Button onClick={() => controller.resolveCueRecordChoice("overwrite")}>
							Overwrite Cue
						</Button>
						<Button onClick={close}>Cancel</Button>
					</div>
				</section>
			</div>
		</ModalRegistration>,
		document.body,
	);
}
