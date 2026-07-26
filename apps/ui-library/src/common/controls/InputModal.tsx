import { type ReactNode, useState } from "react";
import {
	ModalCaretValue,
	ModalNumberInput,
	ModalNumberValue,
	ModalTextKeyboard,
} from "../../input/ModalInputControls";
import { ModalLayer } from "../../modals/ModalStack";
import { ModalTitleBar } from "../ModalTitleBar";
import { Button } from "./foundation";

export interface InputModalProps {
	kind: "text" | "multiline" | "number";
	value: string;
	allowDecimal?: boolean;
	secure?: boolean;
	label?: string;
	placeholder?: string;
	unit?: ReactNode;
	onCommit: (value: string) => void;
	onDraftChange?: (value: string) => void;
	onCancel: () => void;
}

function ValuePreview({
	caret,
	kind,
	label,
	onCaretChange,
	placeholder,
	secure,
	unit,
	value,
}: Pick<
	InputModalProps,
	"kind" | "label" | "placeholder" | "secure" | "unit" | "value"
> & {
	caret: number;
	onCaretChange: (position: number) => void;
}) {
	if (kind === "number") {
		return (
			<ModalNumberValue
				value={value}
				caret={caret}
				placeholder={placeholder}
				ariaLabel={`${label ?? kind} value`}
				onCaretChange={onCaretChange}
				unit={unit}
			/>
		);
	}
	return (
		<ModalCaretValue
			value={value}
			caret={caret}
			placeholder={placeholder}
			ariaLabel={`${label ?? kind} value`}
			multiline={kind === "multiline"}
			secure={secure}
			onCaretChange={onCaretChange}
		/>
	);
}

export function InputModal({
	kind,
	value,
	allowDecimal = false,
	secure = false,
	label,
	placeholder,
	unit,
	onCommit,
	onDraftChange,
	onCancel,
}: InputModalProps) {
	const [draft, setDraft] = useState(value);
	const [caret, setCaret] = useState(value.length);
	const title = label ?? (kind === "number" ? "Number input" : "Text input");
	const updateDraft = (next: string) => {
		setDraft(next);
		onDraftChange?.(next);
	};
	return (
		<ModalLayer
			ariaLabel={title}
			className="ui-input-modal-layer"
			dialogClassName={
				kind !== "number" ? "keyboard-modal" : "number-field-modal"
			}
			onClose={onCancel}
		>
			<ModalTitleBar
				title={title}
				closeLabel="Close input"
				onClose={onCancel}
				actions={
					kind === "multiline" ? (
						<Button variant="primary" onClick={() => onCommit(draft)}>
							Done
						</Button>
					) : undefined
				}
			/>
			<ValuePreview
				caret={caret}
				kind={kind}
				label={label}
				placeholder={placeholder}
				secure={secure}
				unit={unit}
				value={draft}
				onCaretChange={setCaret}
			/>
			{kind !== "number" ? (
				<ModalTextKeyboard
					value={draft}
					caret={caret}
					onChange={updateDraft}
					onEnter={() => onCommit(draft)}
					onEscape={onCancel}
					onCaretChange={setCaret}
					multiline={kind === "multiline"}
				/>
			) : (
				<ModalNumberInput
					value={draft}
					caret={caret}
					allowDecimal={allowDecimal}
					onChange={updateDraft}
					onEnter={() => onCommit(draft)}
					onEscape={onCancel}
					onCaretChange={setCaret}
				/>
			)}
		</ModalLayer>
	);
}
