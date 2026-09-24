import type { MediaPaneModel } from "../../../../light-desktop/src/windows/media/MediaPaneSurface";
import {
	type ClipLength,
	clipLengthReadout,
	pointDisplay,
} from "../../../../light-desktop/src/windows/media/mediaPointTime";
import { resolveAddress } from "../../entities/catalog";
import type { api } from "../../shared/api/client";
import type {
	CatalogView,
	ModelSlotView,
	OutputView,
	UpdateLayer,
	VisualizerChannelView,
	VisualizerParametersView,
} from "../../shared/api/generated/media-wire";

type LayerState = OutputView["layers"][number];
type ControlSection = MediaPaneModel["controlSections"][number];

export function valueControl(
	id: string,
	label: string,
	value: number,
	minimum: number,
	maximum: number,
	disabled: boolean,
	suffix = "",
	step = 0.1,
) {
	return {
		id,
		kind: "value" as const,
		label,
		value,
		minimum,
		maximum,
		disabled,
		step,
		display: `${Number(value.toFixed(2))}${suffix}`,
	};
}

const BLEND_MODES: Array<[string, string]> = [
	["normal", "Normal"],
	["add", "Add"],
	["screen", "Screen"],
	["multiply", "Multiply"],
	["overlay", "Overlay"],
	["difference", "Difference"],
	["lighten", "Lighten"],
	["darken", "Darken"],
];
/** The Blend mode / Strobe byte strobes from 1 Hz at 128 to 25 Hz at 249. */
const STROBE_OFF = 127;
const STROBE_FASTEST = 249;

/** Translates the Blend, Playback range, Visualizer byte, effect-bank and 3D model controls into layer updates. */
export function layerDmxChange(
	id: string,
	value: string | number,
): UpdateLayer | undefined {
	const number = Number(value);
	const parameter = /^media\.effect\.bank\.(1|2)\.parameter\.([1-4])$/.exec(id);
	if (parameter)
		return {
			effectBank: Number(parameter[1]) - 1,
			effectParameterIndex: Number(parameter[2]) - 1,
			effectParameterValue: Math.round(number),
		};
	const bank = /^media\.effect\.bank\.(1|2)\.(select|strength)$/.exec(id);
	if (bank)
		return {
			effectBank: Number(bank[1]) - 1,
			...(bank[2] === "select"
				? { effectSelect: number }
				: { effectStrength: number / 100 }),
		};
	const visualizer = /^media\.visualizer\.parameter\.([1-4])$/.exec(id);
	if (visualizer)
		return {
			visualizerParameterIndex: Number(visualizer[1]) - 1,
			visualizerParameterValue: Math.round(number),
		};
	switch (id) {
		case "blend-mode":
			return {
				blendDmx: BLEND_MODES.findIndex(([mode]) => mode === value) * 16,
			};
		case "strobe":
			// The fader's bottom stop is Off, which is Normal blending without strobe.
			return { blendDmx: number <= STROBE_OFF ? 0 : Math.round(number) };
		case "in-point":
			return { inPoint: Math.round(number) };
		case "out-point":
			return { outPoint: Math.round(number) };
		case "model":
			return { model: Math.round(number) };
		case "model-pan":
			return { modelPan: number };
		case "model-tilt":
			return { modelTilt: number };
		default:
			return undefined;
	}
}

/** The group heading a shown Visualizer's controls carry on the Effects tab. */
export const VISUALIZER_GROUP = "Visualizer";

function strobeByte(strobeHz: number | null) {
	if (strobeHz === null) return STROBE_OFF;
	return Math.round(128 + ((strobeHz - 1) / 24) * (STROBE_FASTEST - 128));
}

export function blendSection(
	layer: LayerState,
	disabled: boolean,
): ControlSection {
	return {
		id: "blend",
		label: "Blend",
		controls: [
			{
				id: "blend-mode",
				kind: "choice",
				label: "Blend mode",
				value: layer.blendMode,
				options: BLEND_MODES.map(([value, label]) => ({ value, label })),
				disabled,
			},
			{
				...valueControl(
					"strobe",
					"Strobe",
					strobeByte(layer.strobeHz),
					STROBE_OFF,
					STROBE_FASTEST,
					disabled,
					"",
					1,
				),
				display:
					layer.strobeHz === null ? "Off" : `${layer.strobeHz.toFixed(1)} Hz`,
				description: "Strobing blends Normal; choosing a blend mode stops it.",
			},
		],
	};
}

/**
 * In and Out point, shown at the end of the Playback tab under a Playback range heading. They are
 * shown and typed as `mm:ss.ff` at the server's point frame rate, exactly as the desk Media pane
 * shows them; the channel values stay frame counts.
 */
/** What the catalog says about the length of the clip a layer shows. */
export function clipLengthOf(
	item: { kind: string; durationMillis?: number | null } | undefined,
): ClipLength {
	if (!item) return { kind: "none" };
	if (item.kind !== "video") return { kind: "still" };
	return item.durationMillis == null
		? { kind: "unknown" }
		: { kind: "known", seconds: item.durationMillis / 1_000 };
}

/** The playback range of the selected layer: its clip's own length, then its In and Out points. */
export function playbackRangeControls(
	{ layer, output }: { layer: LayerState; output: { frameRate: number } },
	disabled: boolean,
	catalog?: CatalogView,
): ControlSection["controls"] {
	const framesPerSecond = output.frameRate;
	const clip = resolveAddress(catalog, layer.address.folder, layer.address.file);
	const length = clipLengthReadout(clipLengthOf(clip.item), framesPerSecond);
	return [
		{
			id: "clip-length",
			label: "Clip length",
			kind: "readout" as const,
			value: length.value,
			description: length.description,
			group: "Playback range",
			// Back to the whole clip at once: In and Out both 0, offered while either is set.
			action: {
				label: "Clear playback range",
				disabled: disabled || (layer.inPoint === 0 && layer.outPoint === 0),
				changes: [
					{ controlId: "in-point", value: 0 },
					{ controlId: "out-point", value: 0 },
				],
			},
		},
		...(
		[
			["in-point", "In point", layer.inPoint, "start"],
			["out-point", "Out point", layer.outPoint, "end"],
		] as const
	).map(([id, label, frames, reference]) => ({
		id,
		label,
		kind: "point-time" as const,
		value: frames,
		reference,
		framesPerSecond,
		display: pointDisplay(reference, frames, framesPerSecond),
		disabled,
		group: "Playback range",
	})),
	];
}

/**
 * The layer's four dedicated Visualizer Parameter bytes, named and ranged by the visualizer it
 * shows. They sit in the Visualizer group on the Effects tab, apart from the two effect banks; a
 * byte the shown visualizer does not use is inert and disabled.
 */
export function visualizerParameterControls(
	layer: LayerState,
	disabled: boolean,
): ControlSection["controls"] {
	return Array.from({ length: 4 }, (_, index) => {
		const raw = layer.visualizerControls[index] ?? 0;
		const channel = layer.visualizerChannels.find(
			(candidate) => candidate.index === index,
		);
		return {
			...valueControl(
				`media.visualizer.parameter.${index + 1}`,
				channel
					? `Parameter ${index + 1} · ${channel.label}`
					: `Parameter ${index + 1}`,
				raw,
				0,
				255,
				disabled || channel === undefined,
				"",
				1,
			),
			display: channel
				? raw === 0
					? `Default · ${visualizerChannelDisplay(channel, channel.defaultValue)}`
					: visualizerChannelDisplay(channel, channel.value)
				: "Unused",
			group: VISUALIZER_GROUP,
		};
	});
}

function visualizerChannelDisplay(
	channel: VisualizerChannelView,
	value: number,
) {
	if (["mirror", "filled", "wireframe"].includes(channel.parameter))
		return value >= 0.5 ? "On" : "Off";
	if (channel.parameter === "on-beat") return value >= 0.5 ? "Beat" : "Audio";
	if (channel.parameter === "primary" || channel.parameter === "secondary")
		return `${Math.round(value)}° hue`;
	if (channel.step >= 1) return String(Math.round(value));
	return String(Number(value.toFixed(3)));
}

/** Scale, placement and Rotation, then the 3D model that Rotation rolls. */
export function frameSection(
	layer: LayerState,
	models: ModelSlotView[],
	disabled: boolean,
): ControlSection {
	return {
		id: "frame",
		label: "Frame",
		controls: [
			valueControl("scale-x", "Scale X", layer.scaleX, 0, 10, disabled),
			valueControl("scale-y", "Scale Y", layer.scaleY, 0, 10, disabled),
			{
				id: "scaling-mode",
				kind: "choice",
				label: "Scaling mode",
				value: layer.scalingMode,
				options: ["fit", "fill", "original", "stretch"].map((value) => ({
					value,
					label: value,
				})),
				disabled,
			},
			valueControl(
				"position-x",
				"Position X",
				layer.positionX,
				-2,
				2,
				disabled,
			),
			valueControl(
				"position-y",
				"Position Y",
				layer.positionY,
				-2,
				2,
				disabled,
			),
			valueControl(
				"rotation",
				"Rotation",
				layer.rotation,
				-360,
				360,
				disabled,
				"°",
			),
			...modelControls(layer, models, disabled),
		],
	};
}

/** The 3D model heading on the Frame tab. */
const MODEL_GROUP = "3D model";

/**
 * 3D model selection, Pan and Tilt, shown on the Frame tab after Rotation, which is the model's
 * roll. Flat at Pan 0 and Tilt 0 is the baseline; Pan and Tilt still turn a Flat layer.
 */
function modelControls(
	layer: LayerState,
	models: ModelSlotView[],
	disabled: boolean,
): ControlSection["controls"] {
	const options = [
		{ value: "0", label: "Flat" },
		...models.map((model) => ({
			value: String(model.slot),
			label: `${model.slot} · ${model.name}`,
		})),
	];
	if (!options.some((option) => option.value === String(layer.model)))
		options.push({
			value: String(layer.model),
			label: `${layer.model} · Missing`,
		});
	return [
		{
			id: "model",
			kind: "choice",
			label: "Model",
			value: String(layer.model),
			options,
			disabled,
			group: MODEL_GROUP,
			description: "Rotation above is the model's roll.",
		},
		{
			...valueControl(
				"model-pan",
				"Pan",
				layer.modelPan,
				-360,
				360,
				disabled,
				"°",
				1,
			),
			group: MODEL_GROUP,
		},
		{
			...valueControl(
				"model-tilt",
				"Tilt",
				layer.modelTilt,
				-360,
				360,
				disabled,
				"°",
				1,
			),
			group: MODEL_GROUP,
		},
	];
}

export function effectBankSection(
	banks: LayerState["effectBanks"],
	presets: Awaited<ReturnType<typeof api.effects>>,
	disabled: boolean,
): ControlSection {
	const names = new Map(presets.map((preset) => [preset.slot, preset.name]));
	const parameterLabels = new Map(
		presets.map((preset) => [
			preset.slot,
			preset.effect.parameters.map((parameter) => parameter.label),
		]),
	);
	const options = Array.from({ length: 256 }, (_, slot) => ({
		value: String(slot),
		label:
			slot === 0
				? "Off"
				: names.has(slot)
					? `${slot} · ${names.get(slot)}`
					: `${slot} · Unassigned`,
	}));
	return {
		id: "effects",
		label: "Effects",
		controls: banks.flatMap((bank) => {
			const number = bank.index + 1;
			return [
				{
					id: `media.effect.bank.${number}.select`,
					kind: "choice" as const,
					label: "Effect Select",
					value: String(bank.select),
					options,
					disabled,
				},
				valueControl(
					`media.effect.bank.${number}.strength`,
					"Effect Strength",
					bank.strength * 100,
					0,
					100,
					disabled,
					"%",
				),
				// Zero keeps the preset's stored value; 1–255 spans the parameter's range.
				...bank.parameters.map((raw, index) => {
					const meaning = parameterLabels.get(bank.select)?.[index];
					return {
						...valueControl(
							`media.effect.bank.${number}.parameter.${index + 1}`,
							meaning
								? `Parameter ${index + 1} · ${meaning}`
								: `Parameter ${index + 1}`,
							raw,
							0,
							255,
							disabled || meaning === undefined,
							"",
							1,
						),
						display: raw === 0 ? "Preset" : String(raw),
					};
				}),
			];
		}),
	};
}

export const VISUALIZER_NUMBERS: Record<
	string,
	{
		field: keyof VisualizerParametersView;
		label: string;
		minimum: number;
		maximum: number;
		step: number;
	}
> = {
	audioGain: {
		field: "audioGain",
		label: "Audio gain",
		minimum: 0,
		maximum: 8,
		step: 0.1,
	},
	count: { field: "count", label: "Count", minimum: 1, maximum: 512, step: 1 },
	size: {
		field: "size",
		label: "Size",
		minimum: 0.001,
		maximum: 1,
		step: 0.001,
	},
	speed: { field: "speed", label: "Speed", minimum: 0, maximum: 8, step: 0.1 },
	amount: {
		field: "amount",
		label: "Amount",
		minimum: 0,
		maximum: 1,
		step: 0.01,
	},
	radius: {
		field: "radius",
		label: "Radius",
		minimum: 0,
		maximum: 1,
		step: 0.01,
	},
	thickness: {
		field: "thickness",
		label: "Thickness",
		minimum: 0.0005,
		maximum: 0.5,
		step: 0.0005,
	},
	reactivity: {
		field: "reactivity",
		label: "Reactivity",
		minimum: 0,
		maximum: 8,
		step: 0.1,
	},
	decay: { field: "decay", label: "Decay", minimum: 0, maximum: 1, step: 0.01 },
	zoom: {
		field: "zoom",
		label: "Zoom",
		minimum: 0.05,
		maximum: 16,
		step: 0.05,
	},
	iterations: {
		field: "iterations",
		label: "Iterations",
		minimum: 1,
		maximum: 256,
		step: 1,
	},
	threshold: {
		field: "threshold",
		label: "Threshold",
		minimum: 0,
		maximum: 1,
		step: 0.01,
	},
	smoothing: {
		field: "smoothing",
		label: "Smoothing",
		minimum: 0,
		maximum: 1,
		step: 0.01,
	},
	gravity: {
		field: "gravity",
		label: "Gravity",
		minimum: -4,
		maximum: 4,
		step: 0.1,
	},
	lifetime: {
		field: "lifetime",
		label: "Lifetime",
		minimum: 0.05,
		maximum: 60,
		step: 0.05,
	},
	curvature: {
		field: "curvature",
		label: "Curvature",
		minimum: 0,
		maximum: 1,
		step: 0.01,
	},
	mode: { field: "mode", label: "Variant", minimum: 0, maximum: 255, step: 1 },
	burst: { field: "burst", label: "Per beat", minimum: 0, maximum: 8, step: 1 },
};

export const VISUALIZER_FLAGS: Record<
	string,
	{ field: "mirror" | "filled" | "wireframe"; label: string }
> = {
	mirror: { field: "mirror", label: "Mirror" },
	filled: { field: "filled", label: "Filled" },
	wireframe: { field: "wireframe", label: "Wireframe" },
};
