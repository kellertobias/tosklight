import {
	joinClassNames as join, resolveLimits, useEndpointGesture,
	type ControlLimits, type RangeGesture, type RangeGestureCallbacks, type ValueRange,
} from "./HorizontalRangeFader";
import "./colorPickerControls.css";

export type ColorPlanePickerSlot = "root" | "sheet" | "marker" | "caption";

export interface ColorPlanePickerProps extends RangeGestureCallbacks {
	/** Supplied hue (X). With `hueRange`, the first endpoint. */
	hue: number;
	/** Supplied saturation (Y, bottom to top). With `saturationRange`, the first endpoint. */
	saturation: number;
	hueRange?: ValueRange;
	saturationRange?: ValueRange;
	/** Supplied preview color; the picker never solves color. */
	preview: string;
	/** Desk SHIFT (software key or attached hardware) is armed. */
	shiftArmed?: boolean;
	/** False makes every contact a scalar edit, even with Shift. */
	allowRange?: boolean;
	/** Finite hue limits (default 0–359, step 1). */
	hueLimits?: Partial<ControlLimits>;
	/** Finite saturation limits (default 0–100, step 1). */
	saturationLimits?: Partial<ControlLimits>;
	/**
	 * Requested hue/saturation. An ordinary edit carries no ranges and replaces both components;
	 * a shifted edit carries both ordered ranges, starting at the anchor.
	 */
	onChange(hue: number, saturation: number, hueRange: ValueRange | undefined, saturationRange: ValueRange | undefined, gesture: RangeGesture): void;
	classNames?: Partial<Record<ColorPlanePickerSlot, string>>;
}

const HUE: ControlLimits = { minimum: 0, maximum: 359, step: 1 };
const SATURATION: ControlLimits = { minimum: 0, maximum: 100, step: 1 };
const fraction = (value: number) => Math.max(0, Math.min(1, value));

/** Compact full-width 2D picker: hue left to right, saturation bottom to top. */
export function ColorPlanePicker({
	hue, saturation, hueRange, saturationRange, preview, shiftArmed, allowRange = true, hueLimits, saturationLimits,
	onChange, onGestureStart, onGestureEnd, onGestureCancel, classNames = {},
}: ColorPlanePickerProps) {
	const h = resolveLimits(hueLimits?.minimum, hueLimits?.maximum, hueLimits?.step, HUE);
	const s = resolveLimits(saturationLimits?.minimum, saturationLimits?.maximum, saturationLimits?.step, SATURATION);
	const hueSpan = h.maximum - h.minimum || 1, saturationSpan = s.maximum - s.minimum || 1;
	const { handlers, pending } = useEndpointGesture({
		control: "plane", values: [hue, saturation], ranges: [hueRange, saturationRange], limits: [h, s], shiftArmed, allowRange,
		onGestureStart, onGestureEnd, onGestureCancel,
		emit: ({ point, ranges }, gesture) => onChange(point[0], point[1], ranges?.[0], ranges?.[1], gesture),
		pointAt: (event, rect) => [
			h.minimum + fraction((event.clientX - rect.left) / (rect.width || 1)) * hueSpan,
			s.minimum + fraction(1 - (event.clientY - rect.top) / (rect.height || 1)) * saturationSpan,
		],
		keyTarget: (key, [currentHue, currentSaturation]) => {
			if (key === "ArrowLeft" || key === "ArrowRight") return [currentHue + (key === "ArrowRight" ? h.step : -h.step), currentSaturation];
			if (key === "ArrowUp" || key === "ArrowDown") return [currentHue, currentSaturation + (key === "ArrowUp" ? s.step : -s.step)];
			return null;
		},
	});
	const ranged = !!(hueRange || saturationRange);
	const handles = ranged
		? [{ hue: hueRange?.[0] ?? hue, saturation: saturationRange?.[0] ?? saturation }, { hue: hueRange?.[1] ?? hue, saturation: saturationRange?.[1] ?? saturation }]
		: [{ hue, saturation }];
	return <div className={join("color-plane-picker", classNames.root)}>
		<div className={join("color-sheet color-plane-sheet", classNames.sheet)} role="application" aria-label="Color picker"
			aria-description="Hue left to right, saturation bottom to top. Use arrow keys to adjust; hold Shift for range endpoints."
			tabIndex={0} data-testid="color-picker" data-hue={hue} data-saturation={saturation} {...handlers}>
			{handles.map((value, index) => <i key={index} className={join("color-plane-marker", classNames.marker)}
				style={{ left: `clamp(10px, ${(value.hue - h.minimum) / hueSpan * 100}%, calc(100% - 10px))`, top: `clamp(10px, ${100 - (value.saturation - s.minimum) / saturationSpan * 100}%, calc(100% - 10px))` }}>
				{ranged ? index + 1 : pending ? "1" : ""}</i>)}
		</div>
		<div className={join("color-plane-caption", classNames.caption)}><span data-testid="color-preview" style={{ background: preview }} /><span>{pending ? "Shift-click the last color" : "Hue ↔ · Saturation ↕ · Shift for range"}</span></div>
	</div>;
}
