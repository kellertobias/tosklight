// Compact, labelled-by-column inputs for the Pixel Map tables.
//
// Every cell carries an accessible name that names the row and the column, because a table of
// "Left" fields is only readable to a screen reader when each one says whose left it is.

import {
	Button,
	NumberInput,
	SelectField,
	SwitchField,
	TextInput,
} from "@tosklight/ui/controls";
import type { KeyboardEvent, ReactNode } from "react";

/**
 * Keeps typing inside a cell from reaching the table row, which would otherwise treat Space and
 * the arrow keys as row navigation and swallow them.
 */
export function Cell({ children }: { children: ReactNode }) {
	return (
		// biome-ignore lint/a11y/noStaticElementInteractions: only stops key propagation to the row.
		<div
			className="media-pixel-cell"
			onKeyDown={(event: KeyboardEvent) => event.stopPropagation()}
		>
			{children}
		</div>
	);
}

export function TextCell({
	label,
	value,
	onChange,
}: {
	label: string;
	value: string;
	onChange: (value: string) => void;
}) {
	return (
		<Cell>
			<TextInput
				aria-label={label}
				keyboardLabel={label}
				value={value}
				onChange={(event) => onChange(event.target.value)}
			/>
		</Cell>
	);
}

export function NumberCell({
	label,
	value,
	min,
	max,
	step = 1,
	fraction = false,
	onChange,
}: {
	label: string;
	value: number;
	min?: number;
	max?: number;
	step?: number;
	/** A canvas fraction from zero to one, typed with a decimal point. */
	fraction?: boolean;
	onChange: (value: number) => void;
}) {
	return (
		<Cell>
			<NumberInput
				aria-label={label}
				keyboardLabel={label}
				min={fraction ? 0 : min}
				max={fraction ? 1 : max}
				step={fraction ? 0.01 : step}
				allowDecimal={fraction}
				showStepButtons={false}
				value={String(value)}
				onChange={(event) => {
					const next = Number(event.target.value);
					if (Number.isFinite(next)) onChange(next);
				}}
			/>
		</Cell>
	);
}

export function SelectCell<T extends string>({
	label,
	value,
	options,
	onChange,
}: {
	label: string;
	value: T;
	options: { value: T; label: string }[];
	onChange: (value: T) => void;
}) {
	return (
		<Cell>
			<SelectField
				ariaLabel={label}
				size="compact"
				value={value}
				options={options}
				onChange={onChange}
			/>
		</Cell>
	);
}

export function CheckCell({
	label,
	checked,
	onChange,
}: {
	label: string;
	checked: boolean;
	onChange: (checked: boolean) => void;
}) {
	return (
		<Cell>
			<SwitchField
				controlOnly
				label={label}
				aria-label={label}
				checked={checked}
				onChange={(event) => onChange(event.target.checked)}
			/>
		</Cell>
	);
}

export function RemoveCell({
	label,
	onRemove,
}: {
	label: string;
	onRemove: () => void;
}) {
	return (
		<Cell>
			<Button size="compact" aria-label={label} onClick={onRemove}>
				Remove
			</Button>
		</Cell>
	);
}

/** Replaces the row with the given id, leaving every other row as it was. */
export function replaceById<T extends { id: string }>(
	rows: readonly T[],
	next: T,
): T[] {
	return rows.map((row) => (row.id === next.id ? next : row));
}
