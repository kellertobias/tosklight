import { type ReactNode, useState } from "react";
import { Button } from "../common/controls/foundation";
import { ModalLayer } from "../modals/ModalStack";
import { ModalNumberInput, ModalNumberValue } from "./ModalInputControls";

export interface ModalNumberEditorProps {
	ariaLabel: string;
	title: ReactNode;
	value: string;
	onChange(value: string): void;
	onSubmit(): void;
	onClose(): void;
	allowDecimal?: boolean;
	allowThrough?: boolean;
	replaceOnFirstInput?: boolean;
	dialogClassName?: string;
	beforeTitle?: ReactNode;
	bodyAside?: ReactNode;
	unit?: ReactNode;
	onRelease?(): void;
	releaseLabel?: string;
}

export function ModalNumberEditor({
	ariaLabel,
	title,
	value,
	onChange,
	onSubmit,
	onClose,
	allowDecimal = true,
	allowThrough = false,
	replaceOnFirstInput = true,
	dialogClassName = "direct-value-modal",
	beforeTitle,
	bodyAside,
	unit,
	onRelease,
	releaseLabel = "Release",
}: ModalNumberEditorProps) {
	const [caret, setCaret] = useState(value.length);
	const keypad = (
		<ModalNumberInput
			value={value}
			caret={caret}
			onChange={onChange}
			onCaretChange={setCaret}
			onEnter={onSubmit}
			onEscape={onClose}
			allowDecimal={allowDecimal}
			allowThrough={allowThrough}
			replaceOnFirstInput={replaceOnFirstInput}
		/>
	);
	return (
		<ModalLayer
			ariaLabel={ariaLabel}
			dialogClassName={dialogClassName}
			onClose={onClose}
		>
			<Button
				className="modal-close"
				aria-label={`Close ${ariaLabel}`}
				onClick={onClose}
			>
				×
			</Button>
			{beforeTitle}
			<h3>{title}</h3>
			<ModalNumberValue
				value={value}
				caret={caret}
				onCaretChange={setCaret}
				unit={unit}
			/>
			{bodyAside ? (
				<div className="direct-value-modal-body">
					{bodyAside}
					{keypad}
				</div>
			) : (
				keypad
			)}
			{onRelease && (
				<footer className="modal-actions">
					<Button variant="danger" onClick={onRelease}>
						{releaseLabel}
					</Button>
				</footer>
			)}
		</ModalLayer>
	);
}
