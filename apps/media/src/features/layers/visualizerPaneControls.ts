// A shown Visualizer's live configuration on the Media pane's Effects tab, and the layer edits its
// controls make. Kept apart from the pane itself, which only places these controls.

import type { MediaSecondaryControl } from "../../../../light-desktop/src/windows/media/MediaPaneSurface";
import type {
	UpdateLayer,
	VisualizerParametersView,
	VisualizerView,
} from "../../shared/api/generated/media-wire";
import {
	ON_BEAT,
	onBeatOptions,
	visualizerParameterLabel,
} from "../visualizers/parameterLabels";
import {
	VISUALIZER_FLAGS,
	VISUALIZER_GROUP,
	VISUALIZER_NUMBERS,
	valueControl,
} from "./layerDmxSections";

export function tintHex(red: number, green: number, blue: number) {
	return `#${[red, green, blue]
		.map((value) =>
			Math.round(value * 255)
				.toString(16)
				.padStart(2, "0"),
		)
		.join("")}`;
}

export function tintChange(value: string) {
	const raw = value.replace("#", "");
	return {
		tintRed: Number.parseInt(raw.slice(0, 2), 16) / 255,
		tintGreen: Number.parseInt(raw.slice(2, 4), 16) / 255,
		tintBlue: Number.parseInt(raw.slice(4, 6), 16) / 255,
	};
}

/** A shown Visualizer's live configuration, grouped under Visualizer on the Effects tab. */
export function visualizerControls(
	visualizer: VisualizerView,
	disabled: boolean,
): MediaSecondaryControl[] {
	const controls: MediaSecondaryControl[] = [
		{
			id: "visualizer-reset",
			kind: "choice",
			label: visualizer.name,
			value: "current",
			options: [
				{ value: "current", label: "Current" },
				{ value: "reset", label: "Reset parameters" },
			],
			disabled,
		},
	];
	// Audio gain is not one of the kind's parameters (those are its DMX channels), but every
	// visualizer hears audio, so each one offers it first.
	for (const parameter of ["audioGain", ...visualizer.uses]) {
		if (parameter === ON_BEAT) {
			controls.push({
				id: `visualizer-${parameter}`,
				kind: "choice" as const,
				label: visualizerParameterLabel(visualizer.typeId, parameter, "React to"),
				value: String(visualizer.parameters.onBeat),
				options: onBeatOptions(visualizer.typeId),
				disabled,
			});
			continue;
		}
		const number = VISUALIZER_NUMBERS[parameter];
		if (number) {
			const label = visualizerParameterLabel(
				visualizer.typeId,
				parameter,
				number.label,
			);
			const minimum =
				visualizer.typeId === 1 && parameter === "size"
					? 0.005
					: number.minimum;
			const maximum =
				visualizer.typeId === 1 && parameter === "size" ? 0.1 : number.maximum;
			controls.push(
				valueControl(
					`visualizer-${parameter}`,
					label,
					Number(visualizer.parameters[number.field]),
					minimum,
					maximum,
					disabled,
					"",
					number.step,
				),
			);
			continue;
		}
		const flag = VISUALIZER_FLAGS[parameter];
		if (flag) {
			controls.push({
				id: `visualizer-${parameter}`,
				kind: "choice" as const,
				label: flag.label,
				value: String(visualizer.parameters[flag.field]),
				options: [
					{ value: "true", label: "On" },
					{ value: "false", label: "Off" },
				],
				disabled,
			});
			continue;
		}
		if (parameter === "primary" || parameter === "secondary") {
			const prefix = parameter === "primary" ? "primary" : "secondary";
			controls.push({
				id: `visualizer-${parameter}`,
				kind: "color" as const,
				label: visualizerParameterLabel(
					visualizer.typeId,
					parameter,
					parameter === "primary" ? "Colour" : "Second colour",
				),
				value: tintHex(
					visualizer.parameters[`${prefix}Red`],
					visualizer.parameters[`${prefix}Green`],
					visualizer.parameters[`${prefix}Blue`],
				),
				disabled,
			});
		}
	}
	return controls.map((control) => ({ ...control, group: VISUALIZER_GROUP }));
}

/** A layer's own visualizer tuning edit. It never addresses an effect slot. */
export function visualizerChange(
	id: string,
	value: string | number,
	parameters: VisualizerParametersView,
): UpdateLayer {
	if (id === "visualizer-reset")
		return value === "reset" ? { resetVisualizerParameters: true } : {};
	return {
		visualizerParameters: changeVisualizerParameter(
			parameters,
			id.slice("visualizer-".length),
			value,
		),
	};
}

function changeVisualizerParameter(
	parameters: VisualizerParametersView,
	parameter: string,
	value: string | number,
): VisualizerParametersView {
	const number = VISUALIZER_NUMBERS[parameter];
	if (number) return { ...parameters, [number.field]: Number(value) };
	const flag = VISUALIZER_FLAGS[parameter];
	if (flag) return { ...parameters, [flag.field]: value === "true" };
	if (parameter === ON_BEAT) return { ...parameters, onBeat: value === "true" };
	if (parameter === "primary" || parameter === "secondary") {
		const channels = tintChange(String(value));
		return {
			...parameters,
			[`${parameter}Red`]: channels.tintRed,
			[`${parameter}Green`]: channels.tintGreen,
			[`${parameter}Blue`]: channels.tintBlue,
		};
	}
	return parameters;
}
