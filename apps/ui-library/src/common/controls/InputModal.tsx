import { type ReactNode, useState } from "react";
import {
	ModalCaretValue,
	ModalNumberInput,
	ModalNumberValue,
	ModalTextKeyboard,
} from "../../input/ModalInputControls";
import { UnsavedInputCloseConfirmation } from "../../input/UnsavedInputCloseConfirmation";
import { ModalLayer } from "../../modals/ModalStack";
import { ModalTitleBar } from "../ModalTitleBar";
import { Button } from "./foundation";

export interface InputModalProps {
	kind: "text" | "multiline" | "number";
	value: string;
	initialCaret?: number;
	allowDecimal?: boolean;
	leadingIcon?: ReactNode;
	secure?: boolean;
	label?: string;
	placeholder?: string;
	unit?: ReactNode;
	error?: ReactNode;
	onCommit: (value: string) => void;
	onDraftChange?: (value: string) => void;
	onCancel: () => void;
}

function ValuePreview({
	caret,
	kind,
	label,
	leadingIcon,
	onCaretChange,
	placeholder,
	secure,
	unit,
	value,
}: Pick<
	InputModalProps,
	"kind" | "label" | "leadingIcon" | "placeholder" | "secure" | "unit" | "value"
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
	const valuePreview = (
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
	if (!leadingIcon) return valuePreview;
	return (
		<div className="modal-value-with-leading-icon">
			<span className="modal-value-leading-icon" aria-hidden="true">
				{leadingIcon}
			</span>
			{valuePreview}
		</div>
	);
}

export function InputModal({
	kind,
	value,
	initialCaret,
	allowDecimal = false,
	secure = false,
	label,
	leadingIcon,
	placeholder,
	unit,
	error,
	onCommit,
	onDraftChange,
	onCancel,
}: InputModalProps) {
	const [initialValue] = useState(value);
	const [draft, setDraft] = useState(value);
	const [caret, setCaret] = useState(() =>
		Math.max(0, Math.min(initialCaret ?? value.length, value.length)),
	);
	const [confirmClose, setConfirmClose] = useState(false);
	const title = label ?? (kind === "number" ? "Number input" : "Text input");
	const hasUnsavedChanges = kind === "multiline" && draft !== initialValue;
	const updateDraft = (next: string) => {
		setDraft(next);
		onDraftChange?.(next);
	};
	const cancel = () => {
		if (hasUnsavedChanges) onDraftChange?.(initialValue);
		setConfirmClose(false);
		onCancel();
	};
	const requestClose = () => {
		if (hasUnsavedChanges) {
			setConfirmClose(true);
			return;
		}
		onCancel();
	};
	const commit = () => {
		setConfirmClose(false);
		onCommit(draft);
	};
	return (
		<>
			<ModalLayer
				ariaLabel={title}
				className="ui-input-modal-layer"
				dialogClassName={
					kind !== "number" ? "keyboard-modal" : "number-field-modal"
				}
				onClose={requestClose}
			>
				<ModalTitleBar
					title={title}
					closeLabel="Close input"
					onClose={requestClose}
					accept={
						kind === "multiline"
							? {
									id: "done",
									label: "Done",
									variant: "primary",
									className: "ui-input-modal-done",
									onPress: commit,
								}
							: undefined
					}
				/>
				<ValuePreview
					caret={caret}
					kind={kind}
					label={label}
					leadingIcon={leadingIcon}
					placeholder={placeholder}
					secure={secure}
					unit={unit}
					value={draft}
					onCaretChange={setCaret}
				/>
				{error && (
					<p className="ui-field-error" role="alert">
						{error}
					</p>
				)}
				{kind !== "number" ? (
					<ModalTextKeyboard
						value={draft}
						caret={caret}
						onChange={updateDraft}
						onEnter={commit}
						onEscape={requestClose}
						onCaretChange={setCaret}
						multiline={kind === "multiline"}
					/>
				) : (
					<ModalNumberInput
						value={draft}
						caret={caret}
						allowDecimal={allowDecimal}
						onChange={updateDraft}
						onEnter={commit}
						onEscape={requestClose}
						onCaretChange={setCaret}
					/>
				)}
			</ModalLayer>
			{confirmClose && (
				<UnsavedInputCloseConfirmation
					ariaLabel="Unsaved multiline text changes"
					onDiscard={cancel}
					onSave={commit}
					onStay={() => setConfirmClose(false)}
				/>
			)}
		</>
	);
}
