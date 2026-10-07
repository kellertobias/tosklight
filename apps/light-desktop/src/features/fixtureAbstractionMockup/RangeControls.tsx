import type { ReactNode } from "react";
import { HorizontalRangeFader, type ValueRange } from "../../components/modals/specialDialogs/intention/HorizontalRangeFader";
import { HueRingPicker } from "../../components/modals/specialDialogs/intention/HueRingPicker";
import { hsvToRgb } from "../../components/modals/specialColor";
import type { Recipe } from "./mockupModel";

export type { ValueRange };
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

const famFader = { field: "fam-range-field", fader: "fam-range-fader", handle: "fam-range-handle", pending: "fam-range-pending" };
const famHue = { root: "fam-hue-picker", ring: "fam-hue-ring", center: "fam-hue-center", handle: "fam-hue-handle", side: "fam-saturation-control" };

/** Thin mockup adapter: the controlled HorizontalRangeFader with this story's stable class hooks. */
export function RangeFader({ label, value, range, min = 0, max = 100, step = 1, format, gradient, shiftArmed, allowRange = true, onChange }: {
	label: string; value: number; range?: ValueRange; min?: number; max?: number; step?: number;
	format?(v: number): string; gradient?: string; shiftArmed?: boolean; allowRange?: boolean; onChange(value: number, range?: ValueRange): void;
}) {
	return <HorizontalRangeFader label={label} value={value} range={range} min={min} max={max} step={step} format={format} gradient={gradient}
		shiftArmed={shiftArmed} allowRange={allowRange} onChange={(next, nextRange) => onChange(next, nextRange)} classNames={famFader} />;
}

/** Thin mockup adapter: the controlled HueRingPicker with this story's stable class hooks. */
export function HueRing({ preview, hue, range, saturation, saturationRange, shiftArmed, onHue, onSaturation, controls }: {
	controls?: ReactNode; preview: string; hue: number; range?: ValueRange; saturation: number; saturationRange?: ValueRange; shiftArmed?: boolean;
	onHue(value: number, range?: ValueRange): void; onSaturation(value: number, range?: ValueRange): void;
}) {
	return <HueRingPicker preview={preview} hue={hue} range={range} saturation={saturation} saturationRange={saturationRange} shiftArmed={shiftArmed}
		onHue={(next, nextRange) => onHue(next, nextRange)} onSaturation={(next, nextRange) => onSaturation(next, nextRange)} controls={controls}
		classNames={famHue} faderClassNames={famFader} />;
}
