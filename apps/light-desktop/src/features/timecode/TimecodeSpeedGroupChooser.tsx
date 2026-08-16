import { ModalRegistration, ModalTitleBar, SelectField } from "@tosklight/ui";

export const TIMECODE_SPEED_GROUPS = ["A", "B", "C", "D", "E"] as const;

export function TimecodeSpeedGroupChooser({
	available,
	value,
	onChange,
	onClose,
	onAdd,
}: {
	available: readonly string[];
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
					aria-label="Choose Speed Group"
				>
					<ModalTitleBar
						title="Choose Speed Group"
						onClose={onClose}
						closeLabel="Cancel adding Speed Group lane"
						accept={{
							id: "add",
							label: "Add lane",
							variant: "primary",
							disabled: !value || !available.includes(value),
							onPress: onAdd,
						}}
					/>
					<SelectField
						label="Speed Group"
						value={value}
						onChange={onChange}
						options={available.map((group) => ({
							value: group,
							label: `Speed Group ${group}`,
						}))}
					/>
				</section>
			</div>
		</ModalRegistration>
	);
}
