import type { ReactNode } from "react";
import {
	HorizontalRangeFader, joinClassNames as join, resolveLimits, useEndpointGesture,
	type HorizontalRangeFaderSlot, type RangeGesture, type RangeGestureCallbacks, type ValueRange,
} from "./HorizontalRangeFader";
import "./colorPickerControls.css";

export type HueRingPickerSlot = "root" | "ring" | "center" | "handle" | "side";

export interface HueRingPickerProps extends RangeGestureCallbacks {
	/** Supplied preview color; the picker never solves color. */
	preview: string;
	/** Supplied hue in degrees. With `range`, the first endpoint. */
	hue: number;
	/** Ordered hue endpoints [first, last]. Travel direction is the conversion owner's rule. */
	range?: ValueRange;
	saturation: number;
	saturationRange?: ValueRange;
	/** Desk SHIFT (software key or attached hardware) is armed. */
	shiftArmed?: boolean;
	/** Hue step in degrees (default 1). */
	hueStep?: number;
	saturationMin?: number;
	saturationMax?: number;
	saturationStep?: number;
	/** Requested hue or ordered hue range. Only the hue is replaced; the saturation range is untouched. */
	onHue(value: number, range: ValueRange | undefined, gesture: RangeGesture): void;
	/** Requested saturation or ordered saturation range. Only the saturation is replaced. */
	onSaturation(value: number, range: ValueRange | undefined, gesture: RangeGesture): void;
	/** Extra faders shown under Saturation (White Blend, Temperature, Duv). */
	controls?: ReactNode;
	classNames?: Partial<Record<HueRingPickerSlot, string>>;
	/** Extra class names for the Saturation fader. */
	faderClassNames?: Partial<Record<HorizontalRangeFaderSlot, string>>;
}

/** Expanded accurate hue ring (0° at the top, clockwise) with a colored Saturation touch fader. */
export function HueRingPicker({
	preview, hue, range, saturation, saturationRange, shiftArmed, hueStep, saturationMin = 0, saturationMax = 100, saturationStep = 1,
	onHue, onSaturation, controls, onGestureStart, onGestureEnd, onGestureCancel, classNames = {}, faderClassNames,
}: HueRingPickerProps) {
	const limits = resolveLimits(0, 359, hueStep, { minimum: 0, maximum: 359, step: 1 });
	const { handlers, pending } = useEndpointGesture({
		control: "hue", values: [hue], ranges: [range], limits: [limits], shiftArmed,
		onGestureStart, onGestureEnd, onGestureCancel,
		emit: ({ point, ranges }, gesture) => onHue(point[0], ranges?.[0], gesture),
		pointAt: (event, rect) => [(Math.atan2(event.clientX - rect.left - rect.width / 2, -(event.clientY - rect.top - rect.height / 2)) * 180 / Math.PI + 360) % 360],
		keyTarget: (key, [current]) => [key === "Home" ? limits.minimum : key === "End" ? limits.maximum
			: current + (key === "ArrowLeft" || key === "ArrowDown" ? -limits.step : limits.step)],
	});
	return <div className={join("hue-ring-picker", classNames.root)}>
		<div className={join("hue-ring", classNames.ring)} role="slider" aria-label="Hue" aria-valuemin={0} aria-valuemax={359} aria-valuenow={range?.[1] ?? hue}
			aria-valuetext={range ? `${range[0]} through ${range[1]} degrees` : `${Math.round(hue)} degrees`} tabIndex={0} {...handlers}>
			<div className={join("hue-ring-center", classNames.center)}><span data-testid="color-preview" style={{ background: preview }} /><output aria-label="Hue value">{range ? `${Math.round(range[0])}° → ${Math.round(range[1])}°` : `${Math.round(hue)}°`}</output></div>
			{(range ?? [hue]).map((n, i) => <span key={i} className={join("hue-ring-handle", classNames.handle)} style={{ left: `${50 + Math.sin(n * Math.PI / 180) * 42}%`, top: `${50 - Math.cos(n * Math.PI / 180) * 42}%` }}>{range ? i + 1 : pending ? "1" : ""}</span>)}
		</div>
		<div className={join("hue-ring-side", classNames.side)}>
			<HorizontalRangeFader label="Saturation" control="saturation" value={saturation} range={saturationRange} min={saturationMin} max={saturationMax} step={saturationStep}
				gradient={`linear-gradient(90deg, #fff, hsl(${hue} 100% 50%))`} shiftArmed={shiftArmed} onChange={onSaturation}
				onGestureStart={onGestureStart} onGestureEnd={onGestureEnd} onGestureCancel={onGestureCancel} classNames={faderClassNames} />
			{controls}<small>{pending ? "Shift-click the last hue" : "Shift-click first and last to spread"}</small>
		</div>
	</div>;
}
