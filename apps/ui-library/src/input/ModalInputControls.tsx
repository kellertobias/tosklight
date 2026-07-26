import {
	useEffect,
	useId,
	useLayoutEffect,
	useRef,
	useState,
	type PointerEvent as ReactPointerEvent,
	type ReactNode,
} from "react";
import { Button } from "../common/controls/foundation";

const inputStack: string[] = [];

function useModalInput(onKey: (key: string) => void) {
	const id = useId();
	const handler = useRef(onKey);
	const root = useRef<HTMLDivElement>(null);
	handler.current = onKey;
	useEffect(() => {
		inputStack.push(id);
		const keydown = (event: KeyboardEvent) => {
			if (inputStack.at(-1) !== id) return;
			const topLayer = document.querySelector<HTMLElement>(
				'.ui-modal-stack-layer[data-modal-top="true"]',
			);
			if (topLayer && root.current && !topLayer.contains(root.current)) return;
			const key = event.key;
			if (
				key !== "Escape" &&
				key !== "Enter" &&
				key !== "Backspace" &&
				key !== "ArrowLeft" &&
				key !== "ArrowRight" &&
				key.length !== 1
			)
				return;
			event.preventDefault();
			event.stopImmediatePropagation();
			handler.current(key);
		};
		window.addEventListener("keydown", keydown, true);
		return () => {
			window.removeEventListener("keydown", keydown, true);
			const index = inputStack.lastIndexOf(id);
			if (index >= 0) inputStack.splice(index, 1);
		};
	}, [id]);
	return root;
}

interface ModalInputProps {
	value: string;
	caret?: number;
	onChange: (value: string) => void;
	onEnter: () => void;
	onEscape: () => void;
	onCaretChange?: (position: number) => void;
}

function useModalCaret(
	value: string,
	onCaretChange?: (position: number) => void,
	controlledCaret?: number,
) {
	const [storedCaret, setCaretState] = useState(value.length);
	const caret = Math.min(controlledCaret ?? storedCaret, value.length);
	const setCaret = (position: number, maximum = value.length) => {
		const next = Math.max(0, Math.min(position, maximum));
		setCaretState(next);
		onCaretChange?.(next);
	};
	return [caret, setCaret] as const;
}

function replaceAtCaret(
	value: string,
	caret: number,
	removeBefore: number,
	insertion: string,
) {
	const start = Math.max(0, caret - removeBefore);
	return {
		value: `${value.slice(0, start)}${insertion}${value.slice(caret)}`,
		caret: start + insertion.length,
	};
}

export function ModalCaretValue({
	value,
	caret,
	placeholder,
	multiline = false,
	secure = false,
	onCaretChange,
	ariaLabel,
}: {
	value: string;
	caret: number;
	placeholder?: string;
	multiline?: boolean;
	secure?: boolean;
	onCaretChange?: (position: number) => void;
	ariaLabel?: string;
}) {
	const position = Math.max(0, Math.min(caret, value.length));
	const display = secure ? "•".repeat(value.length) : value;
	const multilineInput = useRef<HTMLTextAreaElement>(null);
	useLayoutEffect(() => {
		if (!multiline || !multilineInput.current) return;
		if (
			multilineInput.current.selectionStart === position &&
			multilineInput.current.selectionEnd === position
		)
			return;
		multilineInput.current.focus({ preventScroll: true });
		multilineInput.current.setSelectionRange(position, position);
	}, [multiline, position]);
	if (multiline) {
		const syncCaret = (control: HTMLTextAreaElement) => {
			onCaretChange?.(control.selectionStart);
		};
		return (
			<textarea
				ref={multilineInput}
				className={`modal-caret-value multiline ${!value ? "is-empty" : ""}`}
				aria-label={
					ariaLabel ?? `Input value ${value || placeholder || "empty"}`
				}
				aria-placeholder={placeholder}
				value={display}
				placeholder={placeholder}
				readOnly
				spellCheck={false}
				onClick={(event) => syncCaret(event.currentTarget)}
				onPointerUp={(event) => syncCaret(event.currentTarget)}
				onKeyUp={(event) => syncCaret(event.currentTarget)}
				onSelect={(event) => syncCaret(event.currentTarget)}
			/>
		);
	}
	const placeCaret = (event: ReactPointerEvent<HTMLOutputElement>) => {
		if (!onCaretChange) return;
		const element = event.currentTarget;
		const point = document.caretPositionFromPoint?.(
			event.clientX,
			event.clientY,
		);
		if (point?.offsetNode && element.contains(point.offsetNode)) {
			const before = document.createRange();
			before.selectNodeContents(element);
			before.setEnd(point.offsetNode, point.offset);
			onCaretChange(
				Math.min(value.length, before.toString().replace("\u200b", "").length),
			);
			return;
		}
		const box = element.getBoundingClientRect();
		const fraction = box.width ? (event.clientX - box.left) / box.width : 1;
		onCaretChange(
			Math.round(Math.max(0, Math.min(1, fraction)) * value.length),
		);
	};
	return (
		<output
			className={`modal-caret-value ${multiline ? "multiline" : ""} ${!value ? "is-empty" : ""}`}
			role="textbox"
			aria-label={ariaLabel ?? `Input value ${value || placeholder || "empty"}`}
			aria-placeholder={placeholder}
			aria-multiline={multiline || undefined}
			tabIndex={0}
			onPointerDown={placeCaret}
		>
			{!value && placeholder ? (
				<span className="modal-value-placeholder">{placeholder}</span>
			) : (
				<span>{display.slice(0, position)}</span>
			)}
			<i aria-hidden="true" />
			<span>{display.slice(position)}</span>
		</output>
	);
}

export function ModalNumberValue({
	value,
	caret,
	placeholder,
	unit,
	onCaretChange,
	ariaLabel,
}: {
	value: string;
	caret: number;
	placeholder?: string;
	unit?: ReactNode;
	onCaretChange: (position: number) => void;
	ariaLabel?: string;
}) {
	const moveCaret = (offset: number) => {
		onCaretChange(Math.max(0, Math.min(value.length, caret + offset)));
	};
	return (
		<div className="modal-number-value">
			<ModalCaretValue
				value={value}
				caret={caret}
				placeholder={placeholder}
				onCaretChange={onCaretChange}
				ariaLabel={ariaLabel}
			/>
			{unit !== undefined && <span aria-label="Unit">{unit}</span>}
			<div className="modal-number-cursors" aria-label="Cursor position">
				<Button
					className="action cursor-left"
					aria-label="Move cursor left"
					onClick={() => moveCaret(-1)}
				>
					←
				</Button>
				<Button
					className="action cursor-right"
					aria-label="Move cursor right"
					onClick={() => moveCaret(1)}
				>
					→
				</Button>
			</div>
		</div>
	);
}

const numberPadRows = [
	["ESC", "7", "8", "9", "⌫"],
	["+", "4", "5", "6", "THRU"],
	["DIV", "1", "2", "3", "ENTER"],
	["−", ".", "0", "AT"],
];

function NumberPadView({
	root,
	press,
	allowDecimal,
	allowThrough,
}: {
	root: ReturnType<typeof useModalInput>;
	press: (key: string) => void;
	allowDecimal: boolean;
	allowThrough: boolean;
}) {
	return (
		<div
			ref={root}
			className="modal-number-input numeric-pad"
			aria-label="Number input keypad"
		>
			{numberPadRows.flatMap((row, rowIndex) =>
				row.map((key, columnIndex) => {
					if (
						(key === "." && !allowDecimal) ||
						(key === "THRU" && !allowThrough)
					)
						return null;
					const action = ["ESC", "THRU", "DIV", "AT", "+", "−", "⌫"].includes(
						key,
					);
					return (
						<Button
							data-keypad-key={key}
							key={key}
							style={{
								gridColumn: columnIndex + 1,
								gridRow:
									key === "ENTER" ? `${rowIndex + 1} / span 2` : rowIndex + 1,
							}}
							onClick={() =>
								press(
									key === "ENTER" ? "Enter" : key === "ESC" ? "Escape" : key,
								)
							}
							className={
								key === "ENTER"
									? "enter modal-number-input-enter"
									: action
										? "action"
										: ""
							}
						>
							{key}
						</Button>
					);
				}),
			)}
		</div>
	);
}

export function ModalNumberInput({
	value,
	caret: controlledCaret,
	onChange,
	onEnter,
	onEscape,
	onCaretChange,
	replaceOnFirstInput = false,
	allowDecimal = true,
	allowThrough = false,
}: ModalInputProps & {
	replaceOnFirstInput?: boolean;
	allowDecimal?: boolean;
	allowThrough?: boolean;
}) {
	const replace = useRef(replaceOnFirstInput);
	const previousControlledCaret = useRef(controlledCaret);
	const [caret, setCaret] = useModalCaret(
		value,
		onCaretChange,
		controlledCaret,
	);
	useEffect(() => {
		if (
			controlledCaret !== undefined &&
			previousControlledCaret.current !== undefined &&
			controlledCaret !== previousControlledCaret.current
		) {
			replace.current = false;
		}
		previousControlledCaret.current = controlledCaret;
	}, [controlledCaret]);
	const update = (next: { value: string; caret: number }) => {
		replace.current = false;
		onChange(next.value);
		setCaret(next.caret, next.value.length);
	};
	const press = (key: string) => {
		if (key === "Escape") return onEscape();
		if (key === "Enter") return onEnter();
		if (key === "ArrowLeft") {
			replace.current = false;
			return setCaret(caret - 1);
		}
		if (key === "ArrowRight") {
			replace.current = false;
			return setCaret(caret + 1);
		}
		if (key === "Backspace" || key === "⌫") {
			return update(
				replace.current
					? { value: "", caret: 0 }
					: replaceAtCaret(value, caret, caret > 0 ? 1 : 0, ""),
			);
		}
		if (key === "−" || key === "-") {
			const next = replace.current
				? { value: "-", caret: 1 }
				: value.startsWith("-")
					? { value: value.slice(1), caret: Math.max(0, caret - 1) }
					: { value: `-${value || "0"}`, caret: caret + 1 };
			return update(next);
		}
		if (key === "+") {
			return update(
				value.startsWith("-")
					? { value: value.slice(1), caret: Math.max(0, caret - 1) }
					: { value, caret },
			);
		}
		if (key === "THRU") {
			const before = value.slice(0, caret);
			const after = value.slice(caret);
			if (
				!allowThrough ||
				replace.current ||
				!before.trim() ||
				(!after.trim() && /\bTHRU\s*$/i.test(before))
			)
				return;
			return update(replaceAtCaret(value, caret, 0, " THRU "));
		}
		if (/^\d$/.test(key)) {
			return update(
				replace.current
					? { value: key, caret: 1 }
					: replaceAtCaret(value, caret, 0, key),
			);
		}
		if (allowDecimal && key === ".") {
			const tokenBefore =
				value
					.slice(0, caret)
					.split(/\s+THRU\s+/i)
					.at(-1) ?? "";
			const tokenAfter = value.slice(caret).split(/\s+THRU\s+/i)[0] ?? "";
			if (replace.current) return update({ value: "0.", caret: 2 });
			if (!`${tokenBefore}${tokenAfter}`.includes(".")) {
				return update(
					replaceAtCaret(value, caret, 0, tokenBefore ? "." : "0."),
				);
			}
		}
	};
	const root = useModalInput(press);
	return (
		<NumberPadView
			root={root}
			press={press}
			allowDecimal={allowDecimal}
			allowThrough={allowThrough}
		/>
	);
}

const physicalRows = [
	[
		"Digit1",
		"Digit2",
		"Digit3",
		"Digit4",
		"Digit5",
		"Digit6",
		"Digit7",
		"Digit8",
		"Digit9",
		"Digit0",
		"Minus",
		"Equal",
	],
	[
		"KeyQ",
		"KeyW",
		"KeyE",
		"KeyR",
		"KeyT",
		"KeyY",
		"KeyU",
		"KeyI",
		"KeyO",
		"KeyP",
		"BracketLeft",
		"BracketRight",
	],
	[
		"KeyA",
		"KeyS",
		"KeyD",
		"KeyF",
		"KeyG",
		"KeyH",
		"KeyJ",
		"KeyK",
		"KeyL",
		"Semicolon",
		"Quote",
	],
	[
		"KeyZ",
		"KeyX",
		"KeyC",
		"KeyV",
		"KeyB",
		"KeyN",
		"KeyM",
		"Comma",
		"Period",
		"Slash",
	],
];

const qwertyValues: Record<string, string> = {
	Digit1: "1",
	Digit2: "2",
	Digit3: "3",
	Digit4: "4",
	Digit5: "5",
	Digit6: "6",
	Digit7: "7",
	Digit8: "8",
	Digit9: "9",
	Digit0: "0",
	Minus: "-",
	Equal: "=",
	KeyQ: "Q",
	KeyW: "W",
	KeyE: "E",
	KeyR: "R",
	KeyT: "T",
	KeyY: "Y",
	KeyU: "U",
	KeyI: "I",
	KeyO: "O",
	KeyP: "P",
	BracketLeft: "[",
	BracketRight: "]",
	KeyA: "A",
	KeyS: "S",
	KeyD: "D",
	KeyF: "F",
	KeyG: "G",
	KeyH: "H",
	KeyJ: "J",
	KeyK: "K",
	KeyL: "L",
	Semicolon: ";",
	Quote: "'",
	KeyZ: "Z",
	KeyX: "X",
	KeyC: "C",
	KeyV: "V",
	KeyB: "B",
	KeyN: "N",
	KeyM: "M",
	Comma: ",",
	Period: ".",
	Slash: "/",
};

const shiftedValues: Record<string, string> = {
	Digit1: "!",
	Digit2: "@",
	Digit3: "#",
	Digit4: "$",
	Digit5: "%",
	Digit6: "^",
	Digit7: "&",
	Digit8: "*",
	Digit9: "(",
	Digit0: ")",
	Minus: "_",
	Equal: "+",
	BracketLeft: "{",
	BracketRight: "}",
	Semicolon: ":",
	Quote: '"',
	Comma: "<",
	Period: ">",
	Slash: "?",
};

export function fallbackKeyboardLayout(language = "en") {
	if (!language.toLowerCase().startsWith("de")) return qwertyValues;
	return {
		...qwertyValues,
		KeyY: "Z",
		KeyZ: "Y",
		Minus: "ß",
		BracketLeft: "Ü",
		Semicolon: "Ö",
		Quote: "Ä",
		BracketRight: "+",
	};
}

function displayKey(value: string) {
	return value.length === 1 && value !== "ß"
		? value.toLocaleUpperCase()
		: value;
}

type ShiftState = "inactive" | "one-shot" | "locked";

function useKeyboardLayout() {
	const [layout, setLayout] = useState<Record<string, string>>(() =>
		fallbackKeyboardLayout(navigator.language),
	);
	useEffect(() => {
		let cancelled = false;
		const keyboard = (
			navigator as Navigator & {
				keyboard?: { getLayoutMap?: () => Promise<Map<string, string>> };
			}
		).keyboard;
		if (keyboard?.getLayoutMap)
			void keyboard
				.getLayoutMap()
				.then((map) => {
					if (cancelled) return;
					const fallback = fallbackKeyboardLayout(navigator.language);
					setLayout(
						Object.fromEntries(
							physicalRows
								.flat()
								.map((code) => [code, map.get(code) ?? fallback[code] ?? ""]),
						),
					);
				})
				.catch(() => undefined);
		return () => {
			cancelled = true;
		};
	}, []);
	return layout;
}

function useKeyboardShift() {
	const [shift, setShift] = useState<ShiftState>("inactive");
	const holdTimer = useRef<number | null>(null);
	const holdLocked = useRef(false);
	const cancelHold = () => {
		if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
		holdTimer.current = null;
	};
	const toggle = () =>
		setShift((current) => (current === "inactive" ? "one-shot" : "inactive"));
	const beginHold = () => {
		holdLocked.current = false;
		cancelHold();
		holdTimer.current = window.setTimeout(() => {
			holdLocked.current = true;
			setShift("locked");
		}, 500);
	};
	const endHold = () => {
		cancelHold();
		if (!holdLocked.current) toggle();
	};
	const consume = () => {
		if (shift !== "one-shot") return;
		cancelHold();
		setShift("inactive");
	};
	useEffect(() => cancelHold, []);
	useEffect(() => {
		const down = (event: KeyboardEvent) => {
			if (event.key !== "Shift" || event.repeat) return;
			if (shift === "locked") return setShift("inactive");
			setShift("one-shot");
			beginHold();
		};
		const up = (event: KeyboardEvent) => {
			if (event.key === "Shift") cancelHold();
		};
		window.addEventListener("keydown", down, true);
		window.addEventListener("keyup", up, true);
		return () => {
			window.removeEventListener("keydown", down, true);
			window.removeEventListener("keyup", up, true);
		};
	}, [shift]);
	return { shift, toggle, beginHold, endHold, cancelHold, consume };
}

function ShiftKey({
	controls,
}: {
	controls: ReturnType<typeof useKeyboardShift>;
}) {
	return (
		<Button
			className={`shift shift-${controls.shift}`}
			aria-label="Shift"
			aria-pressed={controls.shift !== "inactive"}
			data-shift-state={controls.shift}
			onClick={(event) => {
				if (event.detail === 0) controls.toggle();
			}}
			onPointerDown={controls.beginHold}
			onPointerUp={controls.endHold}
			onPointerCancel={controls.cancelHold}
		>
			<svg className="modal-shift-icon" viewBox="0 0 24 24" aria-hidden="true">
				<path d="M12 3 4.5 11h4v7h7v-7h4L12 3Z" />
				<path d="M8 21h8" />
			</svg>
		</Button>
	);
}

function TextKeyboardView({
	root,
	multiline,
	actionLabel,
	press,
	keyValue,
	shiftControls,
}: {
	root: ReturnType<typeof useModalInput>;
	multiline: boolean;
	actionLabel: string;
	press: (key: string) => void;
	keyValue: (code: string) => string;
	shiftControls: ReturnType<typeof useKeyboardShift>;
}) {
	const key = (code: string) => (
		<Button
			data-keyboard-code={code}
			key={code}
			onClick={() => press(keyValue(code))}
		>
			{displayKey(keyValue(code))}
		</Button>
	);
	return (
		<div
			ref={root}
			className="modal-text-keyboard"
			aria-label="Full text keyboard"
		>
			<div className="modal-keyboard-main">
				<div className="modal-keyboard-row row-1">
					<Button className="escape" onClick={() => press("Escape")}>
						<b>ESC</b>
						<small>Cancel</small>
					</Button>
					{physicalRows[0].map(key)}
				</div>
				{physicalRows.slice(1).map((row, index) => (
					<div className={`modal-keyboard-row row-${index + 2}`} key={index}>
						{index === 2 ? <ShiftKey controls={shiftControls} /> : null}
						{row.map(key)}
					</div>
				))}
				<div className="modal-keyboard-row modal-keyboard-bottom">
					<Button
						className="action cursor-left"
						aria-label="Move cursor left"
						onClick={() => press("ArrowLeft")}
					>
						←
					</Button>
					<Button
						className="action cursor-right"
						aria-label="Move cursor right"
						onClick={() => press("ArrowRight")}
					>
						→
					</Button>
					<span className="modal-keyboard-gap" aria-hidden="true" />
					<Button className="space" onClick={() => press("SPACE")}>
						SPACE
					</Button>
				</div>
			</div>
			<div className={`modal-keyboard-actions ${multiline ? "multiline" : ""}`}>
				{multiline ? (
					<>
						<Button
							className="action backspace"
							onClick={() => press("Backspace")}
						>
							<b>⌫</b>
							<small>Backspace</small>
						</Button>
						<Button
							className="action newline"
							aria-label="Enter · New line"
							onClick={() => press("Enter")}
						>
							<b>ENTER</b>
							<small>New line</small>
						</Button>
					</>
				) : (
					<>
						<Button
							className="action backspace"
							aria-label="Backspace"
							onClick={() => press("Backspace")}
						>
							<b>⌫</b>
							<small>Backspace</small>
						</Button>
						<Button
							className="enter"
							aria-label={`Enter · ${actionLabel}`}
							onClick={() => press("Enter")}
						>
							<b>ENTER</b>
							<small>{actionLabel}</small>
						</Button>
					</>
				)}
			</div>
		</div>
	);
}

export function ModalTextKeyboard({
	value,
	caret: controlledCaret,
	onChange,
	onEnter,
	onEscape,
	onCaretChange,
	actionLabel = "Confirm",
	multiline = false,
}: ModalInputProps & { actionLabel?: string; multiline?: boolean }) {
	const layout = useKeyboardLayout();
	const [caret, setCaret] = useModalCaret(
		value,
		onCaretChange,
		controlledCaret,
	);
	const shiftControls = useKeyboardShift();
	const update = (next: { value: string; caret: number }) => {
		onChange(next.value);
		const position = Math.max(0, Math.min(next.caret, next.value.length));
		setCaret(position, next.value.length);
	};
	const press = (key: string) => {
		if (key === "Escape") return onEscape();
		if (key === "Enter")
			return multiline
				? update(replaceAtCaret(value, caret, 0, "\n"))
				: onEnter();
		if (key === "Confirm") return onEnter();
		if (key === "ArrowLeft") return setCaret(caret - 1);
		if (key === "ArrowRight") return setCaret(caret + 1);
		if (key === "Backspace" || key === "⌫") {
			return update(replaceAtCaret(value, caret, caret > 0 ? 1 : 0, ""));
		}
		if (key === "SPACE") return update(replaceAtCaret(value, caret, 0, " "));
		if (key.length === 1) {
			const next =
				shiftControls.shift === "inactive"
					? key
					: key.length === 1
						? key.toLocaleUpperCase()
						: key;
			update(replaceAtCaret(value, caret, 0, next));
			shiftControls.consume();
		}
	};
	const keyValue = (code: string) => {
		const normal = layout[code] ?? "";
		if (shiftControls.shift === "inactive")
			return /^[A-Z]$/u.test(normal) ? normal.toLocaleLowerCase() : normal;
		return (
			shiftedValues[code] ??
			(normal.length === 1 ? normal.toLocaleUpperCase() : normal)
		);
	};
	const root = useModalInput(press);
	return (
		<TextKeyboardView
			root={root}
			multiline={multiline}
			actionLabel={actionLabel}
			press={press}
			keyValue={keyValue}
			shiftControls={shiftControls}
		/>
	);
}
