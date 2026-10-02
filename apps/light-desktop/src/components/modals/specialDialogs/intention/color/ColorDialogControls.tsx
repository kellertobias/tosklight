import type { ReactNode } from "react";
import { ColorPlanePicker } from "../ColorPlanePicker";
import {
	HorizontalRangeFader,
	type RangeGesture,
	type RangeGestureCallbacks,
	type ValueRange,
} from "../HorizontalRangeFader";
import { HueRingPicker } from "../HueRingPicker";
import {
	type ColorDescriptors,
	type ColorDialogControl,
	type ColorDialogValues,
	colorControlLimits,
	cssRgb,
	hueSaturationRgb,
	mediaPreviewPixel,
} from "./colorDialogModel";

/** One operator edit: a value or ordered range of one control, within its gesture. */
export type ColorControlEdit = (
	edits: readonly { control: ColorDialogControl; value: number; range?: ValueRange }[],
	gesture: RangeGesture,
) => void;

export interface ColorDialogControlsInput {
	values: ColorDialogValues;
	descriptors: ColorDescriptors;
	shiftArmed: boolean;
	media: boolean;
	edit: ColorControlEdit;
	gestures: Required<RangeGestureCallbacks>;
}

const FORMAT: Record<ColorDialogControl, (value: number) => string> = {
	hue: (value) => `${Math.round(value)}°`,
	saturation: (value) => `${Math.round(value)}%`,
	white_blend: (value) => `${Math.round(value)}%`,
	temperature: (value) => `${Math.round(value)} K`,
	duv: (value) => `${value > 0 ? "+" : ""}${value.toFixed(4)}`,
};

/** Warm → white → cool and magenta → white → green, white exactly at the centre. */
export const TEMPERATURE_GRADIENT =
	"linear-gradient(90deg, #ff902b, #fff 50%, #75aaff)";
export const DUV_GRADIENT = "linear-gradient(90deg, #f783e5, #fff 50%, #77da91)";

const MEDIA_SOURCE = [
	[1, 0.34, 0.22],
	[0.96, 0.77, 0.3],
	[0.51, 0.76, 0.32],
	[0.26, 0.66, 0.81],
	[0.4, 0.35, 0.81],
	[0.82, 0.33, 0.67],
	[0.83, 0.9, 0.93],
	[0, 0, 0],
	[1, 1, 1],
] as const;

/** The shared tint applied to a fixed reference card; Intensity is not part of it. */
export function MediaColorPreview({
	hue,
	saturation,
	whiteBlend,
}: {
	hue: number;
	saturation: number;
	whiteBlend: number;
}) {
	const tint = hueSaturationRgb(hue, saturation);
	return (
		<figure className="media-color-preview" data-testid="media-color-preview">
			<div className="media-color-preview-card" role="img" aria-label="Media color preview">
				{MEDIA_SOURCE.map((source, index) => (
					<i
						// biome-ignore lint/suspicious/noArrayIndexKey: fixed reference card
						key={index}
						style={{ background: cssRgb(mediaPreviewPixel(source, tint, whiteBlend / 100)) }}
					/>
				))}
			</div>
			<figcaption>White Blend greys the picture; Intensity stays separate.</figcaption>
		</figure>
	);
}

/** The controlled pickers and faders of the semantic Color dialog. */
export function colorDialogControls({
	values,
	descriptors,
	shiftArmed,
	media,
	edit,
	gestures,
}: ColorDialogControlsInput) {
	const limits = (control: ColorDialogControl) =>
		colorControlLimits(descriptors[control]);
	const base = cssRgb(hueSaturationRgb(values.hue, values.saturation));
	const fader = (control: ColorDialogControl, label: string, gradient: string) => {
		const { min, max, step } = limits(control);
		return (
			<HorizontalRangeFader
				label={label}
				control={control}
				value={values[control]}
				range={values.ranges[control]}
				min={min}
				max={max}
				step={step}
				format={FORMAT[control]}
				gradient={gradient}
				shiftArmed={shiftArmed}
				onChange={(value, range, gesture) =>
					edit([{ control, value, range }], gesture)
				}
				{...gestures}
			/>
		);
	};
	const whiteBlend = fader(
		"white_blend",
		"White Blend",
		media
			? `linear-gradient(90deg, ${base}, #808080)`
			: `linear-gradient(90deg, ${base}, #fff)`,
	);
	const whiteBalance: ReactNode = (
		<div className="color-white-balance">
			{fader("temperature", "Temperature", TEMPERATURE_GRADIENT)}
			{fader("duv", "Duv", DUV_GRADIENT)}
		</div>
	);
	const hue = limits("hue");
	const saturation = limits("saturation");
	const plane = (
		<ColorPlanePicker
			hue={values.hue}
			saturation={values.saturation}
			hueRange={values.ranges.hue}
			saturationRange={values.ranges.saturation}
			preview={values.preview}
			shiftArmed={shiftArmed}
			hueLimits={{ minimum: hue.min, maximum: hue.max, step: hue.step }}
			saturationLimits={{ minimum: saturation.min, maximum: saturation.max, step: saturation.step }}
			onChange={(nextHue, nextSaturation, hueRange, saturationRange, gesture) =>
				edit(
					[
						{ control: "hue", value: nextHue, range: hueRange },
						{ control: "saturation", value: nextSaturation, range: saturationRange },
					],
					gesture,
				)
			}
			{...gestures}
		/>
	);
	const ring = (
		<HueRingPicker
			preview={values.preview}
			hue={values.hue}
			range={values.ranges.hue}
			saturation={values.saturation}
			saturationRange={values.ranges.saturation}
			shiftArmed={shiftArmed}
			hueStep={hue.step}
			saturationMin={saturation.min}
			saturationMax={saturation.max}
			saturationStep={saturation.step}
			onHue={(value, range, gesture) => edit([{ control: "hue", value, range }], gesture)}
			onSaturation={(value, range, gesture) =>
				edit([{ control: "saturation", value, range }], gesture)
			}
			controls={
				<div className="color-dialog-blend-balance">
					{whiteBlend}
					{!media && whiteBalance}
				</div>
			}
			{...gestures}
		/>
	);
	const mediaPreview = media ? (
		<MediaColorPreview
			hue={values.hue}
			saturation={values.saturation}
			whiteBlend={values.white_blend}
		/>
	) : undefined;
	return { plane, whiteBlend, whiteBalance, ring, mediaPreview };
}
