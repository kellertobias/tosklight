import {
	type Dispatch,
	forwardRef,
	type MouseEvent as ReactMouseEvent,
	type SetStateAction,
	useCallback,
	useImperativeHandle,
	useState,
} from "react";
import { Button } from "../common";
import {
	ModalNumberEditor,
	type ModalNumberPresetConfig,
} from "../input/ModalNumberEditor";
import { submitNumericExpression } from "../input/numericExpression";

export type HardwareEncoderTarget = {
	label: string;
	value: string;
	role?: string;
};

export interface HardwareEncoderDisplayHandle {
	activate(): void;
}

export interface HardwareEncoderDisplayProps {
	slot: number;
	target?: HardwareEncoderTarget;
	secondary?: HardwareEncoderTarget;
	editValue?: number;
	secondaryEditValue?: number;
	onEdit?: (value: number) => void;
	onSecondaryEdit?: (value: number) => void;
	onEditRange?: (points: number[]) => void;
	onSecondaryEditRange?: (points: number[]) => void;
	canRelease?: boolean;
	presets?: ModalNumberPresetConfig;
	onPresetSelect?: (value: string) => void;
	onRelease?: () => void;
}

type EncoderEditorState = {
	target: "primary" | "secondary";
	selectable: boolean;
};

type EncoderInputValues = {
	primary: string;
	secondary: string;
};

function HardwareEncoderContent({
	slot,
	target,
	secondary,
}: {
	slot: number;
	target: HardwareEncoderTarget;
	secondary?: HardwareEncoderTarget;
}) {
	return (
		<>
			<header className="hardware-encoder-primary-labels">
				<b title={target.label}>{target.label}</b>
				<small>Enc {slot}</small>
			</header>
			<div className="hardware-encoder-target hardware-encoder-primary">
				<strong>{target.value}</strong>
			</div>
			{secondary && (
				<>
					<div className="hardware-encoder-divider" aria-hidden="true" />
					<div className="hardware-encoder-target hardware-encoder-secondary">
						<strong>{secondary.value}</strong>
					</div>
					<footer className="hardware-encoder-secondary-labels">
						<b title={secondary.label}>{secondary.label}</b>
						<small>Push-turn</small>
					</footer>
				</>
			)}
		</>
	);
}

function HardwareEncoderEditor({
	slot,
	target,
	secondary,
	editor,
	inputValues,
	setInputValues,
	setEditor,
	submit,
	onEditRange,
	onSecondaryEditRange,
	canRelease,
	presets,
	onPresetSelect,
	onRelease,
}: {
	slot: number;
	target: HardwareEncoderTarget;
	secondary?: HardwareEncoderTarget;
	editor: EncoderEditorState;
	inputValues: EncoderInputValues;
	setInputValues: Dispatch<SetStateAction<EncoderInputValues>>;
	setEditor: Dispatch<SetStateAction<EncoderEditorState | null>>;
	submit: (candidate?: string) => void;
	onEditRange?: (points: number[]) => void;
	onSecondaryEditRange?: (points: number[]) => void;
	canRelease: boolean;
	presets?: ModalNumberPresetConfig;
	onPresetSelect?: (value: string) => void;
	onRelease?: () => void;
}) {
	const selectedTarget = editor.target === "secondary" ? secondary : target;
	if (!selectedTarget) return null;
	return (
		<ModalNumberEditor
			key={`${editor.target}-${editor.selectable ? "selectable" : "direct"}`}
			ariaLabel={`Encoder ${slot} value`}
			dialogClassName="direct-value-modal hardware-encoder-modal"
			title={selectedTarget.label}
			value={inputValues[editor.target]}
			onChange={(value) =>
				setInputValues((current) => ({
					...current,
					[editor.target]: value,
				}))
			}
			onSubmit={submit}
			onClose={() => setEditor(null)}
			allowThrough={Boolean(
				editor.target === "primary" ? onEditRange : onSecondaryEditRange,
			)}
			presets={presets}
			onPresetSelect={onPresetSelect}
			beforeTitle={
				editor.selectable && secondary ? (
					<div className="hardware-encoder-target-selector">
						<Button
							aria-pressed={editor.target === "primary"}
							onClick={() => setEditor({ target: "primary", selectable: true })}
						>
							{target.label}
						</Button>
						<Button
							aria-pressed={editor.target === "secondary"}
							onClick={() =>
								setEditor({ target: "secondary", selectable: true })
							}
						>
							{secondary.label}
						</Button>
					</div>
				) : undefined
			}
			onRelease={
				editor.target === "primary" && canRelease && onRelease
					? () => {
							onRelease();
							setEditor(null);
						}
					: undefined
			}
			releaseLabel={`Release ${selectedTarget.label}`}
		/>
	);
}

export const HardwareEncoderDisplayView = forwardRef<
	HardwareEncoderDisplayHandle,
	HardwareEncoderDisplayProps
>(function HardwareEncoderDisplayView(
	{
		slot,
		target,
		secondary,
		editValue,
		secondaryEditValue,
		onEdit,
		onSecondaryEdit,
		onEditRange,
		onSecondaryEditRange,
		canRelease = false,
		presets,
		onPresetSelect,
		onRelease,
	},
	ref,
) {
	const [editor, setEditor] = useState<EncoderEditorState | null>(null);
	const [inputValues, setInputValues] = useState<EncoderInputValues>({
		primary: "",
		secondary: "",
	});
	const openEditor = useCallback(
		(target: "primary" | "secondary", selectable: boolean) => {
			setInputValues({
				primary: String(Number((editValue ?? 0).toFixed(1))),
				secondary: String(Number((secondaryEditValue ?? 0).toFixed(1))),
			});
			setEditor({ target, selectable });
		},
		[editValue, secondaryEditValue],
	);
	const activate = useCallback(() => {
		if (secondary && onSecondaryEdit)
			openEditor(onEdit ? "primary" : "secondary", Boolean(onEdit));
		else if (onEdit) openEditor("primary", false);
	}, [onEdit, onSecondaryEdit, openEditor, secondary]);
	useImperativeHandle(ref, () => ({ activate }), [activate]);
	const submit = (candidate = editor ? inputValues[editor.target] : "") => {
		if (!editor) return;
		const handler = editor.target === "primary" ? onEdit : onSecondaryEdit;
		const rangeHandler =
			editor.target === "primary" ? onEditRange : onSecondaryEditRange;
		if (submitNumericExpression(candidate, handler, rangeHandler))
			setEditor(null);
	};
	if (!target)
		return (
			<section
				className="hardware-encoder-display unassigned"
				aria-label={`Encoder ${slot} unassigned`}
			>
				<header>
					<b>Unassigned</b>
					<small>Enc {slot}</small>
				</header>
			</section>
		);
	const content = (
		<HardwareEncoderContent slot={slot} target={target} secondary={secondary} />
	);
	const displayClassName = `hardware-encoder-display ${secondary ? "dual-target" : "single-target"}`;
	const handleSurfaceClick = (event: ReactMouseEvent<HTMLButtonElement>) => {
		if (!secondary || !onSecondaryEdit || event.detail === 0) {
			activate();
			return;
		}
		const bounds = event.currentTarget.getBoundingClientRect();
		const touchedTarget =
			event.clientY < bounds.top + bounds.height / 2 ? "primary" : "secondary";
		if (touchedTarget === "primary" && !onEdit) {
			activate();
			return;
		}
		openEditor(touchedTarget, false);
	};
	return (
		<>
			{onEdit || onSecondaryEdit ? (
				<Button
					className={displayClassName}
					aria-label={`Encoder ${slot}: ${target.label}, ${target.value}`}
					onClick={handleSurfaceClick}
				>
					{content}
				</Button>
			) : (
				<section
					className={displayClassName}
					aria-label={`Encoder ${slot}: ${target.label}, ${target.value}`}
				>
					{content}
				</section>
			)}
			{editor && (
				<HardwareEncoderEditor
					slot={slot}
					target={target}
					secondary={secondary}
					editor={editor}
					inputValues={inputValues}
					setInputValues={setInputValues}
					setEditor={setEditor}
					submit={submit}
					onEditRange={onEditRange}
					onSecondaryEditRange={onSecondaryEditRange}
					canRelease={canRelease}
					presets={presets}
					onPresetSelect={onPresetSelect}
					onRelease={onRelease}
				/>
			)}
		</>
	);
});
