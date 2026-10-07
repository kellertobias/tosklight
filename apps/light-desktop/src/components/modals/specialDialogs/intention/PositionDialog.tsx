import { Button, ModalFrame } from "@tosklight/ui";
import { Input } from "@tosklight/ui/controls";
import { VerticalTouchFaderControl } from "@tosklight/ui/faders";
import {
	useCallback,
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
	type KeyboardEvent,
	type PointerEvent,
	type ReactNode,
} from "react";
import "./PositionDialog.css";

/** Finite limits and steps for one angle. `step` quantizes every emitted value. */
export interface PositionAxisLimits {
	minimum: number;
	maximum: number;
	step: number;
	/** Arrow-key increment; defaults to `step`. */
	keyStep?: number;
	/** Page Up/Down increment; defaults to one turn (360°) for Pan and ten key steps for Tilt. */
	largeKeyStep?: number;
}

/** Unwrapped Pan in degrees: values beyond ±180° are further turns, never wrapped. */
export interface PositionPanValue extends PositionAxisLimits {
	value: number;
}

/** Tilt in degrees. */
export interface PositionTiltValue extends PositionAxisLimits {
	value: number;
}

/**
 * Full-deflection joystick rates. The rates are a host capability; the dialog never guesses them.
 * Omit the descriptor to keep the joystick inert.
 */
export interface PositionJoystickRates {
	panDegreesPerSecond: number;
	tiltDegreesPerSecond: number;
}

export type PositionControl = "pan" | "tilt" | "joystick";
export type PositionGestureSource = "pointer" | "keyboard" | "button";
export type PositionCancelReason = "pointer-cancel" | "lost-capture" | "blur" | "hidden" | "close" | "superseded" | "teardown";

/** One coherent edit gesture. Every started gesture receives exactly one end or cancel. */
export interface PositionGesture {
	id: number;
	control: PositionControl;
	source: PositionGestureSource;
	pointerId?: number;
	/** The supplied angles when the gesture started. */
	initialPan: number;
	initialTilt: number;
}

/** Only the axes that this sample changed are present. */
export interface PositionChange {
	pan?: number;
	tilt?: number;
}

export interface PositionGestureResult {
	/** At least one change was emitted during the gesture. */
	changed: boolean;
}

export interface PositionDialogProps {
	pan: PositionPanValue;
	tilt: PositionTiltValue;
	joystick?: PositionJoystickRates;
	/** Requested angles. The dialog never stores them; render the new props. */
	onChange(change: PositionChange, gesture: PositionGesture): void;
	onGestureStart?(gesture: PositionGesture): void;
	/** The operator released the pointer or key, or a keyboard/button step completed. */
	onGestureEnd?(gesture: PositionGesture, result: PositionGestureResult): void;
	/** The gesture stopped without a release. `teardown` is unmount cleanup, never new authoring. */
	onGestureCancel?(gesture: PositionGesture, reason: PositionCancelReason, result: PositionGestureResult): void;
	onClose(): void;
	title?: string;
	ariaLabel?: string;
	closeLabel?: string;
	className?: string;
	dialogClassName?: string;
	/**
	 * Optional readout caption shown with the Pan and Tilt values, for example `Resolved` while the
	 * angles are the resolved commanded angles of the displayed output (TL-549). Omitted, nothing is
	 * rendered and the dialog is unchanged.
	 */
	valueCaption?: string;
	/**
	 * Return Home beside the relative (joystick) controls: one complete request the host turns into
	 * the selection's home pose. Omitted, no button is rendered and the dialog is unchanged.
	 */
	returnHome?: PositionReturnHome;
}

/** The Return Home action of the host. The dialog only reports the press. */
export interface PositionReturnHome {
	disabled?: boolean;
	onPress(): void;
}

interface ResolvedLimits {
	minimum: number;
	maximum: number;
	step: number;
	keyStep: number;
	largeKeyStep: number;
	decimals: number;
}

interface ActiveGesture {
	gesture: PositionGesture;
	element: Element;
	changed: boolean;
	/** Pan circle: last pointer angle, unquantized accumulated value and last emitted value. */
	angle: number;
	raw: number;
	last: number;
	/** Tilt fader: pointer-down point for the movement threshold. */
	x: number;
	y: number;
}

type Vector = { x: number; y: number };

const NEUTRAL: Vector = { x: 0, y: 0 };
/** No pointer turn is taken from the inner hub, where the angle is undefined. */
const PAN_HUB_RADIUS = 18;
/** Pointer movement below this distance from the fader press point does not move Tilt. */
const FADER_THRESHOLD = 3;
/** One frame never integrates more than this, so a stalled frame cannot produce a large jump. */
const MAX_FRAME_SECONDS = .05;
const JOYSTICK_DEAD_ZONE = .08;
const JOYSTICK_INSET = 26;

const bounded = (value: number, minimum: number, maximum: number) => Math.max(minimum, Math.min(maximum, value));
/** Gentle center: a dead zone, then a squared response that reaches full rate at the edge. */
const response = (value: number) => Math.abs(value) <= JOYSTICK_DEAD_ZONE ? 0 : Math.sign(value) * ((Math.abs(value) - JOYSTICK_DEAD_ZONE) / (1 - JOYSTICK_DEAD_ZONE)) ** 2;

function decimals(step: number) {
	if (!Number.isFinite(step) || step <= 0) return 0;
	const text = String(step);
	const exponent = text.match(/e-(\d+)$/);
	if (exponent) return Number(exponent[1]);
	return text.includes(".") ? text.length - text.indexOf(".") - 1 : 0;
}

/** Non-finite limits are an unknown host capability: the control stays inert instead of guessing. */
function resolveLimits(limits: PositionAxisLimits, largeDefault: (keyStep: number) => number): ResolvedLimits | null {
	if (!Number.isFinite(limits.minimum) || !Number.isFinite(limits.maximum)) return null;
	const minimum = Math.min(limits.minimum, limits.maximum);
	const maximum = Math.max(limits.minimum, limits.maximum);
	const step = Number.isFinite(limits.step) && limits.step > 0 ? limits.step : 0;
	const keyStep = limits.keyStep && Number.isFinite(limits.keyStep) && limits.keyStep > 0 ? limits.keyStep : step || (maximum - minimum) / 100 || 1;
	const large = limits.largeKeyStep && Number.isFinite(limits.largeKeyStep) && limits.largeKeyStep > 0 ? limits.largeKeyStep : largeDefault(keyStep);
	return { minimum, maximum, step, keyStep, largeKeyStep: large, decimals: decimals(step) };
}

function quantize(value: number, limits: ResolvedLimits) {
	const clamped = bounded(value, limits.minimum, limits.maximum);
	if (!limits.step) return clamped;
	const stepped = limits.minimum + Math.round((clamped - limits.minimum) / limits.step) * limits.step;
	return bounded(Number(stepped.toFixed(limits.decimals)), limits.minimum, limits.maximum);
}

const displayed = (value: number, limits: ResolvedLimits | null) => limits && Number.isFinite(value) ? bounded(value, limits.minimum, limits.maximum) : null;
const signed = (value: number, digits: number) => {
	const text = String(Number(Math.abs(value).toFixed(digits)));
	return value < 0 ? `−${text}°` : value > 0 ? `+${text}°` : "0°";
};
const validRates = (rates?: PositionJoystickRates) => !!rates && Number.isFinite(rates.panDegreesPerSecond) && Number.isFinite(rates.tiltDegreesPerSecond)
	&& (rates.panDegreesPerSecond !== 0 || rates.tiltDegreesPerSecond !== 0);
const join = (...names: (string | undefined)[]) => names.filter(Boolean).join(" ");

interface LifecycleState extends Pick<PositionDialogProps, "onChange" | "onGestureStart" | "onGestureEnd" | "onGestureCancel" | "onClose"> {
	pan: number | null;
	tilt: number | null;
	panLimits: ResolvedLimits | null;
	tiltLimits: ResolvedLimits | null;
	rates: PositionJoystickRates | null;
}

/** Shared lifecycle for every control: one active gesture, and exactly one end or cancel for each. */
function useGestureLifecycle(state: LifecycleState) {
	const latest = useRef(state);
	useLayoutEffect(() => {
		latest.current = state;
	});
	const active = useRef<ActiveGesture | null>(null);
	const nextId = useRef(1);
	const mounted = useRef(true);
	/** Per-control reset run when that control's gesture stops; the joystick stops its frames here. */
	const resets = useRef<Partial<Record<PositionControl, () => void>>>({});

	const stop = useCallback((outcome: "release" | PositionCancelReason) => {
		const current = active.current;
		if (!current) return;
		active.current = null;
		resets.current[current.gesture.control]?.();
		const pointerId = current.gesture.pointerId;
		if (pointerId !== undefined) {
			try {
				if (current.element.hasPointerCapture?.(pointerId)) current.element.releasePointerCapture(pointerId);
			} catch {
				// The element may already be detached during teardown.
			}
		}
		const result = { changed: current.changed };
		if (outcome === "release") latest.current.onGestureEnd?.(current.gesture, result);
		else latest.current.onGestureCancel?.(current.gesture, outcome, result);
	}, []);

	useEffect(() => {
		mounted.current = true;
		const blur = () => stop("blur");
		const hide = () => { if (document.hidden) stop("hidden"); };
		window.addEventListener("blur", blur);
		document.addEventListener("visibilitychange", hide);
		return () => {
			window.removeEventListener("blur", blur);
			document.removeEventListener("visibilitychange", hide);
			mounted.current = false;
			stop("teardown");
		};
	}, [stop]);

	/** Start a gesture; a different control's active gesture is superseded, the same control's is kept. */
	const begin = (control: PositionControl, source: PositionGestureSource, element: Element, pointerId?: number) => {
		const current = active.current;
		if (current) {
			if (current.gesture.control === control) return null;
			stop("superseded");
		}
		const { pan, tilt } = latest.current;
		const gesture: PositionGesture = { id: nextId.current++, control, source, pointerId, initialPan: pan ?? 0, initialTilt: tilt ?? 0 };
		const next: ActiveGesture = { gesture, element, changed: false, angle: 0, raw: pan ?? 0, last: (control === "tilt" ? tilt : pan) ?? 0, x: 0, y: 0 };
		active.current = next;
		latest.current.onGestureStart?.(gesture);
		// A start callback may close the dialog.
		return active.current === next ? next : null;
	};
	const emit = (current: ActiveGesture, change: PositionChange) => {
		current.changed = true;
		latest.current.onChange(change, current.gesture);
	};
	return {
		latest, active, mounted, resets, stop, begin, emit,
		/** A keyboard or button step is one complete gesture; it emits only a real change. */
		step(control: "pan" | "tilt", source: PositionGestureSource, element: Element, next: number) {
			const value = latest.current[control];
			if (active.current || value === null || next === value || !Number.isFinite(next)) return;
			const current = begin(control, source, element);
			if (!current) return;
			emit(current, control === "pan" ? { pan: next } : { tilt: next });
			if (active.current === current) stop("release");
		},
		matches: (control: PositionControl, event: PointerEvent<Element>) =>
			active.current?.gesture.control === control && active.current.gesture.pointerId === event.pointerId,
		capture(event: PointerEvent<Element>) {
			try {
				event.currentTarget.setPointerCapture?.(event.pointerId);
			} catch {
				// Capture is best effort; release, cancel, blur and teardown still end the gesture.
			}
		},
		/** Shared non-release terminal paths of one control. */
		endHandlers: (control: PositionControl) => ({
			onPointerCancel: (event: PointerEvent<Element>) => { if (active.current?.gesture.control === control && active.current.gesture.pointerId === event.pointerId) stop("pointer-cancel"); },
			onLostPointerCapture: (event: PointerEvent<Element>) => { if (active.current?.gesture.control === control && active.current.gesture.pointerId === event.pointerId) stop("lost-capture"); },
			onBlur: () => { if (active.current?.gesture.control === control) stop("blur"); },
		}),
		primary: (event: PointerEvent<Element>) => event.pointerType !== "mouse" || event.button === 0,
	};
}

type Gestures = ReturnType<typeof useGestureLifecycle>;
interface AxisProps {
	gestures: Gestures;
	value: number | null;
	limits: ResolvedLimits | null;
	caption?: string;
}

/**
 * Controlled Position Special Dialog: unwrapped multi-turn Pan circle with −90°/Reset/+90°, a Tilt touch
 * fader below it, and a square rate joystick beside them. All values come from props; every edit is
 * reported as a finite, bounded, quantized change with an explicit gesture lifecycle.
 */
export function PositionDialog({
	pan,
	tilt,
	joystick,
	onClose,
	title = "Position",
	ariaLabel = "Position Special Dialog",
	closeLabel = "Close Position Special Dialog",
	className,
	dialogClassName,
	valueCaption,
	returnHome,
	...callbacks
}: PositionDialogProps) {
	const panLimits = resolveLimits(pan, () => 360);
	const tiltLimits = resolveLimits(tilt, keyStep => keyStep * 10);
	const panValue = displayed(pan.value, panLimits);
	const tiltValue = displayed(tilt.value, tiltLimits);
	const rates = validRates(joystick) ? joystick! : null;
	const gestures = useGestureLifecycle({ ...callbacks, onClose, pan: panValue, tilt: tiltValue, panLimits, tiltLimits, rates });
	const close = () => {
		gestures.stop("close");
		gestures.latest.current.onClose();
	};
	return (
		<ModalFrame
			title={title}
			ariaLabel={ariaLabel}
			closeLabel={closeLabel}
			onClose={close}
			dialogClassName={join("position-dialog", dialogClassName)}
			className={join("position-dialog-layer", className)}
		>
			<div className="position-dialog-body" data-testid="editor-page">
				<div className="position-dialog-layout">
					<div className="position-dialog-angle-controls">
						<PanCircle gestures={gestures} value={panValue} limits={panLimits} caption={valueCaption} />
						<TiltFader gestures={gestures} value={tiltValue} limits={tiltLimits} caption={valueCaption} />
					</div>
					<RateJoystick gestures={gestures} enabled={!!rates && panValue !== null && tiltValue !== null} pan={panValue} tilt={tiltValue}>
						{returnHome ? (
							<Button className="position-dialog-home" disabled={returnHome.disabled}
								onClick={() => {
									// A press is its own request: any open drag or held joystick stops first.
									gestures.stop("superseded");
									returnHome.onPress();
								}}>Return Home</Button>
						) : null}
					</RateJoystick>
				</div>
			</div>
		</ModalFrame>
	);
}

function PanCircle({ gestures, value, limits, caption }: AxisProps) {
	const shown = value ?? 0;
	const digits = limits?.decimals ?? 1;
	const angle = shown * Math.PI / 180;
	const turns = shown / 360;
	const pointerAngle = (event: PointerEvent<Element>) => {
		const box = event.currentTarget.getBoundingClientRect();
		const x = event.clientX - box.left - box.width / 2, y = event.clientY - box.top - box.height / 2;
		return Math.hypot(x, y) < PAN_HUB_RADIUS ? null : Math.atan2(x, -y) * 180 / Math.PI;
	};
	const move = (event: PointerEvent<SVGSVGElement>) => {
		const current = gestures.active.current, next = pointerAngle(event);
		if (!current || !limits || !gestures.matches("pan", event) || next === null) return;
		// Shortest signed turn since the last sample keeps multi-turn travel continuous.
		const delta = ((next - current.angle + 540) % 360) - 180;
		current.angle = next;
		current.raw = bounded(current.raw + delta, limits.minimum, limits.maximum);
		const sample = quantize(current.raw, limits);
		if (sample === current.last) return;
		current.last = sample;
		gestures.emit(current, { pan: sample });
	};
	const keys = (event: KeyboardEvent<SVGSVGElement>) => {
		if (!["ArrowLeft", "ArrowRight", "Home", "PageUp", "PageDown"].includes(event.key)) return;
		event.preventDefault();
		if (!limits || value === null) return;
		const amount = event.key === "PageUp" ? limits.largeKeyStep : event.key === "PageDown" ? -limits.largeKeyStep
			: event.key === "ArrowRight" ? limits.keyStep : -limits.keyStep;
		gestures.step("pan", "keyboard", event.currentTarget, quantize(event.key === "Home" ? 0 : value + amount, limits));
	};
	const button = (element: Element, next: number) => {
		if (limits) gestures.step("pan", "button", element, quantize(next, limits));
	};
	return (
		<div className="position-dialog-pan">
			<svg viewBox="0 0 220 220" role="slider" tabIndex={0} aria-label="Pan circle" aria-disabled={value === null || undefined}
				aria-valuemin={limits?.minimum} aria-valuemax={limits?.maximum} aria-valuenow={value ?? undefined}
				aria-valuetext={value === null ? "Unavailable" : `${shown.toFixed(digits)} degrees, ${turns.toFixed(2)} turns${caption ? `, ${caption}` : ""}`} data-testid="pan-circle"
				onKeyDown={keys}
				onPointerDown={event => {
					if (!limits || value === null || !gestures.primary(event) || gestures.active.current?.gesture.control === "pan") return;
					const start = pointerAngle(event);
					if (start === null) return;
					event.preventDefault();
					const element = event.currentTarget;
					const current = gestures.begin("pan", "pointer", element, event.pointerId);
					if (!current) return;
					current.angle = start;
					element.focus();
					gestures.capture(event);
				}}
				onPointerMove={move}
				onPointerUp={event => { if (!gestures.matches("pan", event)) return; move(event); if (gestures.matches("pan", event)) gestures.stop("release"); }}
				{...gestures.endHandlers("pan")}>
				<circle cx="110" cy="110" r="81" className="position-dialog-pan-ring" />
				{Array.from({ length: 24 }, (_, i) => <line key={i} x1="110" y1="20" x2="110" y2={i % 6 ? "25" : "32"} transform={`rotate(${i * 15} 110 110)`} className="position-dialog-pan-tick" />)}
				<text x="110" y="13" textAnchor="middle" className="position-dialog-pan-zero">0°</text>
				<line x1="110" y1="110" x2={110 + Math.sin(angle) * 76} y2={110 - Math.cos(angle) * 76} className="position-dialog-pan-direction" />
				<circle cx={110 + Math.sin(angle) * 81} cy={110 - Math.cos(angle) * 81} r="10" className="position-dialog-pan-handle" />
				<circle cx="110" cy="110" r="47" className="position-dialog-pan-center" />
				<text x="110" y="101" textAnchor="middle" className="position-dialog-pan-label">Pan</text>
				<text x="110" y="124" textAnchor="middle" className="position-dialog-pan-value">{value === null ? "—" : `${shown.toFixed(digits)}°`}</text>
				{caption ? <text x="110" y="141" textAnchor="middle" className="position-dialog-value-caption" data-testid="pan-value-caption">{caption}</text> : null}
			</svg>
			<div className="position-dialog-pan-actions">
				<Button aria-label="Decrease pan by 90 degrees" disabled={!limits || value === null || value <= limits.minimum}
					onClick={event => button(event.currentTarget, shown - 90)}>−90°</Button>
				<Button aria-label="Reset pan to zero" disabled={!limits || value === null} onClick={event => button(event.currentTarget, 0)}>Reset</Button>
				<Button aria-label="Increase pan by 90 degrees" disabled={!limits || value === null || value >= limits.maximum}
					onClick={event => button(event.currentTarget, shown + 90)}>+90°</Button>
			</div>
			<output className="position-dialog-turns" aria-label="Pan turns">{turns > 0 ? "+" : ""}{turns.toFixed(2)} turns</output>
		</div>
	);
}

function TiltFader({ gestures, value, limits, caption }: AxisProps) {
	const field = useRef<HTMLDivElement>(null);
	useEffect(() => {
		// The pointer handlers own touch contacts; the native range input must not also consume them.
		const element = field.current;
		if (!element) return;
		const suppress = (event: TouchEvent) => { if ((event.target as Element | null)?.matches?.('input[type="range"]')) event.preventDefault(); };
		element.addEventListener("touchstart", suppress, { passive: false });
		element.addEventListener("touchmove", suppress, { passive: false });
		return () => { element.removeEventListener("touchstart", suppress); element.removeEventListener("touchmove", suppress); };
	}, []);
	const digits = limits?.decimals ?? 1;
	const fraction = limits && value !== null && limits.maximum > limits.minimum ? (value - limits.minimum) / (limits.maximum - limits.minimum) : 0;
	const text = value === null ? "—" : `${value.toFixed(digits)}°`;
	const at = (event: PointerEvent<HTMLInputElement>, range: ResolvedLimits) => {
		const rect = event.currentTarget.getBoundingClientRect();
		return quantize(range.minimum + (event.clientX - rect.left) / Math.max(1, rect.width) * (range.maximum - range.minimum), range);
	};
	const sample = (current: ActiveGesture, next: number) => {
		if (next === current.last) return;
		current.last = next;
		gestures.emit(current, { tilt: next });
	};
	const keys = (event: KeyboardEvent<HTMLInputElement>) => {
		if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(event.key)) return;
		event.preventDefault();
		if (!limits || value === null) return;
		const amount = event.key.startsWith("Page") ? limits.largeKeyStep : limits.keyStep;
		const negative = ["ArrowLeft", "ArrowDown", "PageDown"].includes(event.key);
		gestures.step("tilt", "keyboard", event.currentTarget, event.key === "Home" ? limits.minimum : event.key === "End" ? limits.maximum
			: quantize(value + (negative ? -amount : amount), limits));
	};
	return (
		<div className="position-dialog-tilt">
			<div ref={field} className="position-dialog-tilt-field">
				<VerticalTouchFaderControl label="Tilt angle" display={<output aria-label="Tilt angle value">{text}{caption ? <small className="position-dialog-value-caption" data-testid="tilt-value-caption"> {caption}</small> : null}</output>} fraction={fraction}
					disabled={value === null} className="position-dialog-tilt-fader fam-range-fader">
					<Input type="range" aria-label="Tilt angle" aria-orientation="horizontal" disabled={value === null}
						min={limits?.minimum} max={limits?.maximum} step={limits?.step || "any"} value={value ?? 0}
						aria-valuemin={limits?.minimum} aria-valuemax={limits?.maximum} aria-valuenow={value ?? undefined} aria-valuetext={text}
						// Assistive technology can still set the native range; that is one step.
						onChange={event => { if (limits) gestures.step("tilt", "keyboard", event.currentTarget, quantize(Number(event.target.value), limits)); }}
						onKeyDown={keys}
						onPointerDown={event => {
							if (!limits || value === null || !gestures.primary(event) || gestures.active.current?.gesture.control === "tilt") return;
							event.preventDefault();
							const element = event.currentTarget;
							const current = gestures.begin("tilt", "pointer", element, event.pointerId);
							if (!current) return;
							current.x = event.clientX;
							current.y = event.clientY;
							element.focus();
							gestures.capture(event);
							sample(current, at(event, limits));
						}}
						onPointerMove={event => {
							const current = gestures.active.current;
							if (!current || !limits || !gestures.matches("tilt", event)) return;
							if (Math.hypot(event.clientX - current.x, event.clientY - current.y) < FADER_THRESHOLD) return;
							sample(current, at(event, limits));
						}}
						onPointerUp={event => { if (gestures.matches("tilt", event)) gestures.stop("release"); }}
						{...gestures.endHandlers("tilt")} />
					<i aria-hidden="true" className="position-dialog-tilt-handle fam-range-handle" style={{ left: `clamp(12px, ${fraction * 100}%, calc(100% - 12px))` }} />
				</VerticalTouchFaderControl>
			</div>
			<div className="position-dialog-tilt-limits">
				{limits ? <><span>{signed(limits.minimum, digits)}</span><span>{signed((limits.minimum + limits.maximum) / 2, digits)}</span>
					<span>{signed(limits.maximum, digits)}</span></> : <span>Tilt limits unavailable</span>}
			</div>
		</div>
	);
}

function RateJoystick({ gestures, enabled, pan, tilt, children }: { gestures: Gestures; enabled: boolean; pan: number | null; tilt: number | null; children?: ReactNode }) {
	// `raw` keeps sub-step motion between frames; `emitted` is the last value supplied or sent.
	const axes = useRef({ raw: { pan: pan ?? 0, tilt: tilt ?? 0 }, emitted: { pan: pan ?? 0, tilt: tilt ?? 0 } });
	useLayoutEffect(() => {
		// A supplied value that differs from the last emitted value is an external update: adopt it.
		if (pan !== null && pan !== axes.current.emitted.pan) axes.current.raw.pan = axes.current.emitted.pan = pan;
		if (tilt !== null && tilt !== axes.current.emitted.tilt) axes.current.raw.tilt = axes.current.emitted.tilt = tilt;
	}, [pan, tilt]);
	const velocity = useRef(NEUTRAL);
	const frame = useRef<number | null>(null);
	const lastTime = useRef<number | null>(null);
	const keys = useRef(new Set<string>());
	const [marker, setMarker] = useState(NEUTRAL);
	const [held, setHeld] = useState(false);
	const stopFrames = () => {
		if (frame.current !== null) cancelAnimationFrame(frame.current);
		frame.current = null;
		lastTime.current = null;
	};
	useLayoutEffect(() => {
		gestures.resets.current.joystick = () => {
			keys.current.clear();
			velocity.current = NEUTRAL;
			stopFrames();
			if (gestures.mounted.current) { setMarker(NEUTRAL); setHeld(false); }
		};
	});
	useEffect(() => () => stopFrames(), []);

	const tick = (now: number) => {
		frame.current = null;
		const current = gestures.active.current;
		const { panLimits, tiltLimits } = gestures.latest.current;
		if (!current || current.gesture.control !== "joystick" || !panLimits || !tiltLimits) return;
		const seconds = Math.min(MAX_FRAME_SECONDS, Math.max(0, now - (lastTime.current ?? now)) / 1000);
		lastTime.current = now;
		const { raw, emitted } = axes.current;
		const next = {
			pan: bounded(raw.pan + velocity.current.x * seconds, panLimits.minimum, panLimits.maximum),
			tilt: bounded(raw.tilt + velocity.current.y * seconds, tiltLimits.minimum, tiltLimits.maximum),
		};
		const moved = next.pan !== raw.pan || next.tilt !== raw.tilt;
		axes.current.raw = next;
		const sample = { pan: quantize(next.pan, panLimits), tilt: quantize(next.tilt, tiltLimits) };
		const change: PositionChange = {};
		if (sample.pan !== emitted.pan) change.pan = sample.pan;
		if (sample.tilt !== emitted.tilt) change.tilt = sample.tilt;
		if (change.pan !== undefined || change.tilt !== undefined) {
			axes.current.emitted = sample;
			gestures.emit(current, change);
		}
		// No idle loop at a hard stop; a new displacement restarts motion.
		if (gestures.active.current === current && (velocity.current.x || velocity.current.y) && (moved || seconds === 0)) frame.current = requestAnimationFrame(tick);
	};
	const setVector = (vector: Vector) => {
		const { rates } = gestures.latest.current;
		if (!rates) return;
		const radius = Math.max(1, Math.hypot(vector.x, vector.y));
		const normalized = { x: vector.x / radius, y: vector.y / radius };
		const x = response(normalized.x), y = response(normalized.y);
		velocity.current = { x: x * rates.panDegreesPerSecond, y: -y * rates.tiltDegreesPerSecond };
		setMarker(x || y ? normalized : NEUTRAL);
		// Centering stops frames immediately; the held gesture stays open until release.
		if (!x && !y) { stopFrames(); return; }
		if (gestures.active.current?.gesture.control === "joystick" && frame.current === null) {
			lastTime.current = performance.now();
			frame.current = requestAnimationFrame(tick);
		}
	};
	const point = (event: PointerEvent<HTMLDivElement>) => {
		const rect = event.currentTarget.getBoundingClientRect();
		return {
			x: bounded((event.clientX - rect.left - rect.width / 2) / Math.max(1, rect.width / 2 - JOYSTICK_INSET), -1, 1),
			y: bounded((event.clientY - rect.top - rect.height / 2) / Math.max(1, rect.height / 2 - JOYSTICK_INSET), -1, 1),
		};
	};
	const keyboardVector = () => setVector({
		x: (keys.current.has("ArrowRight") ? 1 : 0) - (keys.current.has("ArrowLeft") ? 1 : 0),
		y: (keys.current.has("ArrowDown") ? 1 : 0) - (keys.current.has("ArrowUp") ? 1 : 0),
	});
	const keyboardHeld = () => gestures.active.current?.gesture.control === "joystick" && gestures.active.current.gesture.source === "keyboard";
	const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
		event.preventDefault();
		if (!enabled || (gestures.active.current && !keyboardHeld())) return;
		if (!gestures.active.current) {
			if (!gestures.begin("joystick", "keyboard", event.currentTarget)) return;
			setHeld(true);
		}
		keys.current.add(event.key);
		keyboardVector();
	};
	const keyUp = (event: KeyboardEvent<HTMLDivElement>) => {
		if (!keyboardHeld() || !keys.current.delete(event.key)) return;
		event.preventDefault();
		if (keys.current.size) keyboardVector();
		else gestures.stop("release");
	};
	return (
		<div className="position-dialog-aim">
			<div className={join("position-dialog-joystick", held ? "is-held" : undefined, enabled ? undefined : "is-disabled")} role="application" tabIndex={0}
				aria-label="Position aim joystick" aria-disabled={!enabled || undefined} data-testid="position-joystick"
				data-active={held} data-pan-rate={velocity.current.x.toFixed(2)} data-tilt-rate={velocity.current.y.toFixed(2)}
				onPointerDown={event => {
					if (!enabled || !gestures.primary(event) || gestures.active.current?.gesture.control === "joystick") return;
					event.preventDefault();
					const element = event.currentTarget;
					if (!gestures.begin("joystick", "pointer", element, event.pointerId)) return;
					element.focus();
					gestures.capture(event);
					setHeld(true);
					setVector(point(event));
				}}
				onPointerMove={event => { if (gestures.matches("joystick", event)) setVector(point(event)); }}
				onPointerUp={event => { if (gestures.matches("joystick", event)) gestures.stop("release"); }}
				{...gestures.endHandlers("joystick")}
				onKeyDown={keyDown} onKeyUp={keyUp}>
				<span className="position-dialog-joystick-ring" aria-hidden="true" />
				<span className="position-dialog-joystick-neutral" aria-hidden="true" />
				<span className="position-dialog-joystick-marker" data-testid="joystick-marker" style={{ left: `${50 + marker.x * 42}%`, top: `${50 + marker.y * 42}%` }} aria-hidden="true" />
				<span className="position-dialog-joystick-label">Aim</span>
			</div>
			<p>{enabled ? "Hold away from center to move · release to stop" : "Joystick unavailable"}</p>
			{children}
		</div>
	);
}
