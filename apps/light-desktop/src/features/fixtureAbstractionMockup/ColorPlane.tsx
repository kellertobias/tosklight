import { ColorPlanePicker } from "../../components/modals/specialDialogs/intention/ColorPlanePicker";
import type { ValueRange } from "./RangeControls";

const famClasses = { root: "fam-color-plane", sheet: "fam-2d-sheet", marker: "fam-plane-marker", caption: "fam-2d-caption" };

/** Thin mockup adapter: the controlled ColorPlanePicker with this story's stable class hooks. */
export function ColorPlane({ hue, saturation, hueRange, saturationRange, preview, shiftArmed, onChange }: {
	hue: number; saturation: number; hueRange?: ValueRange; saturationRange?: ValueRange; preview: string; shiftArmed?: boolean;
	onChange(hue: number, saturation: number, hueRange?: ValueRange, saturationRange?: ValueRange): void;
}) {
	return <ColorPlanePicker hue={hue} saturation={saturation} hueRange={hueRange} saturationRange={saturationRange} preview={preview} shiftArmed={shiftArmed}
		onChange={(nextHue, nextSaturation, nextHueRange, nextSaturationRange) => onChange(nextHue, nextSaturation, nextHueRange, nextSaturationRange)} classNames={famClasses} />;
}
