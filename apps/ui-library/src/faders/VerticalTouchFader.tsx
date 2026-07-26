import {
	type CSSProperties,
	type ReactNode,
	type PointerEvent as ReactPointerEvent,
	useEffect,
	useRef,
	useState,
} from "react";
import { Button, type ButtonProps, Input } from "../common";
import {
	ModalNumberEditor,
	type ModalNumberPresetConfig,
} from "../input/ModalNumberEditor";
import { submitNumericExpression } from "../input/numericExpression";
import { VerticalTouchFaderControl } from "./VerticalTouchFaderControl";

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
	/** THRU spread submission from the value dialog; parity with the hardware encoder modal. */
	onChangeRange?: (points: number[]) => void;
	presets?: ModalNumberPresetConfig;
	onRelease?: () => void;
	releaseLabel?: string;
}

export interface VerticalTouchFaderSurfaceProps
	extends VerticalTouchFaderProps {
	hardware: boolean;
}

interface SetValueDialogProps {
	label: string;
	value: string;
	maximum: number;
	offset: number;
	allowThrough?: boolean;
	presets?: ModalNumberPresetConfig;
	onChange(value: string): void;
	onFaderChange(value: number): void;
	onSubmit(value?: string): void;
	onClose(): void;
	onRelease?: () => void;
	releaseLabel?: string;
}

function SetValueDialog({
	label,
	value,
	maximum,
	offset,
	allowThrough = false,
	presets,
	onChange,
	onFaderChange,
	onSubmit,
	onClose,
	onRelease,
	releaseLabel,
}: SetValueDialogProps) {
	const entered = Number(value);
	const faderValue = Number.isFinite(entered)
		? Math.max(0, Math.min(maximum, entered + offset))
		: offset;
	return (
		<ModalNumberEditor
			ariaLabel={`${label} value`}
			title={label}
			value={value}
			onChange={onChange}
			onSubmit={onSubmit}
			onClose={onClose}
			allowThrough={allowThrough}
			fader={{
				label: `${label} fader`,
				maximum,
				valueFromInput: (next) => {
					const parsed = Number(next);
					return Number.isFinite(parsed)
						? Math.max(0, Math.min(maximum, parsed + offset))
						: faderValue;
				},
				inputFromValue: (next) => String(Number((next - offset).toFixed(1))),
				onChange: onFaderChange,
			}}
			presets={presets}
			onRelease={onRelease}
			releaseLabel={releaseLabel}
		/>
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

function valueFromVerticalPointer(
	clientY: number,
	bounds: Pick<DOMRect, "height" | "top">,
	maximum: number,
) {
	const height = Math.max(1, bounds.height);
	const y = Math.max(0, Math.min(height, clientY - bounds.top));
	const endpointZone = Math.min(
		height / 3,
		Math.max(18, Math.min(36, height * 0.1)),
	);
	if (y <= endpointZone) return maximum;
	if (y >= height - endpointZone) return 0;
	return (
		maximum * (1 - (y - endpointZone) / Math.max(1, height - endpointZone * 2))
	);
}

function useDirectInput(
	localValue: number,
	setLocalValue: (value: number) => void,
	maximum: number,
	offset: number,
	disabled: boolean,
	directInput: boolean,
	onChange?: (value: number) => void,
	onChangeRange?: (points: number[]) => void,
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
	const submit = (candidate = value) => {
		const handled = submitNumericExpression(
			candidate,
			(entered) => apply(Math.max(0, Math.min(maximum, entered + offset))),
			onChangeRange,
		);
		if (handled) setOpen(false);
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

/** Pure fader surface for feature owners that already have scoped layout state. */
export function VerticalTouchFaderSurface({
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
	onChangeRange,
	presets,
	onRelease,
	releaseLabel,
	hardware,
}: VerticalTouchFaderSurfaceProps) {
	const fader = useFaderInteraction(value, onChange);
	const input = useDirectInput(
		fader.localValue,
		fader.setLocalValue,
		maximum,
		directInputOffset,
		disabled,
		directInput,
		onChange,
		onChangeRange,
	);
	const fraction = Math.max(
		0,
		Math.min(1, maximum ? fader.localValue / maximum : 0),
	);
	const canMoveFader = !disabled && !(hardware && directInput);
	const moveFader = (
		event: ReactPointerEvent<HTMLInputElement>,
		finish = false,
	) => {
		if (!canMoveFader) return;
		event.preventDefault();
		const control = event.currentTarget;
		fader.emit(
			valueFromVerticalPointer(
				event.clientY,
				control.getBoundingClientRect(),
				maximum,
			),
		);
		if (finish) fader.finish();
	};
	const visibleActions = [
		...(directInput
			? [
					{
						id: "set-value",
						label: "Set value",
						disabled,
						onClick: input.show,
						className: "set-value-button",
						"aria-haspopup": "dialog",
						"aria-expanded": input.open,
					} satisfies VerticalTouchFaderAction,
				]
			: []),
		...actions,
	].slice(0, 3);
	return (
		<div
			className={`vertical-touch-fader-stack ${visibleActions.length ? `has-actions action-count-${visibleActions.length}` : ""}`}
		>
			<VerticalTouchFaderControl
				onClick={(event) => {
					event.preventDefault();
					if (hardware && directInput) input.show();
				}}
				className={directInput ? "direct-input-fader" : ""}
				label={label}
				mode={mode}
				display={display ?? `${Math.round(fader.localValue)}%`}
				fraction={fraction}
				accentColor={accentColor}
				disabled={disabled}
			>
				<Input
					aria-label={label}
					disabled={disabled || (hardware && directInput)}
					type="range"
					min="0"
					max={maximum}
					step="0.1"
					value={fader.localValue}
					onPointerDown={(event) => {
						if (!canMoveFader) return;
						event.preventDefault();
						event.currentTarget.focus({ preventScroll: true });
						event.currentTarget.setPointerCapture?.(event.pointerId);
						fader.interacting.current = true;
						moveFader(event);
					}}
					onPointerMove={(event) => {
						if (fader.interacting.current) moveFader(event);
					}}
					onPointerUp={(event) => {
						if (!fader.interacting.current) return;
						moveFader(event, true);
						if (event.currentTarget.hasPointerCapture?.(event.pointerId))
							event.currentTarget.releasePointerCapture(event.pointerId);
					}}
					onPointerCancel={() => {
						if (fader.interacting.current) fader.finish();
					}}
					onBlur={() => fader.interacting.current && fader.finish()}
					onInput={(event) => fader.emit(Number(event.currentTarget.value))}
				/>
			</VerticalTouchFaderControl>
			<FaderActions actions={visibleActions} />
			{input.open && (
				<SetValueDialog
					label={label}
					value={input.value}
					maximum={maximum}
					offset={directInputOffset}
					allowThrough={Boolean(onChangeRange)}
					presets={presets}
					onChange={input.setValue}
					onFaderChange={input.apply}
					onSubmit={input.submit}
					onClose={input.close}
					onRelease={
						onRelease
							? () => {
									onRelease();
									input.close();
								}
							: undefined
					}
					releaseLabel={releaseLabel}
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
	presets,
	onRelease,
	releaseLabel,
}: Pick<
	VerticalTouchFaderProps,
	| "label"
	| "value"
	| "maximum"
	| "display"
	| "onChange"
	| "presets"
	| "onRelease"
	| "releaseLabel"
>) {
	const [open, setOpen] = useState(false);
	const [inputValue, setInputValue] = useState("");
	const apply = (next: number) => {
		const clamped = Math.max(0, Math.min(maximum, next));
		setInputValue(String(Number(clamped.toFixed(1))));
		onChange?.(clamped);
	};
	const submit = (candidate = inputValue) => {
		const next = Number(candidate);
		if (Number.isFinite(next)) apply(next);
		setOpen(false);
	};
	return (
		<div className="touch-value-button">
			<Button
				type="button"
				aria-haspopup="dialog"
				aria-expanded={open}
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
					presets={presets}
					onChange={setInputValue}
					onFaderChange={apply}
					onSubmit={submit}
					onClose={() => setOpen(false)}
					onRelease={
						onRelease
							? () => {
									onRelease();
									setOpen(false);
								}
							: undefined
					}
					releaseLabel={releaseLabel}
				/>
			)}
		</div>
	);
}
