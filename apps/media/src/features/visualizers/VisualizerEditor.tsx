// Tuning one visualizer.
//
// Only the controls the kind actually reads are shown — the server publishes which those are, so
// this never has to know what an Equalizer does with `smoothing` and a Starfield does not.

import {
	CheckboxField,
	ColorPickerField,
	SwitchField,
} from "@tosklight/ui/controls";
import { NumberField } from "@tosklight/ui/forms";
import { type ChangeEvent, useState } from "react";
import { requestId } from "../../shared/api/editing";
import type {
	UpdateVisualizer,
	VisualizerParametersView,
	VisualizerView,
} from "../../shared/api/generated/media-wire";
import { GridLandscapeSceneryFields } from "./GridLandscapeSceneryFields";

export interface VisualizerEditorProps {
	visualizer: VisualizerView;
	busy: boolean;
	onChange: (edit: UpdateVisualizer) => void;
}

/// Which fields each published control name writes, and how it is labelled.
const NUMBERS: Record<
	string,
	{ label: string; field: keyof VisualizerParametersView; step: number }
> = {
	count: { label: "Count", field: "count", step: 1 },
	size: { label: "Size", field: "size", step: 0.01 },
	speed: { label: "Speed", field: "speed", step: 0.1 },
	amount: { label: "Amount", field: "amount", step: 0.05 },
	radius: { label: "Radius", field: "radius", step: 0.05 },
	thickness: { label: "Thickness", field: "thickness", step: 0.005 },
	reactivity: { label: "Reactivity", field: "reactivity", step: 0.1 },
	decay: { label: "Decay", field: "decay", step: 0.05 },
	zoom: { label: "Zoom", field: "zoom", step: 0.1 },
	iterations: { label: "Iterations", field: "iterations", step: 1 },
	threshold: { label: "Threshold", field: "threshold", step: 0.05 },
	smoothing: { label: "Smoothing", field: "smoothing", step: 0.05 },
	gravity: { label: "Gravity", field: "gravity", step: 0.1 },
	lifetime: { label: "Lifetime", field: "lifetime", step: 0.1 },
	curvature: { label: "Curvature", field: "curvature", step: 0.05 },
	mode: { label: "Variant", field: "mode", step: 1 },
};

const FLAGS: Record<
	string,
	{ label: string; field: keyof VisualizerParametersView }
> = {
	mirror: { label: "Mirror", field: "mirror" },
	filled: { label: "Filled", field: "filled" },
	wireframe: { label: "Wireframe", field: "wireframe" },
};

export function VisualizerEditor({
	visualizer,
	busy,
	onChange,
}: VisualizerEditorProps) {
	const [parameters, setParameters] = useState(visualizer.parameters);

	const publish = (nextParameters: VisualizerParametersView) =>
		onChange({
			requestId: requestId(),
			parameters: nextParameters,
		});
	const set = (
		field: keyof VisualizerParametersView,
		value: number | boolean,
	) =>
		setParameters((current) => {
			const next = { ...current, [field]: value };
			publish(next);
			return next;
		});
	const setColour = (prefix: "primary" | "secondary", hex: string) =>
		setParameters((current) => {
			const [red, green, blue] = fromHex(hex);
			const next = {
				...current,
				[`${prefix}Red`]: red,
				[`${prefix}Green`]: green,
				[`${prefix}Blue`]: blue,
			};
			publish(next);
			return next;
		});

	return (
		<div className="media-visualizer-editor" aria-busy={busy}>
			{visualizer.uses.map((control) => {
				if (
					visualizer.kind === "Grid Landscape" &&
					(control === "mode" || control === "iterations")
				) {
					if (control === "iterations") return null;
					return (
						<GridLandscapeSceneryFields
							key="grid-landscape-scenery"
							left={parameters.mode}
							right={parameters.iterations}
							onLeftChange={(value) => set("mode", value)}
							onRightChange={(value) => set("iterations", value)}
						/>
					);
				}
				const number = NUMBERS[control];
				if (number) {
					const label =
						visualizer.typeId === 0 && control === "amount"
							? "Bloom"
							: number.label;
					return (
						<NumberField
							key={control}
							label={label}
							step={number.step}
							value={String(parameters[number.field])}
							onChange={(event) =>
								set(number.field, Number(event.target.value))
							}
						/>
					);
				}
				const flag = FLAGS[control];
				if (flag) {
					const props = {
						key: control,
						label: flag.label,
						checked: Boolean(parameters[flag.field]),
						onChange: (event: ChangeEvent<HTMLInputElement>) =>
							set(flag.field, event.target.checked),
					};
					return control === "wireframe" ? (
						<SwitchField {...props} />
					) : (
						<CheckboxField {...props} />
					);
				}
				if (control === "primary" || control === "secondary") {
					return (
						<Colour
							key={control}
							label={control === "primary" ? "Colour" : "Second colour"}
							parameters={parameters}
							prefix={control}
							onChange={setColour}
						/>
					);
				}
				return null;
			})}
			<small className="media-live-save-state">
				{busy ? "Saving changes…" : "Changes update live"}
			</small>
		</div>
	);
}

function Colour({
	label,
	parameters,
	prefix,
	onChange,
}: {
	label: string;
	parameters: VisualizerParametersView;
	prefix: "primary" | "secondary";
	onChange: (prefix: "primary" | "secondary", value: string) => void;
}) {
	const red = `${prefix}Red` as keyof VisualizerParametersView;
	const green = `${prefix}Green` as keyof VisualizerParametersView;
	const blue = `${prefix}Blue` as keyof VisualizerParametersView;
	const value = toHex(
		Number(parameters[red]),
		Number(parameters[green]),
		Number(parameters[blue]),
	);

	return (
		<ColorPickerField
			label={label}
			value={value}
			onChange={(next) => onChange(prefix, next)}
		/>
	);
}

/** The server keeps colour as three 0–1 channels; a colour input speaks hex. */
function toHex(red: number, green: number, blue: number): string {
	const channel = (value: number) =>
		Math.round(Math.min(Math.max(value, 0), 1) * 255)
			.toString(16)
			.padStart(2, "0");
	return `#${channel(red)}${channel(green)}${channel(blue)}`;
}

function fromHex(hex: string): [number, number, number] {
	const value = Number.parseInt(hex.replace("#", ""), 16);
	return [
		((value >> 16) & 0xff) / 255,
		((value >> 8) & 0xff) / 255,
		(value & 0xff) / 255,
	];
}
