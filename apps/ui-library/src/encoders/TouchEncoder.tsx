import {
	type CSSProperties,
	type KeyboardEvent as ReactKeyboardEvent,
	type PointerEvent as ReactPointerEvent,
	type WheelEvent as ReactWheelEvent,
	useEffect,
	useRef,
	useState,
} from "react";
import {
	ModalNumberEditor,
	type ModalNumberPresetConfig,
} from "../input/ModalNumberEditor";
import { submitNumericExpression } from "../input/numericExpression";

export const TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS = 80;
export const TOUCH_ENCODER_DRAG_DEAD_ZONE_PX = 8;
export const TOUCH_ENCODER_FINE_STEP = 0.001;
export const TOUCH_ENCODER_COARSE_STEP = 0.01;

export type TouchEncoderInteraction = "continuous" | "choices";

export interface TouchEncoderProps {
	label: string;
	slot?: number;
	attributeLabel?: string;
	display?: string;
	value: number;
	formatValue?(value: number): string;
	minimum?: number;
	maximum?: number;
	/** Converts the internal value to the number shown in the absolute-entry modal. */
	inputScale?: number;
	slowStep?: number;
	fastStep?: number;
	repeatSeconds?: number;
	disabled?: boolean;
	accentColor?: string;
	mode?: string;
	indexed?: boolean;
	canRelease?: boolean;
	presets?: ModalNumberPresetConfig;
	touchInteraction?: TouchEncoderInteraction;
	onStep(delta: number, undoGroup?: string | null): void;
	onSet(value: number): void;
	onSetRange?(points: number[]): void;
	onPresetSelect?(value: string): void;
	onRelease?(): void;
}

interface DragState {
	pointerId: number;
	startY: number;
	maximumDisplacement: number;
	delta: number;
	undoGroup: string;
	moved: boolean;
}

interface EncoderMotion {
	direction: "up" | "down";
	ridgeCyclesPerSecond: number;
}

const TOUCH_ENCODER_RIDGE_CYCLE_PX = 23;

function useContinuousRidgeMotion(motion: EncoderMotion | null) {
	const encoder = useRef<HTMLElement | null>(null);
	const animationFrame = useRef<number | null>(null);
	const lastFrameTime = useRef<number | null>(null);
	const motionState = useRef(motion);
	const ridgeOffset = useRef(0);
	motionState.current = motion;
	const moving = motion !== null;

	useEffect(() => {
		if (!moving) {
			lastFrameTime.current = null;
			return;
		}
		const animate = (time: number) => {
			const previousTime = lastFrameTime.current;
			lastFrameTime.current = time;
			const currentMotion = motionState.current;
			if (previousTime !== null && currentMotion) {
				const direction = currentMotion.direction === "up" ? -1 : 1;
				const elapsedSeconds = Math.min(0.1, (time - previousTime) / 1000);
				ridgeOffset.current +=
					direction *
					currentMotion.ridgeCyclesPerSecond *
					TOUCH_ENCODER_RIDGE_CYCLE_PX *
					elapsedSeconds;
				encoder.current?.style.setProperty(
					"--encoder-ridge-offset",
					`${ridgeOffset.current}px`,
				);
			}
			animationFrame.current = window.requestAnimationFrame(animate);
		};
		animationFrame.current = window.requestAnimationFrame(animate);
		return () => {
			if (animationFrame.current !== null)
				window.cancelAnimationFrame(animationFrame.current);
			animationFrame.current = null;
			lastFrameTime.current = null;
		};
	}, [moving]);

	return encoder;
}

function TouchEncoderSurface({
	attributeLabel,
	disabled,
	display,
	indexed,
	label,
	onStep,
	onSet,
	choiceMode,
	hasChoices,
	slot,
	slowStep,
}: {
	attributeLabel?: string;
	disabled: boolean;
	display: string;
	indexed: boolean;
	label: string;
	onStep(delta: number): void;
	onSet(): void;
	choiceMode: boolean;
	hasChoices: boolean;
	slot?: number;
	slowStep: number;
}) {
	const range = display.match(/^(.*?)\s*(?:\.\.\.|…)\s*(.*?)$/u);
	return (
		<div className="touch-encoder-surface">
			{(attributeLabel || slot !== undefined) && (
				<header className="touch-encoder-labels">
					<b title={attributeLabel}>{attributeLabel}</b>
					{slot !== undefined && <small>Enc {slot}</small>}
				</header>
			)}
			{choiceMode ? (
				<button
					type="button"
					className="touch-encoder-choice-step touch-encoder-tap-positive"
					aria-label={`Next ${label} value`}
					disabled={disabled}
					onClick={() => onStep(slowStep)}
				>
					<svg viewBox="0 0 24 14" aria-hidden="true">
						<path d="m4 11 8-8 8 8" />
					</svg>
				</button>
			) : (
				<>
					<span className="touch-encoder-ridges" aria-hidden="true" />
					<div
						className="touch-encoder-tap-zone touch-encoder-tap-positive"
						aria-hidden="true"
						onClick={() => onStep(slowStep)}
					/>
				</>
			)}
			<button
				type="button"
				className={`touch-encoder-value ${range ? "range-value" : ""}`.trim()}
				aria-label={`Set ${label} value`}
				disabled={disabled || indexed || (choiceMode && !hasChoices)}
				onClick={onSet}
			>
				{range ? (
					<>
						<span>{range[1]}</span>
						<i aria-hidden="true">...</i>
						<span>{range[2]}</span>
					</>
				) : (
					display
				)}
			</button>
			{choiceMode ? (
				<button
					type="button"
					className="touch-encoder-choice-step touch-encoder-tap-negative"
					aria-label={`Previous ${label} value`}
					disabled={disabled}
					onClick={() => onStep(-slowStep)}
				>
					<svg viewBox="0 0 24 14" aria-hidden="true">
						<path d="m4 3 8 8 8-8" />
					</svg>
				</button>
			) : (
				<>
					<div
						className="touch-encoder-tap-zone touch-encoder-tap-negative"
						aria-hidden="true"
						onClick={() => onStep(-slowStep)}
					/>
					<div className="touch-encoder-legend" aria-hidden="true">
						<span>Increase</span>
						<i>•••</i>
						<strong>Set</strong>
						<i>•••</i>
						<span>Decrease</span>
					</div>
				</>
			)}
		</div>
	);
}

function TouchEncoderEditor({
	label,
	inputValue,
	allowThrough,
	canRelease,
	presets,
	presetsOnly,
	onInput,
	onSubmit,
	onClose,
	onPresetSelect,
	onRelease,
}: {
	label: string;
	inputValue: string;
	allowThrough: boolean;
	canRelease: boolean;
	presets?: ModalNumberPresetConfig;
	presetsOnly: boolean;
	onInput(value: string): void;
	onSubmit(value?: string): void;
	onClose(): void;
	onPresetSelect?(value: string): void;
	onRelease?(): void;
}) {
	return (
		<ModalNumberEditor
			ariaLabel={`${label} value`}
			dialogClassName="direct-value-modal hardware-encoder-modal"
			title={label}
			value={inputValue}
			onChange={onInput}
			onSubmit={onSubmit}
			onClose={onClose}
			allowThrough={allowThrough}
			presets={presets}
			presetsOnly={presetsOnly}
			onPresetSelect={onPresetSelect}
			onRelease={canRelease ? onRelease : undefined}
		/>
	);
}

function useTouchEncoderInteraction({
	disabled,
	fastStep,
	indexed,
	onStep,
	repeatSeconds,
	slowStep,
	continuous,
}: Pick<TouchEncoderProps, "disabled" | "indexed" | "onStep"> & {
	fastStep: number;
	repeatSeconds: number;
	slowStep: number;
	continuous: boolean;
}) {
	const [motion, setMotion] = useState<EncoderMotion | null>(null);
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
		interval.current = window.setInterval(
			() => {
				const current = drag.current;
				if (current?.delta) onStep(current.delta, current.undoGroup);
			},
			Math.max(1, repeatSeconds * 1000),
		);
	};
	const onPointerDown = (event: ReactPointerEvent<HTMLElement>) => {
		if (!continuous || disabled || indexed || event.button !== 0) return;
		drag.current = {
			pointerId: event.pointerId,
			startY: event.clientY,
			maximumDisplacement: Math.max(
				TOUCH_ENCODER_DRAG_DEAD_ZONE_PX + 1,
				(event.currentTarget.getBoundingClientRect().height || 320) / 2,
			),
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
			setMotion(null);
			return;
		}
		current.moved = true;
		event.currentTarget.setPointerCapture?.(event.pointerId);
		const travelRange = Math.max(
			1,
			current.maximumDisplacement - TOUCH_ENCODER_DRAG_DEAD_ZONE_PX,
		);
		const intensity = Math.max(
			0,
			Math.min(
				1,
				(Math.abs(displacement) - TOUCH_ENCODER_DRAG_DEAD_ZONE_PX) /
					travelRange,
			),
		);
		const stepMagnitude = slowStep + (fastStep - slowStep) * intensity;
		current.delta = Math.sign(displacement) * stepMagnitude;
		const direction = displacement > 0 ? "up" : "down";
		const ridgeCyclesPerSecond = 0.75 + intensity * 5.25;
		setMotion({
			direction,
			ridgeCyclesPerSecond,
		});
		beginContinuous();
		event.preventDefault();
	};
	const finishPointer = (event: ReactPointerEvent<HTMLElement>) => {
		const current = drag.current;
		if (!current || current.pointerId !== event.pointerId) return;
		suppressClick.current = current.moved;
		drag.current = null;
		stopContinuous();
		setMotion(null);
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
		finishPointer,
		motion,
		onPointerDown,
		onPointerMove,
		step,
	};
}

export function TouchEncoder({
	label,
	slot,
	attributeLabel,
	display,
	value,
	formatValue,
	minimum = 0,
	maximum = 1,
	inputScale = 100,
	slowStep = TOUCH_ENCODER_FINE_STEP,
	fastStep = TOUCH_ENCODER_COARSE_STEP,
	repeatSeconds = TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS / 1000,
	disabled = false,
	accentColor,
	indexed = false,
	canRelease = false,
	presets,
	touchInteraction = "continuous",
	onStep,
	onSet,
	onSetRange,
	onPresetSelect,
	onRelease,
}: TouchEncoderProps) {
	const [editing, setEditing] = useState(false);
	const [inputValue, setInputValue] = useState("");
	const choiceMode = touchInteraction === "choices";
	const hasChoices = Boolean(
		presets?.groups.some((group) => group.options.length),
	);
	const unavailable = disabled || indexed || (choiceMode && !hasChoices);
	const interaction = useTouchEncoderInteraction({
		continuous: !choiceMode,
		disabled: unavailable,
		fastStep,
		indexed,
		onStep,
		repeatSeconds,
		slowStep,
	});
	const encoderRef = useContinuousRidgeMotion(interaction.motion);
	const resolvedInputScale =
		Number.isFinite(inputScale) && inputScale !== 0 ? inputScale : 1;
	const renderedValue = display ?? formatValue?.(value) ?? String(value);
	const clamp = (next: number) => Math.max(minimum, Math.min(maximum, next));
	const openEditor = () => {
		if (unavailable || !interaction.canActivate()) return;
		setInputValue(String(Number((value * resolvedInputScale).toFixed(4))));
		setEditing(true);
	};
	const submit = (candidate = inputValue) => {
		if (
			submitNumericExpression(
				candidate,
				(next) => onSet(clamp(next / resolvedInputScale)),
				onSetRange,
			)
		)
			setEditing(false);
	};
	const onWheel = (event: ReactWheelEvent<HTMLElement>) => {
		if (disabled || indexed || !event.deltaY) return;
		event.preventDefault();
		onStep(Math.sign(-event.deltaY) * (event.shiftKey ? fastStep : slowStep));
	};
	const onKeyDown = (event: ReactKeyboardEvent<HTMLElement>) => {
		if (disabled || indexed) return;
		if (event.key === "ArrowUp" || event.key === "ArrowRight") {
			event.preventDefault();
			onStep(slowStep);
		} else if (event.key === "ArrowDown" || event.key === "ArrowLeft") {
			event.preventDefault();
			onStep(-slowStep);
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
				ref={encoderRef}
				role="group"
				tabIndex={unavailable ? -1 : 0}
				className={`touch-encoder ${choiceMode ? "choice-encoder" : ""} ${disabled ? "disabled" : ""} ${indexed ? "indexed" : ""}`}
				data-motion={interaction.motion?.direction}
				style={
					{
						"--encoder-color": accentColor ?? "#176777",
						"--encoder-motion-speed": interaction.motion
							? interaction.motion.ridgeCyclesPerSecond
							: undefined,
					} as CSSProperties
				}
				aria-label={label}
				aria-describedby={instructionsId}
				aria-disabled={unavailable}
				onPointerDown={interaction.onPointerDown}
				onPointerMove={interaction.onPointerMove}
				onPointerUp={interaction.finishPointer}
				onPointerCancel={interaction.finishPointer}
				onWheel={onWheel}
				onKeyDown={onKeyDown}
			>
				<TouchEncoderSurface
					attributeLabel={attributeLabel}
					disabled={unavailable}
					display={renderedValue}
					choiceMode={choiceMode}
					hasChoices={hasChoices}
					indexed={indexed}
					label={label}
					onStep={interaction.step}
					onSet={openEditor}
					slot={slot}
					slowStep={slowStep}
				/>
				<span id={instructionsId} className="visually-hidden">
					{choiceMode
						? "Tap the upper chevron for the next value or the lower chevron for the previous value. Tap the center or press Enter to choose from the available options."
						: "The upper third increases the value and the lower third decreases it. Drag vertically to accelerate linearly. Tap the full-width center third or press Enter for absolute entry."}
				</span>
			</section>
			{editing && (
				<TouchEncoderEditor
					label={label}
					inputValue={inputValue}
					allowThrough={Boolean(onSetRange)}
					canRelease={canRelease}
					presets={presets}
					presetsOnly={choiceMode}
					onInput={setInputValue}
					onSubmit={submit}
					onClose={() => setEditing(false)}
					onPresetSelect={onPresetSelect}
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
