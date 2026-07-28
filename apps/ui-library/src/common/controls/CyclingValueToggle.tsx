import type { ReactNode } from "react";
import { Button, FormField, type LabelPlacement } from "./foundation";

export interface CyclingValueOption<T extends string> {
	value: T;
	label: string;
}

export interface CyclingValueToggleProps<T extends string> {
	ariaLabel: string;
	value: T;
	options: readonly CyclingValueOption<T>[];
	onChange(value: T): void;
	className?: string;
	disabled?: boolean;
}

export function CyclingValueToggle<T extends string>({
	ariaLabel,
	value,
	options,
	onChange,
	className = "",
	disabled = false,
}: CyclingValueToggleProps<T>) {
	const selectedIndex = Math.max(
		0,
		options.findIndex((option) => option.value === value),
	);
	const selected = options[selectedIndex];
	const next = options[(selectedIndex + 1) % options.length];
	const unavailable = disabled || options.length < 2;

	return (
		<Button
			className={`ui-cycling-value-toggle ${className}`.trim()}
			aria-label={
				selected && next
					? `${ariaLabel}: ${selected.label}. Press to select ${next.label}.`
					: ariaLabel
			}
			disabled={unavailable}
			onClick={() => {
				if (next) onChange(next.value);
			}}
		>
			{options.map((option) => (
				<span
					key={option.value}
					className={
						option.value === value
							? "ui-cycling-value-option is-active"
							: "ui-cycling-value-option"
					}
					aria-hidden="true"
				>
					{option.label}
				</span>
			))}
		</Button>
	);
}

export interface CyclingValueToggleFieldProps<T extends string>
	extends CyclingValueToggleProps<T> {
	label: ReactNode;
	description?: ReactNode;
	labelPlacement?: LabelPlacement;
}

export function CyclingValueToggleField<T extends string>({
	label,
	description,
	labelPlacement,
	...toggleProps
}: CyclingValueToggleFieldProps<T>) {
	return (
		<FormField
			label={label}
			description={description}
			labelPlacement={labelPlacement}
		>
			<CyclingValueToggle {...toggleProps} />
		</FormField>
	);
}
