import type { MediaPaneModel } from "../../../../light-desktop/src/windows/media/MediaPaneSurface";
import type { api } from "../../shared/api/client";
import type {
	OutputView,
	UpdateLayer,
	VisualizerParametersView,
	VisualizerView,
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

/** Translates the effect-bank and mapping-layout controls into layer updates. */
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

/** The mapping layout's Blend, Playback range, Visualizer, and 3D mapping sections. */
export function layerDmxSections(
	layer: LayerState,
	visualizer: VisualizerView | undefined,
	disabled: boolean,
): ControlSection[] {
	return [
		blendSection(layer, disabled),
		playbackRangeSection(layer, disabled),
		layerVisualizerSection(layer, visualizer, disabled),
		mappingSection(layer, disabled),
	];
}

function strobeByte(strobeHz: number | null) {
	if (strobeHz === null) return STROBE_OFF;
	return Math.round(128 + ((strobeHz - 1) / 24) * (STROBE_FASTEST - 128));
}

function blendSection(layer: LayerState, disabled: boolean): ControlSection {
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

function playbackRangeSection(
	layer: LayerState,
	disabled: boolean,
): ControlSection {
	return {
		id: "playback-range",
		label: "Playback range",
		controls: [
			{
				...valueControl(
					"in-point",
					"In point",
					layer.inPoint,
					0,
					65535,
					disabled,
					"",
					1,
				),
				display: `Frame ${layer.inPoint}`,
			},
			{
				...valueControl(
					"out-point",
					"Out point",
					layer.outPoint,
					0,
					65535,
					disabled,
					"",
					1,
				),
				display:
					layer.outPoint === 0
						? "End of clip"
						: `${layer.outPoint} frames before end`,
			},
		],
	};
}

function visualizerParameterName(parameter: string) {
	if (parameter === "primary") return "Colour";
	if (parameter === "secondary") return "Second colour";
	return (
		VISUALIZER_NUMBERS[parameter]?.label ??
		VISUALIZER_FLAGS[parameter]?.label ??
		parameter
	);
}

/** Four DMX bytes follow the shown visualizer kind's own parameter order. */
function layerVisualizerSection(
	layer: LayerState,
	visualizer: VisualizerView | undefined,
	disabled: boolean,
): ControlSection {
	return {
		id: "visualizer",
		label: "Visualizer",
		controls: Array.from({ length: 4 }, (_, index) => {
			const raw = layer.visualizerControls[index] ?? 0;
			const parameter = visualizer?.uses[index];
			return {
				...valueControl(
					`media.visualizer.parameter.${index + 1}`,
					parameter
						? `Parameter ${index + 1} · ${visualizerParameterName(parameter)}`
						: `Parameter ${index + 1}`,
					raw,
					0,
					255,
					disabled || parameter === undefined,
					"",
					1,
				),
				display: raw === 0 ? "Configured" : String(raw),
			};
		}),
	};
}

function mappingSection(layer: LayerState, disabled: boolean): ControlSection {
	return {
		id: "mapping",
		label: "3D mapping",
		controls: [
			{
				...valueControl("model", "Model", layer.model, 0, 255, disabled, "", 1),
				display: layer.model === 0 ? "Flat" : `Model ${layer.model}`,
			},
			valueControl(
				"model-pan",
				"Pan",
				layer.modelPan,
				-360,
				360,
				disabled,
				"°",
				1,
			),
			valueControl(
				"model-tilt",
				"Tilt",
				layer.modelTilt,
				-360,
				360,
				disabled,
				"°",
				1,
			),
			{
				id: "model-roll",
				kind: "readout",
				label: "Roll",
				value: `${Number(layer.rotation.toFixed(2))}°`,
				description: "Rotation on the Frame tab is the model's roll.",
			},
		],
	};
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
};

export const VISUALIZER_FLAGS: Record<
	string,
	{ field: "mirror" | "filled" | "wireframe"; label: string }
> = {
	mirror: { field: "mirror", label: "Mirror" },
	filled: { field: "filled", label: "Filled" },
	wireframe: { field: "wireframe", label: "Wireframe" },
};
