import { Input } from "@tosklight/ui/controls";
import { VerticalTouchFaderControl } from "@tosklight/ui/faders";
import {
	useEffect, useLayoutEffect, useRef, useState,
	type CSSProperties, type FocusEvent, type KeyboardEvent, type PointerEvent,
} from "react";
import "./colorPickerControls.css";

/** Ordered endpoints: [first, last]. A descending pair is intentional and is never sorted. */
export type ValueRange = readonly [number, number];
export type RangeGestureSource = "pointer" | "keyboard" | "native";
/** Why a started gesture stopped without a release. */
export type RangeGestureCancelReason = "pointer-cancel" | "lost-capture" | "blur" | "hidden" | "external" | "teardown";
/** Opaque identifier returned by the host's `onGestureStart`; the components never mint one. */
export type RangeGestureId = string | number;

/** One finite operator gesture. Every started gesture receives exactly one end or one cancel. */
export interface RangeGesture {
	/** Host identifier returned from `onGestureStart`, if any. */
	id?: RangeGestureId;
	/** Which control produced the gesture, for example "White Blend", "hue" or "plane". */
	control: string;
	source: RangeGestureSource;
	pointerId?: number;
	/** Keyboard Shift, software SHIFT or hardware `shiftArmed` was active when the gesture began. */
	shifted: boolean;
}

export interface RangeGestureCallbacks {
	/** Return a host gesture ID; it is attached to every later change, end and cancel of the gesture. */
	onGestureStart?(gesture: RangeGesture): RangeGestureId | void;
	/** Pointer released, or a keyboard/native step completed. */
	onGestureEnd?(gesture: RangeGesture): void;
	/** Pointer cancel, lost capture, blur, hidden document, external replacement or unmount. */
	onGestureCancel?(gesture: RangeGesture, reason: RangeGestureCancelReason): void;
}

export interface ControlLimits { minimum: number; maximum: number; step: number }

const finite = (value: number | undefined, fallback: number) => (typeof value === "number" && Number.isFinite(value) ? value : fallback);

/** Finite, ordered limits; a missing or invalid step falls back to 1/100 of the span (or 1). */
export function resolveLimits(minimum: number | undefined, maximum: number | undefined, step: number | undefined, fallback: ControlLimits): ControlLimits {
	let low = finite(minimum, fallback.minimum), high = finite(maximum, fallback.maximum);
	if (low > high) [low, high] = [high, low];
	const span = high - low;
	const resolved = finite(step, fallback.step);
	return { minimum: low, maximum: high, step: resolved > 0 ? resolved : span > 0 ? span / 100 : 1 };
}

function decimals(step: number) {
	const text = String(step), exponent = text.match(/e-(\d+)$/);
	if (exponent) return Number(exponent[1]);
	return text.includes(".") ? text.length - text.indexOf(".") - 1 : 0;
}

export function snapToLimits(value: number, { minimum, maximum, step }: ControlLimits) {
	if (!Number.isFinite(value)) return minimum;
	const stepped = minimum + Math.round((value - minimum) / step) * step;
	return Math.max(minimum, Math.min(maximum, Number(stepped.toFixed(Math.max(6, decimals(step))))));
}

const join = (...names: (string | false | undefined)[]) => names.filter(Boolean).join(" ");
export { join as joinClassNames };

type Point = number[];
type Ranges = (ValueRange | undefined)[];
interface Emission { point: Point; ranges?: ValueRange[] }
interface Plan extends Emission { anchor: Point; pending: boolean }
interface ActivePointer {
	gesture: RangeGesture;
	element: Element;
	pointerId: number;
	x: number;
	y: number;
	/** Geometry captured at gesture start: a resize during the gesture never re-maps the contact. */
	rect: DOMRect;
	shifted: boolean;
	first: Point;
	last: string;
}

export interface EndpointGestureOptions extends RangeGestureCallbacks {
	control: string;
	/** Current supplied value per component (1 for a fader or ring, 2 for the hue/saturation plane). */
	values: Point;
	ranges: Ranges;
	limits: ControlLimits[];
	shiftArmed?: boolean;
	allowRange?: boolean;
	emit(emission: Emission, gesture: RangeGesture): void;
	/** Raw (unsnapped) values under a contact, using the rectangle captured at gesture start. */
	pointAt(event: { clientX: number; clientY: number }, rect: DOMRect): Point;
	/** Raw target for a key from the current (or last-endpoint) values; null leaves the key alone. */
	keyTarget(key: string, current: Point): Point | null;
}

const KEYS = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"];
const startAnchor = (values: Point, ranges: Ranges) => (ranges.some(Boolean) ? values.map((value, i) => ranges[i]?.[0] ?? value) : null);
const near = (a: number, b: number, tolerance: number) => a === b || Math.abs(a - b) < tolerance;
const sameRange = (a: ValueRange | undefined, b: ValueRange | undefined, tolerance: number) =>
	(!a && !b) || (!!a && !!b && near(a[0], b[0], tolerance) && near(a[1], b[1], tolerance));

/**
 * Shared ordered-endpoint interaction for the controlled Color pickers and range faders.
 * An ordinary contact sets one value and becomes the anchor; a shifted contact completes
 * [anchor, contact] or, without an anchor, becomes the pending first endpoint. The host owns
 * every value: a supplied update that is not an echo of this control's own emissions discards
 * the anchor, the pending endpoint and any active gesture, so no stale endpoint is ever emitted.
 */
export function useEndpointGesture(options: EndpointGestureOptions) {
	const latest = useRef(options);
	useLayoutEffect(() => { latest.current = options; });
	const { values, ranges } = options;
	const anchor = useRef<Point | null>(startAnchor(values, ranges));
	const [pending, setPending] = useState(false);
	const active = useRef<ActivePointer | null>(null);
	const recent = useRef<Emission[]>([]);

	const stop = (outcome: "release" | RangeGestureCancelReason) => {
		const current = active.current;
		if (!current) return;
		active.current = null;
		try {
			if (current.element.hasPointerCapture?.(current.pointerId)) current.element.releasePointerCapture(current.pointerId);
		} catch {
			// The element may already be detached during teardown.
		}
		if (outcome === "release") latest.current.onGestureEnd?.(current.gesture);
		else latest.current.onGestureCancel?.(current.gesture, outcome);
	};
	const stopRef = useRef(stop);
	stopRef.current = stop;

	useEffect(() => {
		const hidden = () => { if (document.visibilityState === "hidden") stopRef.current("hidden"); };
		document.addEventListener("visibilitychange", hidden);
		return () => { document.removeEventListener("visibilitychange", hidden); stopRef.current("teardown"); };
	}, []);

	// Controlled replacement: compare each supplied change with this control's own recent emissions.
	const key = JSON.stringify([values, ranges]);
	const observed = useRef(key);
	useLayoutEffect(() => {
		if (observed.current === key) return;
		observed.current = key;
		const tolerance = latest.current.limits.map(limit => limit.step / 2);
		const match = recent.current.findIndex(emission => emission.point.every((value, i) => near(value, values[i], tolerance[i]))
			&& values.every((_, i) => sameRange(emission.ranges?.[i], ranges[i], tolerance[i])));
		if (match >= 0) { recent.current = recent.current.slice(match); return; }
		recent.current = [];
		anchor.current = startAnchor(values, ranges);
		setPending(false);
		stop("external");
	});

	const start = (gesture: RangeGesture): RangeGesture => {
		const id = latest.current.onGestureStart?.(gesture);
		return id === undefined || id === null ? gesture : { ...gesture, id };
	};
	const snap = (point: Point) => point.map((value, i) => snapToLimits(value, latest.current.limits[i]));
	const plan = (next: Point, shifted: boolean): Plan => {
		const first = anchor.current;
		if (shifted && first) return { anchor: first, pending: false, point: first, ranges: first.map((value, i) => [value, next[i]] as const) };
		return { anchor: next, pending: shifted, point: next };
	};
	const apply = (next: Plan, gesture: RangeGesture) => {
		anchor.current = next.anchor;
		setPending(next.pending);
		const emission: Emission = { point: next.point, ranges: next.ranges };
		recent.current = [...recent.current.slice(-31), emission];
		latest.current.emit(emission, gesture);
	};
	const isShifted = (event: { shiftKey: boolean }) => (latest.current.allowRange ?? true) && (event.shiftKey || !!latest.current.shiftArmed);
	const signature = (emission: Emission) => JSON.stringify([emission.point, emission.ranges ?? null]);

	const handlers = {
		onPointerDown(event: PointerEvent<HTMLElement>) {
			if (active.current || (event.pointerType === "mouse" && event.button !== 0)) return;
			event.preventDefault();
			const element = event.currentTarget;
			element.focus();
			try { element.setPointerCapture?.(event.pointerId); } catch { /* Capture is best effort; release, cancel and teardown still end the gesture. */ }
			const rect = element.getBoundingClientRect(), shifted = isShifted(event);
			const gesture = start({ control: latest.current.control, source: "pointer", pointerId: event.pointerId, shifted });
			const next = snap(latest.current.pointAt(event, rect));
			const planned = plan(next, shifted);
			active.current = { gesture, element, pointerId: event.pointerId, x: event.clientX, y: event.clientY, rect, shifted, first: planned.anchor, last: signature(planned) };
			apply(planned, gesture);
		},
		onPointerMove(event: PointerEvent<HTMLElement>) {
			const current = active.current;
			if (!current || current.pointerId !== event.pointerId) return;
			if (Math.hypot(event.clientX - current.x, event.clientY - current.y) < 3) return;
			const next = snap(latest.current.pointAt(event, current.rect));
			const planned: Plan = current.shifted
				? { anchor: current.first, pending: false, point: current.first, ranges: current.first.map((value, i) => [value, next[i]] as const) }
				: { anchor: next, pending: false, point: next };
			const text = signature(planned);
			if (text === current.last) return;
			current.last = text;
			apply(planned, current.gesture);
		},
		onPointerUp(event: PointerEvent<HTMLElement>) { if (active.current?.pointerId === event.pointerId) stop("release"); },
		onPointerCancel(event: PointerEvent<HTMLElement>) { if (active.current?.pointerId === event.pointerId) stop("pointer-cancel"); },
		onLostPointerCapture(event: PointerEvent<HTMLElement>) { if (active.current?.pointerId === event.pointerId) stop("lost-capture"); },
		onBlur(_event: FocusEvent<HTMLElement>) { stop("blur"); },
		onKeyDown(event: KeyboardEvent<HTMLElement>) {
			if (!KEYS.includes(event.key)) return;
			const { values: supplied, ranges: suppliedRanges } = latest.current;
			const shifted = isShifted(event);
			const current = shifted ? supplied.map((value, i) => suppliedRanges[i]?.[1] ?? value) : supplied;
			const target = latest.current.keyTarget(event.key, current);
			if (!target) return;
			event.preventDefault();
			if (active.current) return;
			if (shifted && !anchor.current) anchor.current = [...supplied];
			const planned = plan(snap(target), shifted);
			const unchanged = signature(planned) === signature({ point: supplied, ranges: suppliedRanges.some(Boolean) ? supplied.map((value, i) => suppliedRanges[i] ?? [value, value]) : undefined });
			if (unchanged && planned.pending === pending) { anchor.current = planned.anchor; return; }
			const gesture = start({ control: latest.current.control, source: "keyboard", shifted });
			apply(planned, gesture);
			latest.current.onGestureEnd?.(gesture);
		},
	};
	/** A native scalar edit (assistive technology); suppressed while a pointer gesture owns the control. */
	const native = (value: number) => {
		if (active.current) return;
		const { values: supplied, ranges: suppliedRanges } = latest.current;
		const next = snap([value]);
		if (!suppliedRanges[0] && near(next[0], supplied[0], 1e-9)) return;
		const gesture = start({ control: latest.current.control, source: "native", shifted: false });
		apply({ anchor: next, pending: false, point: next }, gesture);
		latest.current.onGestureEnd?.(gesture);
	};
	return { handlers, pending, native };
}

export type HorizontalRangeFaderSlot = "field" | "fader" | "handle" | "pending";

export interface HorizontalRangeFaderProps extends RangeGestureCallbacks {
	label: string;
	/** Supplied current value. When `range` is set, `value` is its first endpoint. */
	value: number;
	/** Supplied ordered endpoints [first, last]. */
	range?: ValueRange;
	min?: number;
	max?: number;
	step?: number;
	format?(value: number): string;
	/** Colored travel behind the touch face. */
	gradient?: string;
	/** Desk SHIFT (software key or attached hardware) is armed. */
	shiftArmed?: boolean;
	/** False makes every contact a scalar edit, even with Shift. */
	allowRange?: boolean;
	/** Gesture identity; defaults to `label`. */
	control?: string;
	/** Requested value or ordered range. The component never stores it; render the new props. */
	onChange(value: number, range: ValueRange | undefined, gesture: RangeGesture): void;
	/** Extra class names per element, e.g. for an existing story's stable hooks. */
	classNames?: Partial<Record<HorizontalRangeFaderSlot, string>>;
}

const percentFormat = (value: number) => `${Math.round(value)}%`;
const DEFAULT_LIMITS: ControlLimits = { minimum: 0, maximum: 100, step: 1 };

/**
 * Controlled horizontal touch fader on the existing vertical touch-fader face: colored travel, a
 * touched-value indicator and numbered ordered-range endpoint indicators.
 */
export function HorizontalRangeFader({
	label, value, range, min, max, step, format = percentFormat, gradient, shiftArmed, allowRange = true, control,
	onChange, onGestureStart, onGestureEnd, onGestureCancel, classNames = {},
}: HorizontalRangeFaderProps) {
	const limits = resolveLimits(min, max, step, DEFAULT_LIMITS);
	const span = limits.maximum - limits.minimum || 1;
	const { handlers, pending, native } = useEndpointGesture({
		control: control ?? label, values: [value], ranges: [range], limits: [limits], shiftArmed, allowRange,
		onGestureStart, onGestureEnd, onGestureCancel,
		emit: ({ point, ranges }, gesture) => onChange(point[0], ranges?.[0], gesture),
		pointAt: (event, rect) => [limits.minimum + (event.clientX - rect.left) / (rect.width || 1) * span],
		keyTarget: (key, [current]) => [key === "Home" ? limits.minimum : key === "End" ? limits.maximum
			: current + (key === "ArrowLeft" || key === "ArrowDown" ? -limits.step : limits.step)],
	});
	// The pointer handlers own touch contacts. Without this, Chromium's native range slider also
	// consumes the touch, maps it through its thumb inset and emits a scalar change that would
	// replace the ordered endpoints (mouse is already suppressed by preventDefault on pointerdown).
	const field = useRef<HTMLDivElement>(null);
	useEffect(() => {
		const element = field.current;
		if (!element) return;
		const suppress = (event: TouchEvent) => { if ((event.target as Element | null)?.matches?.('input[type="range"]')) event.preventDefault(); };
		element.addEventListener("touchstart", suppress, { passive: false });
		element.addEventListener("touchmove", suppress, { passive: false });
		return () => { element.removeEventListener("touchstart", suppress); element.removeEventListener("touchmove", suppress); };
	}, []);
	const percent = (n: number) => (n - limits.minimum) / span * 100;
	const shown = range?.[1] ?? value;
	const display = range ? `${format(range[0])} → ${format(range[1])}` : format(value);
	const travel = gradient ?? "linear-gradient(90deg, #103039, #176777)";
	return <div ref={field} className={join("horizontal-range-field", classNames.field)} data-control={control ?? label}
		style={{ "--range-fader-gradient": travel, "--fam-fader-gradient": travel } as CSSProperties}>
		<VerticalTouchFaderControl label={label} display={<output aria-label={`${label} value`}>{display}</output>} fraction={percent(shown) / 100}
			className={join("horizontal-range-fader", range && "has-range", classNames.fader)}>
			<Input type="range" aria-label={label} aria-orientation="horizontal" min={limits.minimum} max={limits.maximum} step={limits.step} value={shown}
				aria-valuemin={limits.minimum} aria-valuemax={limits.maximum} aria-valuenow={shown}
				aria-valuetext={range ? `${format(range[0])} through ${format(range[1])}` : format(value)}
				onChange={event => native(Number(event.target.value))} {...handlers} />
			{(range ?? [value]).map((n, i) => <i key={i} aria-hidden="true" className={join("horizontal-range-handle", classNames.handle)}
				data-endpoint={range ? i + 1 : pending ? 1 : 0}
				style={{ left: `clamp(12px, ${percent(n)}%, calc(100% - 12px))` }}>{range ? i + 1 : pending ? "1" : ""}</i>)}
		</VerticalTouchFaderControl>
		{pending && <small className={join("horizontal-range-pending", classNames.pending)}>Shift-click the last value</small>}
	</div>;
}
