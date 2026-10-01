import { useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent, type ReactNode } from "react";
import { Input } from "@tosklight/ui/controls";
import { VerticalTouchFaderControl } from "@tosklight/ui/faders";
import { hsvToRgb } from "../../components/modals/specialColor";
import type { Recipe } from "./mockupModel";

export type ValueRange = readonly [number, number];
export type ColorRangeKey = "hue" | "saturation" | "white" | "temperature" | "tint";
export type ColorRanges = Partial<Record<ColorRangeKey, ValueRange>>;
export function recipeHsv(recipe: Recipe, hueFallback = 0) {
	const rgb = [recipe.red, recipe.green, recipe.blue].map(v => v / 100);
	const max = Math.max(...rgb), min = Math.min(...rgb), d = max - min;
	const hue = d === 0 ? hueFallback : ((max === rgb[0] ? (rgb[1] - rgb[2]) / d : max === rgb[1] ? (rgb[2] - rgb[0]) / d + 2 : (rgb[0] - rgb[1]) / d + 4) * 60 + 360) % 360;
	return { hue, saturation: max ? d / max * 100 : 0, brightness: max };
}
export const hueTravel = (start: number, end: number) => ((end - start + 540) % 360) - 180 === -180 ? 180 : ((end - start + 540) % 360) - 180;
export function spreadRecipes(recipe: Recipe, ranges: ColorRanges, count: number, hueFallback = 0) {
	return Array.from({ length: count }, (_, index) => {
		const t = count <= 1 ? 0 : index / (count - 1), hsv = recipeHsv(recipe, hueFallback);
		const value = (key: ColorRangeKey, fallback: number) => ranges[key] ? ranges[key]![0] + (ranges[key]![1] - ranges[key]![0]) * t : fallback;
		const hue = ranges.hue ? (ranges.hue[0] + hueTravel(...ranges.hue) * t + 360) % 360 : hsv.hue;
		const rgb = hsvToRgb({ hue: hue / 360, saturation: value("saturation", hsv.saturation) / 100, brightness: hsv.brightness });
		return { ...recipe, red: rgb[0] * 100, green: rgb[1] * 100, blue: rgb[2] * 100,
			white: value("white", recipe.white), temperature: value("temperature", recipe.temperature), tint: value("tint", recipe.tint) };
	});
}

type GestureProps = { value: number; range?: ValueRange; min: number; max: number; step: number; shiftArmed?: boolean; allowRange?: boolean; onChange(value: number, range?: ValueRange): void };
function useRangeGesture({ value, range, min, max, step, shiftArmed, allowRange = true, onChange }: GestureProps, at: (event: PointerEvent<HTMLElement>) => number) {
	const anchor = useRef<number | null>(range?.[0] ?? null);
	const [pending, setPending] = useState<number | null>(null);
	const drag = useRef<{ shifted: boolean; first: number; x: number; y: number } | null>(null);
	const snap = (n: number) => Math.max(min, Math.min(max, Number((min + Math.round((n - min) / step) * step).toFixed(6))));
	const choose = (n: number, shifted: boolean) => {
		if (shifted && anchor.current !== null) { onChange(anchor.current, [anchor.current, n]); setPending(null); }
		else { anchor.current = n; onChange(n); setPending(shifted ? n : null); }
	};
	const handlers = {
		onPointerDown(event: PointerEvent<HTMLElement>) {
			event.preventDefault(); event.currentTarget.focus(); event.currentTarget.setPointerCapture(event.pointerId);
			const n = snap(at(event)), shifted = allowRange && (event.shiftKey || !!shiftArmed);
			choose(n, shifted); drag.current = { shifted, first: anchor.current ?? n, x: event.clientX, y: event.clientY };
		},
		onPointerMove(event: PointerEvent<HTMLElement>) {
			if (!drag.current || !event.currentTarget.hasPointerCapture(event.pointerId)) return;
			if (Math.hypot(event.clientX - drag.current.x, event.clientY - drag.current.y) < 3) return;
			const n = snap(at(event));
			if (drag.current.shifted) { onChange(drag.current.first, [drag.current.first, n]); setPending(null); }
			else { anchor.current = n; onChange(n); }
		},
		onPointerUp(event: PointerEvent<HTMLElement>) { drag.current = null; if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId); },
		onPointerCancel() { drag.current = null; },
		onKeyDown(event: KeyboardEvent<HTMLElement>) {
			if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) return;
			event.preventDefault(); const shifted = allowRange && (event.shiftKey || !!shiftArmed);
			const current = shifted && range ? range[1] : value;
			const n = snap(event.key === "Home" ? min : event.key === "End" ? max : current + (["ArrowLeft", "ArrowDown"].includes(event.key) ? -step : step));
			if (shifted && anchor.current === null) anchor.current = value;
			choose(n, shifted);
		},
	};
	return { handlers, pending };
}

export function RangeFader({ label, value, range, min = 0, max = 100, step = 1, format = v => `${Math.round(v)}%`, gradient, shiftArmed, allowRange = true, onChange }: {
	label: string; value: number; range?: ValueRange; min?: number; max?: number; step?: number;
	format?(v: number): string; gradient?: string; shiftArmed?: boolean; allowRange?: boolean; onChange(value: number, range?: ValueRange): void;
}) {
	const { handlers, pending } = useRangeGesture({ value, range, min, max, step, shiftArmed, allowRange, onChange }, event => {
		const rect = event.currentTarget.getBoundingClientRect(); return min + (event.clientX - rect.left) / rect.width * (max - min);
	});
	const percent = (n: number) => (n - min) / (max - min) * 100;
	const display = range ? `${format(range[0])} → ${format(range[1])}` : format(value);
	return <div className="fam-range-field" style={{ "--fam-fader-gradient": gradient ?? "linear-gradient(90deg, #103039, #176777)" } as CSSProperties}>
		<VerticalTouchFaderControl label={label} display={<output aria-label={`${label} value`}>{display}</output>} fraction={percent(range?.[1] ?? value) / 100} className={`fam-range-fader${range ? " has-range" : ""}`}>
			<Input type="range" aria-label={label} aria-orientation="horizontal" min={min} max={max} step={step} value={range?.[1] ?? value}
				aria-valuemin={min} aria-valuemax={max} aria-valuenow={range?.[1] ?? value} aria-valuetext={range ? `${format(range[0])} through ${format(range[1])}` : format(value)}
				onChange={event => onChange(Number(event.target.value))} {...handlers} />
			{(range ?? [value]).map((n, i) => <i key={i} aria-hidden="true" className="fam-range-handle" style={{ left: `clamp(12px, ${percent(n)}%, calc(100% - 12px))` }}>{range ? i + 1 : pending !== null ? "1" : ""}</i>)}
		</VerticalTouchFaderControl>
		{pending !== null && <small className="fam-range-pending">Shift-click the last value</small>}
	</div>;
}

export function HueRing({ preview, hue, range, saturation, saturationRange, shiftArmed, onHue, onSaturation, controls }: {
	controls?: ReactNode; preview: string; hue: number; range?: ValueRange; saturation: number; saturationRange?: ValueRange; shiftArmed?: boolean;
	onHue(value: number, range?: ValueRange): void; onSaturation(value: number, range?: ValueRange): void;
}) {
	const { handlers, pending } = useRangeGesture({ value: hue, range, min: 0, max: 359, step: 1, shiftArmed, onChange: onHue }, event => {
		const rect = event.currentTarget.getBoundingClientRect(); return (Math.atan2(event.clientX - rect.left - rect.width / 2, -(event.clientY - rect.top - rect.height / 2)) * 180 / Math.PI + 360) % 360;
	});
	return <div className="fam-hue-picker">
		<div className="fam-hue-ring" role="slider" aria-label="Hue" aria-valuemin={0} aria-valuemax={359} aria-valuenow={range?.[1] ?? hue}
			aria-valuetext={range ? `${range[0]} through ${range[1]} degrees` : `${Math.round(hue)} degrees`} tabIndex={0} {...handlers}>
			<div className="fam-hue-center"><span data-testid="color-preview" style={{ background: preview }} /><output aria-label="Hue value">{range ? `${Math.round(range[0])}° → ${Math.round(range[1])}°` : `${Math.round(hue)}°`}</output></div>
			{(range ?? [hue]).map((n, i) => <span key={i} className="fam-hue-handle" style={{ left: `${50 + Math.sin(n * Math.PI / 180) * 42}%`, top: `${50 - Math.cos(n * Math.PI / 180) * 42}%` }}>{range ? i + 1 : pending !== null ? "1" : ""}</span>)}
		</div>
		<div className="fam-saturation-control"><RangeFader label="Saturation" value={saturation} range={saturationRange} gradient={`linear-gradient(90deg, #fff, hsl(${hue} 100% 50%))`} shiftArmed={shiftArmed} onChange={onSaturation} />
			{controls}<small>{pending !== null ? "Shift-click the last hue" : "Shift-click first and last to spread"}</small>
		</div>
	</div>;
}
