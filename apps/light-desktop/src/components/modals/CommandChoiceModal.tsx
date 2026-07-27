import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useCueTransfer } from "../../features/cueTransfer/CueTransferProvider";
import { useProgrammingPendingCommandChoiceView } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
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

	const selectCue = async (mode: "plain" | "status") => {
		if (choice.type !== "cue_move_copy") return;
		setExecuting(mode);
		const succeeded = await transfer?.apply(choice, mode);
		if (!succeeded) setExecuting(null);
	};
	const selectDynamic = async (controllerId: string, command: string) => {
		if (choice.type !== "dynamic_instance") return;
		setExecuting(controllerId);
		const succeeded = await commandLine.execute(command);
		if (!succeeded) setExecuting(null);
	};

	const cancelLabel = choice.cancelLabel;
	const cancel = () => {
		void commandLine.cancelChoice();
	};
	return createPortal(
		<ModalRegistration onClose={cancel}>
			<div className="stacked-modal-layer command-choice-layer">
			<section
				className="nested-modal command-choice-modal"
				role="dialog"
				aria-modal="true"
				aria-label={
					choice.type === "cue_move_copy"
						? `Cue ${choice.operation === "copy" ? "Copy" : "Move"} choice`
						: `Dynamic ${choice.poolNumber} instance choice`
				}
			>
				<ModalTitleBar
					title={
						choice.type === "cue_move_copy"
							? `Cue ${choice.operation === "copy" ? "Copy" : "Move"}`
							: `Dynamic ${choice.poolNumber} · Choose Instance`
					}
				/>
				<p>
					{choice.type === "cue_move_copy"
						? "Choose whether to transfer only the stored Cue delta or its complete tracked status."
						: "More than one running targetless instance matches this command. Choose the exact instance to control."}
				</p>
				<div className="command-choice-actions">
					{choice.type === "cue_move_copy"
						? choice.options.map((option) => (
								<Button
									key={option.id}
									variant="primary"
									loading={executing === option.id}
									disabled={executing !== null}
									onClick={() => void selectCue(option.id)}
								>
									{option.label}
								</Button>
							))
						: choice.options.map((option) => (
								<Button
									key={option.controllerId}
									variant="primary"
									loading={executing === option.controllerId}
									disabled={executing !== null}
									onClick={() =>
										void selectDynamic(option.controllerId, option.command)
									}
								>
									{option.label}
								</Button>
							))}
					<Button disabled={executing !== null} onClick={cancel}>
						{cancelLabel}
					</Button>
				</div>
			</section>
			</div>
		</ModalRegistration>,
		document.body,
	);
}
