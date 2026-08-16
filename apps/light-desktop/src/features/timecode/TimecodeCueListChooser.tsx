import { ModalRegistration, ModalTitleBar, SelectField } from "@tosklight/ui";
import type { TimecodeCueListOption } from "./TimecodeTimelineEditor";

export function CueListChooser({
	cueLists,
	value,
	onChange,
	onClose,
	onAdd,
}: {
	cueLists: readonly TimecodeCueListOption[];
	value: string;
	onChange(value: string): void;
	onClose(): void;
	onAdd(): void;
}) {
	return (
		<ModalRegistration onClose={onClose}>
			<div
				className="modal-backdrop"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<section
					className="modal-card"
					role="dialog"
					aria-modal="true"
					aria-label="Choose Cue List"
				>
					<ModalTitleBar
						title="Choose Cue List"
						onClose={onClose}
						closeLabel="Cancel adding Cue List lane"
						accept={{
							id: "add",
							label: "Add lane",
							variant: "primary",
							disabled: !value,
							onPress: onAdd,
						}}
					/>
					<SelectField
						label="Cue List"
						value={value}
						onChange={onChange}
						options={cueLists.map((cueList) => ({
							value: cueList.id,
							label: cueList.name,
						}))}
					/>
				</section>
			</div>
		</ModalRegistration>
	);
}
