import {
	type CSSProperties,
	type ReactNode,
	useEffect,
	useRef,
	useState,
} from "react";
import { createPortal } from "react-dom";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { Button, type ButtonProps, Input } from "../common";
import { ModalNumberInput } from "../input/ModalInputControls";

export interface VerticalTouchFaderAction
	extends Omit<ButtonProps, "children"> {
	id: string;
	label: ReactNode;
}

export interface VerticalTouchFaderProps {
	label: string;
	value: number;
	maximum?: number;
	display?: string;
	disabled?: boolean;
	accentColor?: string;
	mode?: string;
	directInput?: boolean;
	directInputOffset?: number;
	actions?: VerticalTouchFaderAction[];
	onChange?: (value: number) => void;
}

interface SetValueDialogProps {
	label: string;
	value: string;
	maximum: number;
	offset: number;
	onChange(value: string): void;
	onFaderChange(value: number): void;
	onSubmit(): void;
	onClose(): void;
}

function SetValueDialog({
	label,
	value,
	maximum,
	offset,
	onChange,
	onFaderChange,
	onSubmit,
	onClose,
}: SetValueDialogProps) {
	const entered = Number(value);
	const faderValue = Number.isFinite(entered)
		? Math.max(0, Math.min(maximum, entered + offset))
		: offset;
	return createPortal(
		// biome-ignore lint/a11y: The modal backdrop only intercepts pointer bubbling; the dialog owns keyboard actions.
		<div
			className="stacked-modal-layer"
			onClick={(event) => event.stopPropagation()}
			onPointerDown={(event) =>
				event.target === event.currentTarget && onClose()
			}
		>
			<section
				className="nested-modal direct-value-modal"
				role="dialog"
				aria-modal="true"
				aria-label={`${label} value`}
			>
				<Button
					className="modal-close"
					aria-label="Close attribute value"
					onClick={onClose}
				>
					×
				</Button>
				<h3>{label}</h3>
				<strong>{value || "0"}</strong>
				<div className="direct-value-modal-body">
					<VerticalTouchFader
						label={label}
						value={faderValue}
						maximum={maximum}
						display={value || "0"}
						onChange={onFaderChange}
					/>
					<ModalNumberInput
						value={value}
						onChange={onChange}
						onEnter={onSubmit}
						onEscape={onClose}
						replaceOnFirstInput
					/>
				</div>
			</section>
		</div>,
		document.body,
	);
}

function useFaderInteraction(
	value: number,
	onChange?: (value: number) => void,
) {
	const [localValue, setLocalValue] = useState(value);
	const interacting = useRef(false);
	const frame = useRef<number | null>(null);
	const queued = useRef(value);
	useEffect(() => {
		if (!interacting.current) setLocalValue(value);
	}, [value]);
	useEffect(
		() => () => {
			if (frame.current !== null) window.cancelAnimationFrame(frame.current);
		},
		[],
	);
	const emit = (next: number) => {
		queued.current = next;
		setLocalValue(next);
		if (frame.current !== null) return;
		frame.current = window.requestAnimationFrame(() => {
			frame.current = null;
			onChange?.(queued.current);
		});
	};
	const finish = () => {
		interacting.current = false;
		if (frame.current !== null) window.cancelAnimationFrame(frame.current);
		frame.current = null;
		onChange?.(queued.current);
	};
	return { localValue, setLocalValue, interacting, emit, finish };
}

function useDirectInput(
	localValue: number,
	setLocalValue: (value: number) => void,
	maximum: number,
	offset: number,
	disabled: boolean,
	directInput: boolean,
	onChange?: (value: number) => void,
) {
	const [open, setOpen] = useState(false);
	const [value, setValue] = useState("");
	const show = () => {
		if (disabled || !directInput) return;
		setValue(String(Number((localValue - offset).toFixed(1))));
		setOpen(true);
	};
	const apply = (next: number) => {
		const entered = next - offset;
		setValue(String(Number(entered.toFixed(1))));
		setLocalValue(next);
		onChange?.(next);
	};
	const submit = () => {
		const entered = Number(value);
		const next = Math.max(0, Math.min(maximum, entered + offset));
		if (Number.isFinite(entered)) apply(next);
		setOpen(false);
	};
	return {
		open,
		value,
		setValue,
		show,
		apply,
		submit,
		close: () => setOpen(false),
	};
}

function FaderActions({ actions }: { actions: VerticalTouchFaderAction[] }) {
	if (!actions.length) return null;
	return (
		<div
			className="vertical-touch-fader-actions"
			style={{ "--fader-action-count": actions.length } as CSSProperties}
		>
			{actions.map(({ id, label, ...props }) => (
				<Button type="button" {...props} key={id}>
					{label}
				</Button>
			))}
		</div>
	);
}

export function VerticalTouchFader({
	label,
	value,
	maximum = 100,
	display,
	disabled = false,
	accentColor,
	mode,
	directInput = false,
	directInputOffset = 0,
	actions = [],
	onChange,
}: VerticalTouchFaderProps) {
	const server = useServer();
	const { state } = useApp();
	const hardware = Boolean(
		server.bootstrap?.hardware_connected || state.midiProfile,
	);
	const fader = useFaderInteraction(value, onChange);
	const input = useDirectInput(
		fader.localValue,
		fader.setLocalValue,
		maximum,
		directInputOffset,
		disabled,
		directInput,
		onChange,
	);
	const fraction = Math.max(
		0,
		Math.min(1, maximum ? fader.localValue / maximum : 0),
	);
	const visibleActions = [
		...(directInput
			? [
					{
						id: "set-value",
						label: "Set value",
						disabled,
						onClick: input.show,
						className: "set-value-button",
					} satisfies VerticalTouchFaderAction,
				]
			: []),
		...actions,
	].slice(0, 3);
	return (
		<div
			className={`vertical-touch-fader-stack ${visibleActions.length ? `has-actions action-count-${visibleActions.length}` : ""}`}
		>
			{/* biome-ignore lint/a11y/noLabelWithoutControl: Input renders the native range inside this label. */}
			{/* biome-ignore lint/a11y/useKeyWithClickEvents: The adjacent Set value button provides the keyboard path; the label expands the hardware pointer target. */}
			<label
				onClick={() => hardware && input.show()}
				className={`vertical-touch-fader ${disabled ? "disabled" : ""} ${directInput ? "direct-input-fader" : ""}`}
				style={
					{
						"--fader-level": fraction,
						"--fader-color": accentColor ?? "#176777",
						"--fader-color-dark": accentColor
							? `color-mix(in srgb, ${accentColor} 42%, #081014)`
							: "#103039",
					} as CSSProperties
				}
			>
				<span>
					{label}
					{mode && <small>{mode}</small>}
				</span>
				<strong>{display ?? `${Math.round(fader.localValue)}%`}</strong>
				<Input
					aria-label={label}
					disabled={disabled || (hardware && directInput)}
					type="range"
					min="0"
					max={maximum}
					step="0.1"
					value={fader.localValue}
					onPointerDown={() => {
						fader.interacting.current = true;
					}}
					onPointerUp={fader.finish}
					onPointerCancel={fader.finish}
					onBlur={() => fader.interacting.current && fader.finish()}
					onInput={(event) => fader.emit(Number(event.currentTarget.value))}
				/>
			</label>
			<FaderActions actions={visibleActions} />
			{input.open && (
				<SetValueDialog
					label={label}
					value={input.value}
					maximum={maximum}
					offset={directInputOffset}
					onChange={input.setValue}
					onFaderChange={input.apply}
					onSubmit={input.submit}
					onClose={input.close}
				/>
			)}
		</div>
	);
}

export function TouchValueButton({
	label,
	value,
	maximum = 100,
	display,
	onChange,
}: Pick<
	VerticalTouchFaderProps,
	"label" | "value" | "maximum" | "display" | "onChange"
>) {
	const [open, setOpen] = useState(false);
	const [inputValue, setInputValue] = useState("");
	const apply = (next: number) => {
		const clamped = Math.max(0, Math.min(maximum, next));
		setInputValue(String(Number(clamped.toFixed(1))));
		onChange?.(clamped);
	};
	const submit = () => {
		const next = Number(inputValue);
		if (Number.isFinite(next)) apply(next);
		setOpen(false);
	};
	return (
		<div className="touch-value-button">
			<Button
				type="button"
				onClick={() => {
					setInputValue(String(Number(value.toFixed(1))));
					setOpen(true);
				}}
			>
				<span>{label}</span>
				<strong>{display ?? `${Math.round(value)}%`}</strong>
				<small>Set value</small>
			</Button>
			{open && (
				<SetValueDialog
					label={label}
					value={inputValue}
					maximum={maximum}
					offset={0}
					onChange={setInputValue}
					onFaderChange={apply}
					onSubmit={submit}
					onClose={() => setOpen(false)}
				/>
			)}
		</div>
	);
}
