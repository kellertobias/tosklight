import type { ProgrammerFixtureValue } from "../../features/programmerValues/contracts";
import type { BuildMediaPaneModelInput } from "./buildMediaPaneModel";
import {
	BLEND_MODE_OPTIONS,
	blendStrobeHz,
	effectLibrarySlotDescription,
	effectLibrarySlotOptions,
	FLIP_MIRROR_OPTIONS,
	MASK_INVERT_OPTIONS,
	nearestBlendValue,
	nearestOpacityCycleValue,
	OPACITY_CYCLE_OPTIONS,
	PLAY_MODE_OPTIONS,
	SCALING_MODE_OPTIONS,
	SPEED_OPTIONS,
} from "./mediaControlOptions";
import type { MediaControlSection } from "./mediaPaneModel";

type MediaControl = MediaControlSection["controls"][number];

interface SpecializedControlContext {
	input: BuildMediaPaneModelInput;
	attribute: string;
	normalized: number;
	rawValue: number;
}

function colourControl({ input, attribute }: SpecializedControlContext) {
	return {
		id: attribute,
		label: "Colour",
		kind: "color",
		value: mediaRgbFromComponents(
			normalizedValue(input.liveProgrammer, "color.red") ?? 1,
			normalizedValue(input.liveProgrammer, "color.green") ?? 1,
			normalizedValue(input.liveProgrammer, "color.blue") ?? 1,
		),
	} satisfies MediaControl;
}

function playModeControl({ attribute, rawValue }: SpecializedControlContext) {
	return {
		id: attribute,
		label: "Play mode",
		kind: "choice",
		value: String(rawValue),
		options: PLAY_MODE_OPTIONS,
		quickActions: [
			{ value: "216", label: "Stop" },
			{ value: "60", label: "Play" },
			{ value: "0", label: "Play looped" },
		],
	} satisfies MediaControl;
}

/** A choice control whose value is the raw DMX value, as a string, after an optional mapping. */
function rawChoice(
	label: string,
	options: Extract<MediaControl, { kind: "choice" }>["options"],
	value: (rawValue: number) => number | string = (rawValue) => rawValue,
) {
	return ({ attribute, rawValue }: SpecializedControlContext): MediaControl => ({
		id: attribute,
		label,
		kind: "choice",
		value: String(value(rawValue)),
		options,
	});
}

function playbackBpmControl({ attribute, rawValue }: SpecializedControlContext) {
	return rawValueControl(
		attribute,
		"Playback BPM",
		rawValue,
		rawValue === 0 ? "Off" : `${rawValue} BPM`,
	);
}

function modelControl({ attribute, rawValue }: SpecializedControlContext) {
	return rawValueControl(
		attribute,
		"3D model",
		rawValue,
		rawValue === 0 ? "Flat" : `Model ${rawValue}`,
	);
}

function blendModeControl({ attribute, rawValue }: SpecializedControlContext) {
	const strobeHz = blendStrobeHz(rawValue);
	return {
		id: attribute,
		label: "Blend mode",
		kind: "choice",
		value: String(nearestBlendValue(rawValue)),
		options: BLEND_MODE_OPTIONS,
		description:
			strobeHz === null ? undefined : `Strobe ${strobeHz.toFixed(1)} Hz`,
	} satisfies MediaControl;
}

function effectSelectControl({
	input,
	attribute,
	rawValue,
}: SpecializedControlContext) {
	return {
		id: attribute,
		label: "Effect Select",
		kind: "choice",
		value: String(rawValue),
		options: effectLibrarySlotOptions(input.effectLibrarySlots),
		description: effectLibrarySlotDescription(
			rawValue,
			input.effectLibrarySlots,
		),
	} satisfies MediaControl;
}

function effectStrengthControl({
	attribute,
	normalized,
	rawValue,
}: SpecializedControlContext) {
	return {
		id: attribute,
		label: "Effect Strength",
		kind: "value",
		value: Math.round(normalized * 100),
		minimum: 0,
		maximum: 100,
		step: 1,
		display: `${Math.round((rawValue / 255) * 100)}%`,
	} satisfies MediaControl;
}

function parameterControl(
	{ attribute, rawValue }: SpecializedControlContext,
	parameter: string,
) {
	return rawValueControl(
		attribute,
		`Parameter ${parameter}`,
		rawValue,
		// Raw 0 is not the bottom of the range: an effect keeps the preset's stored parameter.
		rawValue === 0 ? "Preset" : String(rawValue),
	);
}

/**
 * A dedicated Visualizer Parameter channel, named and ranged by the visualizer the layer shows.
 * Without a shown visualizer, or past the channels it uses, the byte is inert on the server; the
 * fader stays available so a cue can be programmed before the visualizer is shown.
 */
function visualizerParameterControl(
	{ input, attribute, rawValue }: SpecializedControlContext,
	parameter: string,
): MediaControl {
	const index = Number(parameter) - 1;
	const channel = input.visualizerChannels?.find(
		(candidate) => candidate.index === index,
	);
	if (!channel) {
		const known = input.visualizerChannels !== undefined;
		return {
			...rawValueControl(
				attribute,
				`Parameter ${parameter}`,
				rawValue,
				known ? "Unused" : rawValue === 0 ? "Default" : String(rawValue),
			),
			...(known
				? {
						description: input.visualizerChannels?.length
							? "The shown visualizer does not use this channel."
							: "Only a shown visualizer uses this channel.",
					}
				: {}),
		};
	}
	const value =
		rawValue === 0
			? channel.defaultValue
			: visualizerChannelValue(channel, rawValue);
	return rawValueControl(
		attribute,
		channel.label,
		rawValue,
		rawValue === 0
			? `Default · ${formatVisualizerValue(channel, value)}`
			: formatVisualizerValue(channel, value),
	);
}

/** The value a raw byte selects, exactly as the Media Server decodes it. */
export function visualizerChannelValue(
	channel: { parameter: string; minimum: number; maximum: number },
	rawValue: number,
) {
	if (["mirror", "filled", "wireframe"].includes(channel.parameter))
		return rawValue >= 128 ? 1 : 0;
	if (channel.parameter === "mode") return rawValue - 1;
	const value =
		channel.minimum +
		((rawValue - 1) / 254) * (channel.maximum - channel.minimum);
	return channel.parameter === "count" || channel.parameter === "iterations"
		? Math.round(value)
		: value;
}

function formatVisualizerValue(
	channel: { parameter: string; step: number },
	value: number,
) {
	if (["mirror", "filled", "wireframe"].includes(channel.parameter))
		return value >= 0.5 ? "On" : "Off";
	if (channel.parameter === "primary" || channel.parameter === "secondary")
		return `${Math.round(value)}° hue`;
	if (channel.step >= 1) return String(Math.round(value));
	return String(Number(value.toFixed(3)));
}

function rawValueControl(
	attribute: string,
	label: string,
	rawValue: number,
	display: string,
): MediaControl {
	return {
		id: attribute,
		label,
		kind: "value",
		value: rawValue,
		minimum: 0,
		maximum: 255,
		step: 1,
		display,
	};
}

const EXACT_CONTROL_BUILDERS = new Map<
	string,
	(context: SpecializedControlContext) => MediaControl
>([
	["color.tint", colourControl],
	["media.play_mode", playModeControl],
	["media.scaling_mode", rawChoice("Scaling mode", SCALING_MODE_OPTIONS)],
	[
		"media.mask.invert",
		rawChoice("Invert", MASK_INVERT_OPTIONS, (raw) => (raw < 128 ? 0 : 255)),
	],
	["media.playback_speed", rawChoice("Speed", SPEED_OPTIONS)],
	[
		"media.master.effect.opacity_cycle",
		rawChoice(
			"Multiplier / Divider",
			OPACITY_CYCLE_OPTIONS,
			nearestOpacityCycleValue,
		),
	],
	[
		"media.flip_mirror",
		rawChoice("Flip / Mirror", FLIP_MIRROR_OPTIONS, (raw) => raw % 4),
	],
	["media.playback_bpm", playbackBpmControl],
	["media.model", modelControl],
	["media.blend_mode", blendModeControl],
]);

export function specializedControl(
	input: BuildMediaPaneModelInput,
	attribute: string,
	normalized: number,
	rawValue: number,
): MediaControl | undefined {
	const context = { input, attribute, normalized, rawValue };
	const exact = EXACT_CONTROL_BUILDERS.get(attribute);
	if (exact) return exact(context);
	if (/^media\.effect\.bank\.[12]\.select$/u.test(attribute))
		return effectSelectControl(context);
	if (/^media\.effect\.bank\.[12]\.strength$/u.test(attribute))
		return effectStrengthControl(context);
	const visualizer = /^media\.visualizer\.parameter\.([1-4])$/u.exec(attribute);
	if (visualizer) return visualizerParameterControl(context, visualizer[1]);
	const parameter = /^media\.effect\.bank\.[12]\.parameter\.([1-4])$/u.exec(
		attribute,
	);
	if (parameter) return parameterControl(context, parameter[1]);
	return undefined;
}

export function normalizedValue(
	values: readonly ProgrammerFixtureValue[] | undefined,
	attribute: string,
) {
	const value = values?.find(
		(candidate) => candidate.attribute === attribute,
	)?.value;
	return value?.kind === "normalized" && typeof value.value === "number"
		? value.value
		: undefined;
}

function mediaRgbFromComponents(red: number, green: number, blue: number) {
	return `#${[red, green, blue]
		.map((component) =>
			Math.round(Math.max(0, Math.min(1, component)) * 255)
				.toString(16)
				.padStart(2, "0"),
		)
		.join("")}`;
}
