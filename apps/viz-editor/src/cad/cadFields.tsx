/**
 * The CAD side panels' fields, made for a keyboard and a mouse: plain inputs that take typing
 * directly, with no on-screen keyboard or number pad beside them.
 *
 * A field keeps what is being typed to itself and hands the value on only when the edit is finished
 * — Enter, or leaving the field — so a half-typed number never reaches the show. Escape puts back what
 * was there.
 */
import { Input, TextArea } from "@tosklight/ui";
import { type KeyboardEvent, useEffect, useState } from "react";

function useDraft(value: string) {
	const [draft, setDraft] = useState(value);
	useEffect(() => setDraft(value), [value]);
	return [draft, setDraft] as const;
}

export function CommitText({
	label,
	value,
	onCommit,
	ariaLabel,
	placeholder,
	accepts,
}: {
	label: string;
	value: string;
	onCommit(value: string): void;
	ariaLabel?: string;
	placeholder?: string;
	/** Whether a finished edit can be written; one that cannot is put back. */
	accepts?(draft: string): boolean;
}) {
	const [draft, setDraft] = useDraft(value);
	const commit = () => {
		if (draft === value) return;
		if (accepts && !accepts(draft)) setDraft(value);
		else onCommit(draft);
	};
	return (
		<label className="cad-field">
			<span>{label}</span>
			<Input
				type="text"
				aria-label={ariaLabel ?? label}
				placeholder={placeholder}
				value={draft}
				onChange={(event) => setDraft(event.currentTarget.value)}
				onBlur={commit}
				onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
					if (event.key === "Enter") commit();
					if (event.key === "Escape") setDraft(value);
				}}
			/>
		</label>
	);
}

/** A number typed in the units shown, refused back to its last value when it is not one. */
export function CommitNumber({
	label,
	value,
	onCommit,
	ariaLabel,
	digits = 3,
	min,
	max,
	unit,
}: {
	label: string;
	value: number;
	onCommit(value: number): void;
	ariaLabel?: string;
	digits?: number;
	/** The unit the number is typed in, shown inside the field after the value. */
	unit?: string;
	min?: number;
	max?: number;
}) {
	const shown = formatNumber(value, digits);
	const [draft, setDraft] = useDraft(shown);
	const commit = () => {
		if (draft === shown) return;
		const next = Number(draft.trim().replace(",", "."));
		if (
			draft.trim() === "" ||
			!Number.isFinite(next) ||
			(min !== undefined && next < min) ||
			(max !== undefined && next > max)
		) {
			setDraft(shown);
			return;
		}
		onCommit(next);
	};
	return (
		<label className={`cad-field ${unit ? "has-unit" : ""}`.trim()}>
			<span>{label}</span>
			<span className="cad-field-control">
			<Input
				type="text"
				inputMode="decimal"
				aria-label={ariaLabel ?? label}
				value={draft}
				onChange={(event) => setDraft(event.currentTarget.value)}
				onBlur={commit}
				onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
					if (event.key === "Enter") commit();
					if (event.key === "Escape") setDraft(shown);
				}}
			/>
			{unit ? (
				<span className="cad-field-unit" aria-hidden="true">
					{unit}
				</span>
			) : null}
			</span>
		</label>
	);
}

export function CommitTextArea({
	label,
	value,
	onCommit,
}: {
	label: string;
	value: string;
	onCommit(value: string): void;
}) {
	const [draft, setDraft] = useDraft(value);
	return (
		<label className="cad-field">
			<span>{label}</span>
			<TextArea
				aria-label={label}
				rows={3}
				value={draft}
				onChange={(event) => setDraft(event.currentTarget.value)}
				onBlur={() => {
					if (draft !== value) onCommit(draft);
				}}
				onKeyDown={(event) => {
					if (event.key === "Escape") setDraft(value);
				}}
			/>
		</label>
	);
}

/** A value as a field shows it: rounded to `digits` places, with no trailing zeros. */
export function formatNumber(value: number, digits: number): string {
	if (!Number.isFinite(value)) return "";
	return String(Number(value.toFixed(digits)));
}
