import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useCueTransfer } from "../../features/cueTransfer/CueTransferProvider";
import { useProgrammingPendingCommandChoiceView } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { Button, ModalTitleBar } from "../common";
import { useCommandLineSurface } from "../control/commandLine/useCommandLineSurface";

export function CommandChoiceModal() {
	const commandLine = useCommandLineSurface({ observeCommand: false });
	const transfer = useCueTransfer();
	const choice = useProgrammingPendingCommandChoiceView();
	const [executing, setExecuting] = useState<string | null>(null);
	useEffect(() => {
		setExecuting(null);
	}, [choice]);
	if (!choice) return null;

	const select = async (mode: "plain" | "status") => {
		setExecuting(mode);
		const succeeded = await transfer?.apply(choice, mode);
		if (!succeeded) setExecuting(null);
	};

	const operation = choice.operation === "copy" ? "Copy" : "Move";
	const cancelLabel = choice.cancelLabel;
	const cancel = () => {
		void commandLine.cancelChoice();
	};
	return createPortal(
		<div className="stacked-modal-layer command-choice-layer">
			<section
				className="nested-modal command-choice-modal"
				role="dialog"
				aria-modal="true"
				aria-label={`Cue ${operation} choice`}
			>
				<ModalTitleBar title={`Cue ${operation}`} />
				<p>
					Choose whether to transfer only the stored Cue delta or its complete
					tracked status.
				</p>
				<div className="command-choice-actions">
					{choice.options.map((option) => (
						<Button
							key={option.id}
							variant="primary"
							loading={executing === option.id}
							disabled={executing !== null}
							onClick={() => void select(option.id)}
						>
							{option.label}
						</Button>
					))}
					<Button disabled={executing !== null} onClick={cancel}>
						{cancelLabel}
					</Button>
				</div>
			</section>
		</div>,
		document.body,
	);
}
