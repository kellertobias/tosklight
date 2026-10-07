import { ModalFrame } from "@tosklight/ui";
import {
	useCallback,
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
	type KeyboardEvent,
	type PointerEvent,
	type RefObject,
} from "react";
import "./FocusZoomDialog.css";

/** How the supplied full opening angle was measured by the fixture profile. */
export type ZoomConvention = "beam" | "field";

/** Limits and steps for one control. `step` quantizes every emitted value. */
export interface FocusZoomLimits {
	minimum: number;
	maximum: number;
	step: number;
	/** Arrow-key increment; defaults to `step`. */
	keyStep?: number;
	/** Shift+Arrow and Page Up/Down increment; defaults to ten key steps. */
	largeKeyStep?: number;
}

/**
 * Zoom as a full opening angle in degrees, with the profile's beam/field convention. `null` is an
 * unknown or mixed convention: the opening is then labelled neutrally, never guessed.
 */
export interface FocusZoomZoomValue extends FocusZoomLimits {
	value: number;
	convention: ZoomConvention | null;
}

/**
 * Focus as a normalized 0–1 lens setting, displayed as 0–100 %. It is never a focal distance;
 * limits outside 0–1 are clamped to that domain.
 */
export interface FocusZoomFocusValue extends FocusZoomLimits {
	value: number;
}

export type FocusZoomControl = "zoom" | "focus";
export type FocusZoomGestureSource = "pointer" | "keyboard";
export type FocusZoomCancelReason = "pointer-cancel" | "lost-capture" | "close" | "teardown";

/** One coherent edit gesture. Every started gesture receives exactly one end or cancel. */
export interface FocusZoomGesture {
	id: number;
	control: FocusZoomControl;
	source: FocusZoomGestureSource;
	pointerId?: number;
	/** The supplied value when the gesture started. */
	initialValue: number;
}

export interface FocusZoomDialogProps {
	zoom: FocusZoomZoomValue;
	focus: FocusZoomFocusValue;
	/** Requested Zoom in degrees. The component never stores the value; render the new prop. */
	onZoomChange(value: number, gesture: FocusZoomGesture): void;
	/** Requested normalized Focus (0–1). The component never stores the value. */
	onFocusChange(value: number, gesture: FocusZoomGesture): void;
	onGestureStart?(gesture: FocusZoomGesture): void;
	/** The operator released the pointer, or a keyboard step completed. */
	onGestureEnd?(gesture: FocusZoomGesture): void;
	/** The gesture stopped without a release: pointer cancel, lost capture, close or unmount. */
	onGestureCancel?(gesture: FocusZoomGesture, reason: FocusZoomCancelReason): void;
	onClose(): void;
	/** Quiet per-control state beside the readout (for example requested-only or unsupported). */
	zoomStatus?: string;
	focusStatus?: string;
	/**
	 * Edits cannot be taken yet (the capture lane or the requested values are still loading):
	 * both controls are inert and `aria-disabled`, so no key or drag is silently lost.
	 */
	disabled?: boolean;
	title?: string;
	ariaLabel?: string;
	closeLabel?: string;
	className?: string;
	dialogClassName?: string;
}

interface ActiveGesture {
	gesture: FocusZoomGesture;
	element: Element;
	x: number;
	y: number;
	last: number;
	// Geometry captured at gesture start; a resize during the gesture never moves the grab point.
	opening: number;
	side: number;
	fraction: number;
	length: number;
	maximumOpening: number;
	tangentAtMaximum: number;
	focusSpan: number;
}

const MARGIN = 32;
// Handles stay outside the 44 px Focus target and apart from each other, even for a narrow beam.
const HANDLE_REACH = 44;
const DEFAULT_SIZE = { width: 640, height: 180 };

const bounded = (value: number, minimum: number, maximum: number) => Math.max(minimum, Math.min(maximum, value));
const radians = (degrees: number) => (degrees * Math.PI) / 180;
const degrees = (value: number) => (value * 180) / Math.PI;
/** Keep the schematic finite: openings at or beyond 180° cannot be drawn as a cone. */
const drawableAngle = (angle: number) => bounded(Number.isFinite(angle) ? angle : 0, 0, 170);

function decimals(step: number) {
	if (!Number.isFinite(step) || step <= 0) return 0;
	const text = String(step);
	const exponent = text.match(/e-(\d+)$/);
	if (exponent) return Number(exponent[1]);
	return text.includes(".") ? text.length - text.indexOf(".") - 1 : 0;
}

interface ResolvedLimits {
	minimum: number;
	maximum: number;
	step: number;
	keyStep: number;
	largeKeyStep: number;
	decimals: number;
}

function resolveLimits(limits: FocusZoomLimits, domain?: [number, number]): ResolvedLimits {
	let minimum = Math.min(limits.minimum, limits.maximum);
	let maximum = Math.max(limits.minimum, limits.maximum);
	if (domain) {
		minimum = bounded(minimum, domain[0], domain[1]);
		maximum = bounded(maximum, domain[0], domain[1]);
	}
	const step = Number.isFinite(limits.step) && limits.step > 0 ? limits.step : 0;
	const keyStep = limits.keyStep && limits.keyStep > 0 ? limits.keyStep : step || (maximum - minimum) / 100 || 1;
	const largeKeyStep = limits.largeKeyStep && limits.largeKeyStep > 0 ? limits.largeKeyStep : keyStep * 10;
	return { minimum, maximum, step, keyStep, largeKeyStep, decimals: decimals(step) };
}

function quantize(value: number, limits: ResolvedLimits) {
	const clamped = bounded(value, limits.minimum, limits.maximum);
	if (!limits.step) return clamped;
	const stepped = limits.minimum + Math.round((clamped - limits.minimum) / limits.step) * limits.step;
	return bounded(Number(stepped.toFixed(limits.decimals)), limits.minimum, limits.maximum);
}

const formatZoom = (value: number, limits: ResolvedLimits) => value.toFixed(limits.decimals);
const focusPercentDecimals = (limits: ResolvedLimits) => Math.max(0, limits.decimals - 2);
const formatFocus = (value: number, limits: ResolvedLimits) => (value * 100).toFixed(focusPercentDecimals(limits));

/**
 * Controlled Focus and Zoom Special Dialog: drag either beam edge or its end handles for Zoom, and the
 * focus plane for Focus. All data comes from props; gestures are reported for atomic commands.
 */
export function FocusZoomDialog({
	zoom,
	focus,
	onClose,
	title = "Focus",
	ariaLabel = "Focus Special Dialog",
	closeLabel = "Close Focus Special Dialog",
	className,
	dialogClassName,
	zoomStatus,
	focusStatus,
	disabled = false,
	...callbacks
}: FocusZoomDialogProps) {
	const latest = useRef({ ...callbacks, onClose });
	useLayoutEffect(() => {
		latest.current = { ...callbacks, onClose };
	});
	const active = useRef<ActiveGesture | null>(null);
	const nextId = useRef(1);

	const stopGesture = useCallback((outcome: "release" | FocusZoomCancelReason) => {
		const current = active.current;
		if (!current) return;
		active.current = null;
		const pointerId = current.gesture.pointerId;
		if (pointerId !== undefined) {
			try {
				if (current.element.hasPointerCapture?.(pointerId)) current.element.releasePointerCapture(pointerId);
			} catch {
				// The element may already be detached during teardown.
			}
		}
		if (outcome === "release") latest.current.onGestureEnd?.(current.gesture);
		else latest.current.onGestureCancel?.(current.gesture, outcome);
	}, []);

	useEffect(() => () => stopGesture("teardown"), [stopGesture]);

	const close = useCallback(() => {
		stopGesture("close");
		latest.current.onClose();
	}, [stopGesture]);

	return (
		<ModalFrame
			title={title}
			ariaLabel={ariaLabel}
			closeLabel={closeLabel}
			onClose={close}
			dialogClassName={["focus-zoom-dialog", dialogClassName].filter(Boolean).join(" ")}
			className={["focus-zoom-layer", className].filter(Boolean).join(" ")}
		>
			<div className="focus-zoom-body" data-testid="editor-page">
				<BeamDiagram zoom={zoom} focus={focus} active={active} nextId={nextId} latest={latest} stopGesture={stopGesture}
					statuses={{ zoom: zoomStatus, focus: focusStatus }} disabled={disabled} />
			</div>
		</ModalFrame>
	);
}

interface BeamDiagramProps {
	zoom: FocusZoomZoomValue;
	focus: FocusZoomFocusValue;
	active: RefObject<ActiveGesture | null>;
	nextId: RefObject<number>;
	latest: RefObject<Omit<FocusZoomDialogProps, "zoom" | "focus" | "title" | "ariaLabel" | "closeLabel" | "className" | "dialogClassName" | "zoomStatus" | "focusStatus" | "disabled">>;
	stopGesture(outcome: "release" | FocusZoomCancelReason): void;
	statuses: { zoom?: string; focus?: string };
	disabled: boolean;
}

function BeamDiagram({ zoom, focus, active, nextId, latest, stopGesture, statuses, disabled }: BeamDiagramProps) {
	const svg = useRef<SVGSVGElement>(null);
	const viewport = useRef<HTMLDivElement>(null);
	const size = useViewportSize(viewport);

	const zoomLimits = resolveLimits(zoom);
	const focusLimits = resolveLimits(focus, [0, 1]);
	const zoomValue = bounded(zoom.value, zoomLimits.minimum, zoomLimits.maximum);
	const focusValue = bounded(focus.value, focusLimits.minimum, focusLimits.maximum);
	const left = MARGIN;
	const right = Math.max(left + 60, size.width - MARGIN);
	const length = right - left;
	const center = size.height / 2;
	const maximumOpening = Math.max(8, center - 28);
	// Scale the cone so the supplied maximum opening fills the available height.
	const tangentAtMaximum = Math.max(1e-6, Math.tan(radians(drawableAngle(zoomLimits.maximum) / 2)));
	const halfOpening = maximumOpening * Math.tan(radians(drawableAngle(zoomValue) / 2)) / tangentAtMaximum;
	const focusSpan = focusLimits.maximum - focusLimits.minimum;
	const focusFraction = focusSpan > 0 ? (focusValue - focusLimits.minimum) / focusSpan : 0;
	// This is an angle/focus control schematic. Focus is a normalized setting, not a distance.
	const planeX = left + focusFraction * length;
	const planeHalfHeight = halfOpening * focusFraction;
	// Keep the two end handles independently touchable even for a very narrow beam.
	const handleOffset = Math.max(HANDLE_REACH, halfOpening);
	const edgeStartX = left + length * .2;
	const edgeStartHeight = halfOpening * .2;
	const zoomText = formatZoom(zoomValue, zoomLimits);
	const focusText = formatFocus(focusValue, focusLimits);
	const percentDecimals = focusPercentDecimals(focusLimits);
	const conventionLabel = zoom.convention === "field" ? "Field" : zoom.convention === "beam" ? "Beam" : "Zoom";

	const position = (event: PointerEvent<Element>) => {
		const bounds = svg.current?.getBoundingClientRect();
		if (!bounds || bounds.width <= 0 || bounds.height <= 0) return null;
		// The viewBox matches the viewport size and is not aspect-preserved, so this is exact.
		return {
			x: (event.clientX - bounds.left) * size.width / bounds.width,
			y: (event.clientY - bounds.top) * size.height / bounds.height,
		};
	};

	const emit = (control: FocusZoomControl, value: number, gesture: FocusZoomGesture) => {
		if (control === "zoom") latest.current.onZoomChange(value, gesture);
		else latest.current.onFocusChange(value, gesture);
	};

	const down = (control: FocusZoomControl, event: PointerEvent<SVGGElement>) => {
		if (active.current || disabled) return;
		if (event.pointerType === "mouse" && event.button !== 0) return;
		const point = position(event);
		if (!point) return;
		event.preventDefault();
		const target = event.target as Element;
		const handle = target.closest("[data-angle-handle]");
		// Edge hit strokes overlap near a narrow beam's axis; the pointer's side of the axis decides.
		const side = (handle ? Number(handle.getAttribute("data-beam-side")) : 0) || (point.y < center ? -1 : 1);
		const initialValue = control === "zoom" ? zoomValue : focusValue;
		const gesture: FocusZoomGesture = { id: nextId.current++, control, source: "pointer", pointerId: event.pointerId, initialValue };
		active.current = {
			gesture, element: event.currentTarget, x: point.x, y: point.y, last: initialValue,
			opening: halfOpening, side, fraction: handle ? 1 : bounded((point.x - left) / length, .2, 1),
			length, maximumOpening, tangentAtMaximum, focusSpan,
		};
		try {
			event.currentTarget.setPointerCapture?.(event.pointerId);
		} catch {
			// Capture is best effort; release, cancel and teardown still end the gesture.
		}
		latest.current.onGestureStart?.(gesture);
	};

	const move = (event: PointerEvent<SVGGElement>) => {
		const current = active.current;
		if (!current || current.gesture.pointerId !== event.pointerId) return;
		const point = position(event);
		if (!point || (point.x === current.x && point.y === current.y)) return;
		let next: number;
		if (current.gesture.control === "focus") {
			next = quantize(current.gesture.initialValue + (point.x - current.x) / current.length * current.focusSpan, focusLimits);
		} else {
			// Retain the edge, local beam fraction and initial grab offset for the full gesture.
			const opening = Math.max(0, current.opening + current.side * (point.y - current.y) / current.fraction);
			next = quantize(2 * degrees(Math.atan(opening / current.maximumOpening * current.tangentAtMaximum)), zoomLimits);
		}
		if (next === current.last) return;
		current.last = next;
		emit(current.gesture.control, next, current.gesture);
	};

	const up = (event: PointerEvent<SVGGElement>) => {
		if (active.current?.gesture.pointerId === event.pointerId) stopGesture("release");
	};
	const cancel = (event: PointerEvent<SVGGElement>) => {
		if (active.current?.gesture.pointerId === event.pointerId) stopGesture("pointer-cancel");
	};
	const lost = (event: PointerEvent<SVGGElement>) => {
		if (active.current?.gesture.pointerId === event.pointerId) stopGesture("lost-capture");
	};

	const keys = (control: FocusZoomControl, event: KeyboardEvent<SVGGElement>) => {
		if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(event.key)) return;
		event.preventDefault();
		if (active.current || disabled) return;
		const limits = control === "zoom" ? zoomLimits : focusLimits;
		const value = control === "zoom" ? zoomValue : focusValue;
		const positive = ["ArrowRight", "ArrowUp", "PageUp"].includes(event.key);
		const amount = event.shiftKey || event.key.startsWith("Page") ? limits.largeKeyStep : limits.keyStep;
		const next = event.key === "Home" ? limits.minimum : event.key === "End" ? limits.maximum
			: quantize(value + (positive ? amount : -amount), limits);
		if (next === value) return;
		const gesture: FocusZoomGesture = { id: nextId.current++, control, source: "keyboard", initialValue: value };
		latest.current.onGestureStart?.(gesture);
		emit(control, next, gesture);
		latest.current.onGestureEnd?.(gesture);
	};

	const pointerHandlers = (control: FocusZoomControl) => ({
		onPointerDown: (event: PointerEvent<SVGGElement>) => down(control, event),
		onPointerMove: move,
		onPointerUp: up,
		onPointerCancel: cancel,
		onLostPointerCapture: lost,
	});

	return (
		<div className="focus-zoom-editor">
			<div ref={viewport} className="focus-zoom-viewport">
				<BeamSvg svg={svg} size={size} left={left} right={right} center={center} halfOpening={halfOpening} edgeStartX={edgeStartX}
					edgeStartHeight={edgeStartHeight} planeX={planeX} planeHalfHeight={planeHalfHeight} handleOffset={handleOffset} zoomLimits={zoomLimits}
					focusLimits={focusLimits} zoomText={zoomText} focusText={focusText} percentDecimals={percentDecimals} conventionLabel={conventionLabel}
					keys={keys} pointerHandlers={pointerHandlers} disabled={disabled} />
			</div>
			<div className="focus-zoom-readouts">
				<span>{conventionLabel} <strong>{zoomText}°</strong>{statuses.zoom ? <Status text={statuses.zoom} control="zoom" /> : null}</span>
				<span>Focus <strong>{focusText}%</strong>{statuses.focus ? <Status text={statuses.focus} control="focus" /> : null}</span>
			</div>
		</div>
	);
}

/** A quiet, non-blocking per-control note beside its readout. */
function Status({ text, control }: { text: string; control: FocusZoomControl }) {
	return <em className="focus-zoom-status" role="status" data-testid={`focus-zoom-${control}-status`}>{text}</em>;
}

/** Keeps the diagram viewBox in step with the rendered viewport size. */
function useViewportSize(viewport: RefObject<HTMLDivElement | null>) {
	const [size, setSize] = useState(DEFAULT_SIZE);
	useLayoutEffect(() => {
		const element = viewport.current;
		if (!element || typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(([entry]) => {
			const { width, height } = entry.contentRect;
			if (!width || !height) return;
			setSize(current => Math.abs(current.width - width) < .1 && Math.abs(current.height - height) < .1 ? current : { width, height });
		});
		observer.observe(element);
		return () => observer.disconnect();
	}, [viewport]);
	return size;
}

interface BeamPointerHandlers {
	onPointerDown(event: PointerEvent<SVGGElement>): void;
	onPointerMove(event: PointerEvent<SVGGElement>): void;
	onPointerUp(event: PointerEvent<SVGGElement>): void;
	onPointerCancel(event: PointerEvent<SVGGElement>): void;
	onLostPointerCapture(event: PointerEvent<SVGGElement>): void;
}

interface BeamSvgProps {
	svg: RefObject<SVGSVGElement | null>;
	size: { width: number; height: number };
	left: number;
	right: number;
	center: number;
	halfOpening: number;
	edgeStartX: number;
	edgeStartHeight: number;
	planeX: number;
	planeHalfHeight: number;
	handleOffset: number;
	zoomLimits: ResolvedLimits;
	focusLimits: ResolvedLimits;
	zoomText: string;
	focusText: string;
	percentDecimals: number;
	conventionLabel: string;
	keys(control: FocusZoomControl, event: KeyboardEvent<SVGGElement>): void;
	pointerHandlers(control: FocusZoomControl): BeamPointerHandlers;
	disabled: boolean;
}

/** Presentational beam schematic: cone, edge/angle handles and focus plane with their gesture hooks. */
function BeamSvg({
	svg, size, left, right, center, halfOpening, edgeStartX, edgeStartHeight, planeX, planeHalfHeight, handleOffset,
	zoomLimits, focusLimits, zoomText, focusText, percentDecimals, conventionLabel, keys, pointerHandlers, disabled,
}: BeamSvgProps) {
	return (
		<svg ref={svg} viewBox={`0 0 ${size.width} ${size.height}`} preserveAspectRatio="none" aria-label="Beam angle and focus diagram" role="group">
			<path d={`M${left} ${center} L${right} ${center - halfOpening} L${right} ${center + halfOpening} Z`} className="focus-zoom-cone" />
			<line x1={left} y1={center} x2={right} y2={center} className="focus-zoom-centerline" />
			<g className="focus-zoom-edge-gestures" data-testid="beam-edge-drag" {...pointerHandlers("zoom")}>
				{[-1, 1].map(side => (
					<path key={side} d={`M${edgeStartX} ${center + side * edgeStartHeight} L${right} ${center + side * halfOpening}`} className="focus-zoom-edge-hit fam-beam-angle-hit" />
				))}
				<path d={`M${left} ${center} L${right} ${center - halfOpening} M${left} ${center} L${right} ${center + halfOpening}`} className="focus-zoom-edge" />
			</g>
			<g role="slider" tabIndex={0} aria-disabled={disabled || undefined} aria-label="Focus position" aria-valuemin={Number((focusLimits.minimum * 100).toFixed(percentDecimals))}
				aria-valuemax={Number((focusLimits.maximum * 100).toFixed(percentDecimals))} aria-valuenow={Number(focusText)} aria-valuetext={`${focusText}%`}
				onKeyDown={event => keys("focus", event)} data-testid="focus-position-drag" {...pointerHandlers("focus")}>
				<rect x={planeX - 22} y={center - Math.max(22, planeHalfHeight + 10)} width="44" height={Math.max(44, planeHalfHeight * 2 + 20)} className="focus-zoom-focus-hit fam-focus-hit" />
				<line x1={planeX} y1={center - planeHalfHeight - 10} x2={planeX} y2={center + planeHalfHeight + 10} className="focus-zoom-focus-plane" />
				<rect x={planeX - 13} y={center - 11} width="26" height="22" rx="4" className="focus-zoom-focus-handle fam-focus-handle" />
			</g>
			<g role="slider" tabIndex={0} aria-disabled={disabled || undefined} aria-label={`${conventionLabel} opening angle`} aria-valuemin={zoomLimits.minimum} aria-valuemax={zoomLimits.maximum}
				aria-valuenow={Number(zoomText)} aria-valuetext={`${zoomText} degrees`}
				onKeyDown={event => keys("zoom", event)} data-testid="beam-angle-drag" {...pointerHandlers("zoom")}>
				{[-1, 1].map(side => (
					<g key={side} data-beam-side={side} data-angle-handle data-testid={`beam-angle-handle-${side < 0 ? "upper" : "lower"}`}>
						<line x1={right} y1={center + side * halfOpening} x2={right} y2={center + side * handleOffset} className="focus-zoom-handle-guide" />
						<circle cx={right} cy={center + side * handleOffset} r="22" className="focus-zoom-angle-hit" />
						<circle cx={right} cy={center + side * handleOffset} r="9" className="focus-zoom-angle-handle fam-guide-handle" />
					</g>
				))}
			</g>
			<text x={left} y={size.height - 6} className="focus-zoom-label">Near · {formatFocus(focusLimits.minimum, focusLimits)}%</text>
			<text x={right - 24} y={size.height - 6} textAnchor="end" className="focus-zoom-label">Far · {formatFocus(focusLimits.maximum, focusLimits)}%</text>
		</svg>
	);
}
