import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";

export type PickerOption = { value: string; label: string; detail?: string };

/**
 * A short list chosen from by pressing one entry.
 *
 * Table cells show their value as plain text and open this to change it, so a row stays one line
 * tall instead of carrying a dropdown in every cell.
 */
export function OptionPickerModal({
	title,
	value,
	options,
	onSelect,
	onClose,
}: {
	title: string;
	value: string;
	options: readonly PickerOption[];
	onSelect: (value: string) => void;
	onClose: () => void;
}) {
	return (
		<ModalRegistration onClose={onClose}>
			<div
				className="stacked-modal-layer fixture-option-picker-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<section
					className="nested-modal fixture-option-picker"
					role="dialog"
					aria-modal="true"
					aria-label={title}
				>
					<ModalTitleBar
						title={title}
						closeLabel={`Close ${title.toLowerCase()}`}
						onClose={onClose}
					/>
					<div className="fixture-option-picker-list" role="listbox" aria-label={title}>
						{options.map((option) => (
							<Button
								key={option.value}
								role="option"
								aria-selected={option.value === value}
								className={option.value === value ? "is-active" : undefined}
								onClick={() => {
									onSelect(option.value);
									onClose();
								}}
							>
								<span>{option.label}</span>
								{option.detail && <small>{option.detail}</small>}
							</Button>
						))}
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}
