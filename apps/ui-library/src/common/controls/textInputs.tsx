import {
	type ChangeEvent,
	forwardRef,
	type InputHTMLAttributes,
	type ReactNode,
	type TextareaHTMLAttributes,
	useEffect,
	useImperativeHandle,
	useRef,
	useState,
} from "react";
import {
	ModalNumberEditor,
	type ModalNumberFaderConfig,
	type ModalNumberPresetConfig,
} from "../../input/ModalNumberEditor";
import { Button } from "./foundation";
import { InputModal } from "./InputModal";

function emitInputValue(
	input: HTMLInputElement | null,
	next: string,
	onChange?: (event: ChangeEvent<HTMLInputElement>) => void,
	onValueChange?: (value: string) => void,
) {
	if (!input) return;
	input.value = next;
	onValueChange?.(next);
	onChange?.({
		target: input,
		currentTarget: input,
	} as ChangeEvent<HTMLInputElement>);
}

function emitTextAreaValue(
	input: HTMLTextAreaElement | null,
	next: string,
	onChange?: (event: ChangeEvent<HTMLTextAreaElement>) => void,
	onValueChange?: (value: string) => void,
) {
	if (!input) return;
	input.value = next;
	onValueChange?.(next);
	onChange?.({
		target: input,
		currentTarget: input,
	} as ChangeEvent<HTMLTextAreaElement>);
}

export interface TextInputProps
	extends Omit<InputHTMLAttributes<HTMLInputElement>, "type"> {
	clearable?: boolean;
	clearLabel?: string;
	keyboardLabel?: string;
	liveKeyboard?: boolean;
	modalLeadingIcon?: ReactNode;
	onValueChange?: (value: string) => void;
	onKeyboardCommit?: (value: string) => void;
	openKeyboardInitially?: boolean;
	keyboardRequest?: number;
	secure?: boolean;
}

export const TextInput = forwardRef<HTMLInputElement, TextInputProps>(
	function TextInput(
		{
			className = "",
			value,
			defaultValue,
			onChange,
			onValueChange,
			onKeyboardCommit,
			clearable = false,
			clearLabel = "Clear input",
			keyboardLabel,
			liveKeyboard = false,
			modalLeadingIcon,
			openKeyboardInitially = false,
			keyboardRequest = 0,
			secure = false,
			disabled,
			readOnly,
			...props
		},
		ref,
	) {
		const [open, setOpen] = useState(openKeyboardInitially);
		const lastKeyboardRequest = useRef(keyboardRequest);
		const native = useRef<HTMLInputElement>(null);
		useImperativeHandle(ref, () => native.current!);
		const current = String(
			value ?? native.current?.value ?? defaultValue ?? "",
		);
		useEffect(() => {
			if (keyboardRequest === lastKeyboardRequest.current) return;
			lastKeyboardRequest.current = keyboardRequest;
			setOpen(true);
		}, [keyboardRequest]);
		const update = (next: string) =>
			emitInputValue(native.current, next, onChange, onValueChange);
		const close = () => {
			setOpen(false);
		};
		const commit = (next: string) => {
			update(next);
			close();
			onKeyboardCommit?.(next);
		};
		return (
			<span className="ui-text-control">
				<input
					{...props}
					ref={native}
					type={secure ? "password" : "text"}
					value={value}
					defaultValue={defaultValue}
					disabled={disabled}
					readOnly={readOnly}
					className={`ui-input ${className}`.trim()}
					onChange={(event) => {
						onValueChange?.(event.target.value);
						onChange?.(event);
					}}
				/>
				{clearable && (
					<Button
						size="compact"
						iconOnly
						className={`ui-input-clear ${current ? "" : "is-empty"}`.trim()}
						aria-label={clearLabel}
						aria-hidden={!current}
						tabIndex={current ? undefined : -1}
						disabled={disabled || readOnly || !current}
						onClick={() => {
							update("");
							native.current?.focus();
						}}
					>
						<span className="ui-input-clear-icon" aria-hidden="true">
							×
						</span>
					</Button>
				)}
				<Button
					size="compact"
					iconOnly
					className="ui-input-keyboard"
					aria-label="Open keyboard"
					disabled={disabled || readOnly}
					onClick={() => setOpen(true)}
				>
					<span className="ui-keyboard-icon" aria-hidden="true">
						⌨
					</span>
				</Button>
				{open && (
					<InputModal
						kind="text"
						value={current}
						initialCaret={native.current?.selectionStart ?? current.length}
						secure={secure}
						placeholder={props.placeholder}
						label={keyboardLabel ?? props["aria-label"]}
						leadingIcon={modalLeadingIcon}
						onDraftChange={liveKeyboard ? update : undefined}
						onCommit={commit}
						onCancel={close}
					/>
				)}
			</span>
		);
	},
);

export interface NumberInputProps
	extends Omit<InputHTMLAttributes<HTMLInputElement>, "type"> {
	allowDecimal?: boolean;
	allowThrough?: boolean;
	showStepButtons?: boolean;
	stepBehavior?: "increment" | "snap";
	wrapStepAtBounds?: boolean;
	keyboardLabel?: string;
	onValueChange?: (value: string) => void;
	onStepCommit?: (value: string) => void;
	onKeyboardCommit?: (value: string) => void;
	onRangeCommit?: (points: number[]) => void;
	modalFader?: ModalNumberFaderConfig;
	modalPresets?: ModalNumberPresetConfig;
	onModalRelease?: () => void;
	modalReleaseLabel?: string;
	unit?: ReactNode;
	keyboardRequest?: number;
}

function clampNumber(
	value: number,
	min: NumberInputProps["min"],
	max: NumberInputProps["max"],
) {
	const lower = min == null ? -Infinity : Number(min);
	const upper = max == null ? Infinity : Number(max);
	return Math.max(lower, Math.min(upper, value));
}

function normalizeNumberText(
	next: string,
	allowDecimal: boolean,
	current: string,
) {
	const filtered = allowDecimal
		? next.replace(/[^\d.-]/g, "")
		: next.replace(/[^\d-]/g, "");
	if (filtered === "" || filtered === "-") return filtered;
	const valid = allowDecimal
		? /^-?\d*\.?\d*$/.test(filtered)
		: /^-?\d+$/.test(filtered);
	return valid ? filtered : current;
}

function committedNumberText(
	next: string,
	min: NumberInputProps["min"],
	max: NumberInputProps["max"],
) {
	const parsed = Number(next);
	if (next === "" || next === "-" || Number.isNaN(parsed)) return "";
	return String(clampNumber(parsed, min, max));
}

function NumberStepButton({
	direction,
	disabled,
	onClick,
}: {
	direction: -1 | 1;
	disabled: boolean;
	onClick(): void;
}) {
	const decrease = direction === -1;
	return (
		<Button
			size="compact"
			iconOnly
			className={decrease ? "ui-number-minus" : "ui-number-plus"}
			aria-label={decrease ? "Decrease value" : "Increase value"}
			disabled={disabled}
			onClick={onClick}
		>
			<span className="ui-step-icon" aria-hidden="true">
				{decrease ? "−" : "+"}
			</span>
		</Button>
	);
}

function NumberInputModal({
	allowDecimal,
	allowThrough,
	current,
	fader,
	keyboardLabel,
	modalValue,
	onChange,
	onClose,
	onCommit,
	onRelease,
	presets,
	releaseLabel,
	unit,
}: {
	allowDecimal: boolean;
	allowThrough: boolean;
	current: string;
	fader?: ModalNumberFaderConfig;
	keyboardLabel: string;
	modalValue: string;
	onChange(value: string): void;
	onClose(): void;
	onCommit(value?: string): void;
	onRelease?: () => void;
	presets?: ModalNumberPresetConfig;
	releaseLabel?: string;
	unit?: ReactNode;
}) {
	return (
		<ModalNumberEditor
			ariaLabel={keyboardLabel}
			title={keyboardLabel}
			value={modalValue}
			onChange={onChange}
			onSubmit={onCommit}
			onClose={onClose}
			allowDecimal={allowDecimal}
			allowThrough={allowThrough}
			fader={fader}
			presets={
				presets
					? {
							...presets,
							selectedValue: presets.selectedValue ?? current,
						}
					: undefined
			}
			onRelease={onRelease}
			releaseLabel={releaseLabel}
			unit={unit}
		/>
	);
}

function OpenNumberPadButton({
	disabled,
	onClick,
}: {
	disabled: boolean;
	onClick(): void;
}) {
	return (
		<Button
			size="compact"
			iconOnly
			className="ui-input-keyboard"
			aria-label="Open number pad"
			disabled={disabled}
			onClick={onClick}
		>
			<span className="ui-keyboard-icon" aria-hidden="true">
				⌨
			</span>
		</Button>
	);
}

function steppedNumber(
	current: string,
	direction: -1 | 1,
	step: number | string,
	behavior: "increment" | "snap",
	wrap: boolean,
	min: number | string | undefined,
	max: number | string | undefined,
) {
	const numeric = Number(current) || 0;
	const increment = Number(step);
	let next =
		behavior === "snap"
			? direction === 1
				? (Math.floor(numeric / increment) + 1) * increment
				: (Math.ceil(numeric / increment) - 1) * increment
			: numeric + increment * direction;
	const lower = min == null ? -Infinity : Number(min);
	const upper = max == null ? Infinity : Number(max);
	if (wrap && next < lower) next = upper;
	else if (wrap && next > upper) next = lower;
	else next = clampNumber(next, min, max);
	return String(next);
}

function commitNumberModal(
	next: string,
	allowThrough: boolean,
	onRangeCommit: ((points: number[]) => void) | undefined,
	commit: (value: string) => string,
	close: () => void,
	onKeyboardCommit: ((value: string) => void) | undefined,
) {
	if (allowThrough && /\bTHRU\b/i.test(next) && onRangeCommit) {
		const points = next
			.split(/\s+THRU\s+/i)
			.map((point) => Number(point.trim()));
		if (points.length > 1 && points.every(Number.isFinite)) {
			onRangeCommit(points);
			close();
			return;
		}
	}
	const committed = commit(next);
	close();
	onKeyboardCommit?.(committed);
}

export const NumberInput = forwardRef<HTMLInputElement, NumberInputProps>(
	function NumberInput(inputProps, ref) {
		const {
			className = "",
			value,
			defaultValue,
			onChange,
			onValueChange,
			allowDecimal = false,
			showStepButtons = true,
			stepBehavior = "increment",
			wrapStepAtBounds = false,
			keyboardRequest = 0,
			disabled,
			readOnly,
			min,
			max,
			step = 1,
			...props
		} = inputProps;
		const [open, setOpen] = useState(false);
		const lastKeyboardRequest = useRef(keyboardRequest);
		const [modalValue, setModalValue] = useState("");
		const native = useRef<HTMLInputElement>(null);
		useImperativeHandle(ref, () => native.current!);
		const current = String(
			value ?? native.current?.value ?? defaultValue ?? "",
		);
		useEffect(() => {
			if (keyboardRequest === lastKeyboardRequest.current) return;
			lastKeyboardRequest.current = keyboardRequest;
			setModalValue(current);
			setOpen(true);
		}, [keyboardRequest, current]);
		const update = (next: string) =>
			emitInputValue(
				native.current,
				normalizeNumberText(next, allowDecimal, current),
				onChange,
				onValueChange,
			);
		const commit = (next: string) => {
			const committed = committedNumberText(next, min, max);
			update(committed);
			return committed;
		};
		const bump = (direction: -1 | 1) => {
			const committed = steppedNumber(
				current,
				direction,
				step,
				stepBehavior,
				wrapStepAtBounds,
				min,
				max,
			);
			update(committed);
			inputProps.onStepCommit?.(committed);
		};
		const close = () => {
			setOpen(false);
		};
		const commitModal = (next: string) =>
			commitNumberModal(
				next,
				inputProps.allowThrough ?? false,
				inputProps.onRangeCommit,
				commit,
				close,
				inputProps.onKeyboardCommit,
			);
		return (
			<span
				className={`ui-number-control ${showStepButtons ? "with-steppers" : "without-steppers"}`}
			>
				{showStepButtons && (
					<NumberStepButton
						direction={-1}
						disabled={Boolean(disabled || readOnly)}
						onClick={() => bump(-1)}
					/>
				)}
				<input
					{...props}
					ref={native}
					type="text"
					inputMode={allowDecimal ? "decimal" : "numeric"}
					value={value}
					defaultValue={defaultValue}
					disabled={disabled}
					readOnly={readOnly}
					className={`ui-input ${className}`.trim()}
					onChange={(event) => {
						const next = normalizeNumberText(
							event.target.value,
							allowDecimal,
							current,
						);
						if (next !== event.target.value) event.target.value = next;
						onValueChange?.(next);
						onChange?.(event);
					}}
					onBlur={(event) => {
						commit(event.target.value);
						props.onBlur?.(event);
					}}
				/>
				{showStepButtons && (
					<NumberStepButton
						direction={1}
						disabled={Boolean(disabled || readOnly)}
						onClick={() => bump(1)}
					/>
				)}
				<OpenNumberPadButton
					disabled={Boolean(disabled || readOnly)}
					onClick={() => {
						setModalValue(current);
						setOpen(true);
					}}
				/>
				{open && (
					<NumberInputModal
						allowDecimal={allowDecimal}
						allowThrough={inputProps.allowThrough ?? false}
						current={current}
						fader={inputProps.modalFader}
						keyboardLabel={
							inputProps.keyboardLabel ?? props["aria-label"] ?? "Number input"
						}
						modalValue={modalValue}
						onChange={setModalValue}
						onClose={close}
						onCommit={(next = modalValue) => commitModal(next)}
						onRelease={
							inputProps.onModalRelease
								? () => {
										inputProps.onModalRelease?.();
										close();
									}
								: undefined
						}
						presets={inputProps.modalPresets}
						releaseLabel={inputProps.modalReleaseLabel}
						unit={inputProps.unit}
					/>
				)}
			</span>
		);
	},
);

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
	allowDecimal?: boolean;
	showStepButtons?: boolean;
}

const WheelSafeRangeInput = forwardRef<
	HTMLInputElement,
	InputHTMLAttributes<HTMLInputElement>
>(function WheelSafeRangeInput(
	{ className = "", onWheel: _onWheel, ...props },
	ref,
) {
	const native = useRef<HTMLInputElement>(null);
	useImperativeHandle(ref, () => native.current!);
	useEffect(() => {
		const input = native.current;
		if (!input) return;
		const rejectWheel = (event: WheelEvent) => {
			event.preventDefault();
			input.blur();
		};
		input.addEventListener("wheel", rejectWheel, { passive: false });
		return () => input.removeEventListener("wheel", rejectWheel);
	}, []);
	return (
		<input
			{...props}
			ref={native}
			type="range"
			className={`ui-native-control ${className}`.trim()}
		/>
	);
});

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
	{ className = "", type = "text", allowDecimal, showStepButtons, ...props },
	ref,
) {
	if (type === "number") {
		return (
			<NumberInput
				{...props}
				allowDecimal={allowDecimal}
				showStepButtons={showStepButtons}
				className={className}
				ref={ref}
			/>
		);
	}
	if (type === "range")
		return <WheelSafeRangeInput {...props} className={className} ref={ref} />;
	const native = [
		"checkbox",
		"radio",
		"range",
		"file",
		"color",
		"hidden",
	].includes(type);
	return (
		<input
			ref={ref}
			type={type}
			className={`${native ? "ui-native-control" : "ui-input"} ${className}`.trim()}
			{...props}
		/>
	);
});

export const TextArea = forwardRef<
	HTMLTextAreaElement,
	TextareaHTMLAttributes<HTMLTextAreaElement>
>(function TextArea({ className = "", ...props }, ref) {
	return (
		<textarea
			ref={ref}
			className={`ui-textarea ${className}`.trim()}
			{...props}
		/>
	);
});

export interface LargeTextInputProps
	extends TextareaHTMLAttributes<HTMLTextAreaElement> {
	keyboardLabel?: string;
	liveKeyboard?: boolean;
	onKeyboardCommit?: (value: string) => void;
	onValueChange?: (value: string) => void;
}

function scrollTextArea(area: HTMLTextAreaElement, direction: -1 | 1) {
	const start = area.selectionStart;
	const end = area.selectionEnd;
	const directionState = area.selectionDirection;
	const distance = Math.max(area.clientHeight * 0.75, 66);
	area.scrollBy({ top: direction * distance, behavior: "smooth" });
	area.setSelectionRange(start, end, directionState);
}

export const LargeTextInput = forwardRef<
	HTMLTextAreaElement,
	LargeTextInputProps
>(function LargeTextInput(
	{
		className = "",
		value,
		defaultValue,
		onChange,
		onValueChange,
		onKeyboardCommit,
		keyboardLabel,
		liveKeyboard = false,
		disabled,
		readOnly,
		...props
	},
	ref,
) {
	const [open, setOpen] = useState(false);
	const native = useRef<HTMLTextAreaElement>(null);
	const selection = useRef(0);
	useImperativeHandle(ref, () => native.current!);
	const current = String(value ?? native.current?.value ?? defaultValue ?? "");
	const update = (next: string) =>
		emitTextAreaValue(native.current, next, onChange, onValueChange);
	const close = () => {
		setOpen(false);
	};
	const commit = (next: string) => {
		update(next);
		close();
		onKeyboardCommit?.(next);
	};
	return (
		<span className="ui-large-text-control">
			<textarea
				{...props}
				ref={native}
				value={value}
				defaultValue={defaultValue}
				disabled={disabled}
				readOnly={readOnly}
				className={`ui-textarea ${className}`.trim()}
				onChange={(event) => {
					selection.current = event.currentTarget.selectionStart;
					onValueChange?.(event.target.value);
					onChange?.(event);
				}}
				onClick={(event) => {
					selection.current = event.currentTarget.selectionStart;
					props.onClick?.(event);
				}}
				onKeyUp={(event) => {
					selection.current = event.currentTarget.selectionStart;
					props.onKeyUp?.(event);
				}}
				onSelect={(event) => {
					selection.current = event.currentTarget.selectionStart;
					props.onSelect?.(event);
				}}
			/>
			<Button
				size="compact"
				iconOnly
				className="ui-large-text-up"
				aria-label="Scroll text up"
				disabled={disabled}
				onClick={() => native.current && scrollTextArea(native.current, -1)}
			>
				<svg viewBox="0 0 24 24" aria-hidden="true">
					<path d="m5 15 7-7 7 7" />
				</svg>
			</Button>
			<Button
				size="compact"
				iconOnly
				className="ui-large-text-down"
				aria-label="Scroll text down"
				disabled={disabled}
				onClick={() => native.current && scrollTextArea(native.current, 1)}
			>
				<svg viewBox="0 0 24 24" aria-hidden="true">
					<path d="m5 9 7 7 7-7" />
				</svg>
			</Button>
			<Button
				size="compact"
				iconOnly
				className="ui-large-text-keyboard"
				aria-label="Open keyboard"
				disabled={disabled || readOnly}
				onClick={() => setOpen(true)}
			>
				<span className="ui-keyboard-icon" aria-hidden="true">
					⌨
				</span>
			</Button>
			{open && (
				<InputModal
					kind="multiline"
					value={current}
					initialCaret={selection.current}
					placeholder={props.placeholder}
					label={keyboardLabel ?? props["aria-label"]}
					onDraftChange={liveKeyboard ? update : undefined}
					onCommit={commit}
					onCancel={close}
				/>
			)}
		</span>
	);
});
