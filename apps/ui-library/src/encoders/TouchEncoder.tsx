import {
	type CSSProperties,
	type KeyboardEvent as ReactKeyboardEvent,
	type PointerEvent as ReactPointerEvent,
	type WheelEvent as ReactWheelEvent,
	useEffect,
	useRef,
	useState,
} from "react";
import { Button } from "../common";
import { ModalCaretValue, ModalNumberInput } from "../input/ModalInputControls";
import { ModalLayer } from "../modals/ModalStack";
import { submitNumericExpression } from "../input/numericExpression";

export const TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS = 80;
export const TOUCH_ENCODER_DRAG_DEAD_ZONE_PX = 8;
export const TOUCH_ENCODER_COARSE_THRESHOLD_PX = 48;
export const TOUCH_ENCODER_FINE_STEP = 0.01;
export const TOUCH_ENCODER_COARSE_STEP = 0.1;

export interface TouchEncoderProps {
	label: string;
	display: string;
	value: number;
	disabled?: boolean;
	accentColor?: string;
	mode?: string;
	indexed?: boolean;
	canRelease?: boolean;
	onStep(delta: number, undoGroup?: string | null): void;
	onSet(value: number): void;
	onSetRange?(points: number[]): void;
	onRelease?(): void;
}

interface DragState {
	pointerId: number;
	startY: number;
	delta: number;
	undoGroup: string;
	moved: boolean;
}

function TouchEncoderSurface({
	disabled,
	indexed,
	onStep,
	onSet,
}: {
	disabled: boolean;
	indexed: boolean;
	onStep(delta: number): void;
	onSet(): void;
}) {
	return (
		<>
			<div className="touch-encoder-surface">
				<div
					className="touch-encoder-tap-zone touch-encoder-tap-positive"
					aria-hidden="true"
					onClick={() => onStep(TOUCH_ENCODER_FINE_STEP)}
				>
					<span className="touch-encoder-drag-affordance" />
				</div>
				<Button
					className="touch-encoder-set"
					disabled={disabled || indexed}
					onClick={onSet}
				>
					Set Value
				</Button>
				<div
					className="touch-encoder-tap-zone touch-encoder-tap-negative"
					aria-hidden="true"
					onClick={() => onStep(-TOUCH_ENCODER_FINE_STEP)}
				/>
			</div>
			{indexed && (
				<small className="touch-encoder-constraint">Indexed value</small>
			)}
		</>
	);
}

function TouchEncoderEditor({
	label,
	inputValue,
	allowThrough,
	canRelease,
	onInput,
	onSubmit,
	onClose,
	onRelease,
}: {
	label: string;
	inputValue: string;
	allowThrough: boolean;
	canRelease: boolean;
	onInput(value: string): void;
	onSubmit(): void;
	onClose(): void;
	onRelease?(): void;
}) {
	const [caret, setCaret] = useState(inputValue.length);
	return (
		<ModalLayer
			ariaLabel={`${label} value`}
			dialogClassName="direct-value-modal hardware-encoder-modal"
			onClose={onClose}
		>
			<Button
				className="modal-close"
				aria-label="Close encoder value"
				onClick={onClose}
			>
				×
			</Button>
			<h3>{label}</h3>
			<ModalCaretValue value={inputValue} caret={caret} />
			<ModalNumberInput
				value={inputValue}
				onChange={onInput}
				onCaretChange={setCaret}
				onEnter={onSubmit}
				onEscape={onClose}
				replaceOnFirstInput
				allowThrough={allowThrough}
			/>
			{canRelease && onRelease && (
				<footer className="modal-actions">
					<Button variant="danger" onClick={onRelease}>
						Release
					</Button>
				</footer>
			)}
		</ModalLayer>
	);
}

function useTouchEncoderInteraction({
	disabled,
	indexed,
	onStep,
}: Pick<TouchEncoderProps, "disabled" | "indexed" | "onStep">) {
	const [dragFeedback, setDragFeedback] = useState<string | null>(null);
	const drag = useRef<DragState | null>(null);
	const interval = useRef<number | null>(null);
	const suppressClick = useRef(false);
	const stopContinuous = () => {
		if (interval.current !== null) window.clearInterval(interval.current);
		interval.current = null;
	};
	useEffect(
		() => () => {
			if (interval.current !== null) window.clearInterval(interval.current);
		},
		[],
	);
	const beginContinuous = () => {
		if (interval.current !== null || !drag.current?.delta) return;
		interval.current = window.setInterval(() => {
			const current = drag.current;
			if (current?.delta) onStep(current.delta, current.undoGroup);
		}, TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS);
	};
	const onPointerDown = (event: ReactPointerEvent<HTMLElement>) => {
		if (disabled || indexed || event.button !== 0) return;
		drag.current = {
			pointerId: event.pointerId,
			startY: event.clientY,
			delta: 0,
			undoGroup: crypto.randomUUID(),
			moved: false,
		};
	};
	const onPointerMove = (event: ReactPointerEvent<HTMLElement>) => {
		const current = drag.current;
		if (!current || current.pointerId !== event.pointerId) return;
		const displacement = current.startY - event.clientY;
		if (Math.abs(displacement) < TOUCH_ENCODER_DRAG_DEAD_ZONE_PX) {
			current.delta = 0;
			setDragFeedback(null);
			return;
		}
		current.moved = true;
		event.currentTarget.setPointerCapture?.(event.pointerId);
		const coarse = Math.abs(displacement) >= TOUCH_ENCODER_COARSE_THRESHOLD_PX;
		current.delta =
			Math.sign(displacement) *
			(coarse ? TOUCH_ENCODER_COARSE_STEP : TOUCH_ENCODER_FINE_STEP);
		setDragFeedback(
			`${displacement > 0 ? "Up" : "Down"} · ${coarse ? "Coarse" : "Fine"}`,
		);
		beginContinuous();
		event.preventDefault();
	};
	const finishPointer = (event: ReactPointerEvent<HTMLElement>) => {
		const current = drag.current;
		if (!current || current.pointerId !== event.pointerId) return;
		suppressClick.current = current.moved;
		drag.current = null;
		stopContinuous();
		setDragFeedback(null);
		if (suppressClick.current)
			window.setTimeout(() => {
				suppressClick.current = false;
			}, 0);
	};
	const step = (delta: number) => {
		if (!disabled && !indexed && !suppressClick.current) onStep(delta);
	};
	const canActivate = () => !disabled && !indexed && !suppressClick.current;
	return {
		canActivate,
		dragFeedback,
		finishPointer,
		onPointerDown,
		onPointerMove,
		step,
	};
}

export function TouchEncoder({
	label,
	display,
	value,
	disabled = false,
	accentColor,
	mode,
	indexed = false,
	canRelease = false,
	onStep,
	onSet,
	onSetRange,
	onRelease,
}: TouchEncoderProps) {
	const [editing, setEditing] = useState(false);
	const [inputValue, setInputValue] = useState("");
	const interaction = useTouchEncoderInteraction({ disabled, indexed, onStep });
	const openEditor = () => {
		if (!interaction.canActivate()) return;
		setInputValue(String(Number((value * 100).toFixed(1))));
		setEditing(true);
	};
	const submit = () => {
		if (
			submitNumericExpression(
				inputValue,
				(next) => onSet(Math.max(0, Math.min(100, next)) / 100),
				onSetRange,
			)
		)
			setEditing(false);
	};
	const onWheel = (event: ReactWheelEvent<HTMLElement>) => {
		if (disabled || indexed || !event.deltaY) return;
		event.preventDefault();
		onStep(Math.sign(-event.deltaY) * (event.shiftKey ? 0.1 : 0.01));
	};
	const onKeyDown = (event: ReactKeyboardEvent<HTMLElement>) => {
		if (disabled || indexed) return;
		if (event.key === "ArrowUp" || event.key === "ArrowRight") {
			event.preventDefault();
			onStep(TOUCH_ENCODER_FINE_STEP);
		} else if (event.key === "ArrowDown" || event.key === "ArrowLeft") {
			event.preventDefault();
			onStep(-TOUCH_ENCODER_FINE_STEP);
		} else if (event.key === "Enter" || event.key === " ") {
			event.preventDefault();
			openEditor();
		}
	};
	const instructionsId = `touch-encoder-${label.toLowerCase().replace(/[^a-z0-9]+/g, "-")}-instructions`;
	return (
		<>
			{/* biome-ignore lint/a11y/useSemanticElements: This is one focusable relative encoder, not a form fieldset. */}
			<section
				role="group"
				tabIndex={disabled || indexed ? -1 : 0}
				className={`touch-encoder ${disabled ? "disabled" : ""} ${indexed ? "indexed" : ""}`}
				style={{ "--encoder-color": accentColor ?? "#176777" } as CSSProperties}
				aria-label={label}
				aria-describedby={instructionsId}
				aria-disabled={disabled || indexed}
				onPointerDown={interaction.onPointerDown}
				onPointerMove={interaction.onPointerMove}
				onPointerUp={interaction.finishPointer}
				onPointerCancel={interaction.finishPointer}
				onWheel={onWheel}
				onKeyDown={onKeyDown}
			>
				<header>
					<span>{label}</span>
					<strong aria-live="polite">{display}</strong>
					{mode && <small>{mode}</small>}
				</header>
				<TouchEncoderSurface
					disabled={disabled}
					indexed={indexed}
					onStep={interaction.step}
					onSet={openEditor}
				/>
				<span id={instructionsId} className="visually-hidden">
					Upper and lower surface taps step the value. Drag vertically to
					accelerate. Press Enter for Set Value.
				</span>
				{interaction.dragFeedback && (
					<output className="touch-encoder-drag-feedback" aria-live="polite">
						{interaction.dragFeedback}
					</output>
				)}
			</section>
			{editing && (
				<TouchEncoderEditor
					label={label}
					inputValue={inputValue}
					allowThrough={Boolean(onSetRange)}
					canRelease={canRelease}
					onInput={setInputValue}
					onSubmit={submit}
					onClose={() => setEditing(false)}
					onRelease={
						onRelease
							? () => {
									onRelease();
									setEditing(false);
								}
							: undefined
					}
				/>
			)}
		</>
	);
}
