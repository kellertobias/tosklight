import {
	type CSSProperties,
	type PointerEvent as ReactPointerEvent,
	type WheelEvent as ReactWheelEvent,
	useEffect,
	useRef,
	useState,
} from "react";
import { createPortal } from "react-dom";
import { Button } from "../common";
import {
	ModalNumberInput,
	submitEncoderValue,
} from "../input/ModalInputControls";

const CONTINUOUS_INTERVAL_MILLIS = 80;
const DRAG_DEAD_ZONE_PX = 8;

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

function TouchEncoderZones({
	disabled,
	indexed,
	canRelease,
	label,
	onStep,
	onSet,
	onRelease,
}: {
	disabled: boolean;
	indexed: boolean;
	canRelease: boolean;
	label: string;
	onStep(delta: number): void;
	onSet(): void;
	onRelease?(): void;
}) {
	return (
		<>
			<div className="touch-encoder-zones">
				<Button disabled={disabled || indexed} onClick={() => onStep(0.1)}>
					+10
				</Button>
				<Button disabled={disabled || indexed} onClick={() => onStep(0.01)}>
					+1
				</Button>
				<Button
					className="touch-encoder-set"
					disabled={disabled || indexed}
					onClick={onSet}
				>
					Set Value
				</Button>
				<Button disabled={disabled || indexed} onClick={() => onStep(-0.01)}>
					−1
				</Button>
				<Button disabled={disabled || indexed} onClick={() => onStep(-0.1)}>
					−10
				</Button>
			</div>
			{indexed && <small className="touch-encoder-constraint">Indexed value</small>}
			{canRelease && onRelease && (
				<Button
					className="touch-encoder-release"
					aria-label={`Release ${label.replace(/^Enc \d+ · /, "")}`}
					onClick={onRelease}
				>
					Release
				</Button>
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
	return createPortal(
		<div
			className="stacked-modal-layer"
			onPointerDown={(event) => event.target === event.currentTarget && onClose()}
		>
			<section
				className="nested-modal direct-value-modal hardware-encoder-modal"
				role="dialog"
				aria-modal="true"
				aria-label={`${label} value`}
			>
				<Button
					className="modal-close"
					aria-label="Close encoder value"
					onClick={onClose}
				>
					×
				</Button>
				<h3>{label}</h3>
				<strong>{inputValue || "0"}</strong>
				<ModalNumberInput
					value={inputValue}
					onChange={onInput}
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
			</section>
		</div>,
		document.body,
	);
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
	const drag = useRef<DragState | null>(null);
	const interval = useRef<number | null>(null);
	const suppressClick = useRef(false);

	const stopContinuous = () => {
		if (interval.current !== null) window.clearInterval(interval.current);
		interval.current = null;
	};
	useEffect(() => stopContinuous, []);

	const beginContinuous = () => {
		if (interval.current !== null || !drag.current?.delta) return;
		interval.current = window.setInterval(() => {
			const current = drag.current;
			if (current?.delta)
				onStep(current.delta, current.undoGroup);
		}, CONTINUOUS_INTERVAL_MILLIS);
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
		// Capture on the actual zone button so a stationary tap still dispatches its click.
		// Capturing on the card retargets pointer-up to the section and silently loses the zone.
		(event.target as HTMLElement).setPointerCapture?.(event.pointerId);
	};
	const onPointerMove = (event: ReactPointerEvent<HTMLElement>) => {
		const current = drag.current;
		if (!current || current.pointerId !== event.pointerId) return;
		const displacement = current.startY - event.clientY;
		if (Math.abs(displacement) < DRAG_DEAD_ZONE_PX) {
			current.delta = 0;
			return;
		}
		current.moved = true;
		const magnitude = Math.abs(displacement);
		const step = magnitude < 35 ? 0.0025 : magnitude < 80 ? 0.01 : 0.025;
		current.delta = Math.sign(displacement) * step;
		beginContinuous();
		event.preventDefault();
	};
	const finishPointer = (event: ReactPointerEvent<HTMLElement>) => {
		const current = drag.current;
		if (!current || current.pointerId !== event.pointerId) return;
		suppressClick.current = current.moved;
		drag.current = null;
		stopContinuous();
		if (suppressClick.current)
			window.setTimeout(() => {
				suppressClick.current = false;
			}, 0);
	};
	const step = (delta: number) => {
		if (disabled || indexed || suppressClick.current) return;
		onStep(delta);
	};
	const openEditor = () => {
		if (disabled || indexed || suppressClick.current) return;
		setInputValue(String(Number((value * 100).toFixed(1))));
		setEditing(true);
	};
	const submit = () => {
		if (
			submitEncoderValue(
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
	return (
		<>
			<section
				role="region"
				className={`touch-encoder ${disabled ? "disabled" : ""} ${indexed ? "indexed" : ""}`}
				style={{ "--encoder-color": accentColor ?? "#176777" } as CSSProperties}
				aria-label={label}
				onPointerDown={onPointerDown}
				onPointerMove={onPointerMove}
				onPointerUp={finishPointer}
				onPointerCancel={finishPointer}
				onWheel={onWheel}
			>
				<header>
					<span>{label}</span>
					<strong aria-live="polite">{display}</strong>
					{mode && <small>{mode}</small>}
				</header>
				<TouchEncoderZones
					disabled={disabled}
					indexed={indexed}
					canRelease={canRelease}
					label={label}
					onStep={step}
					onSet={openEditor}
					onRelease={onRelease}
				/>
			</section>
			{editing &&
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
				/>}
		</>
	);
}
