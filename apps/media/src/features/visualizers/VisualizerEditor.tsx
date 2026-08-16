// Tuning one visualizer.
//
// Only the controls the kind actually reads are shown — the server publishes which those are, so
// this never has to know what an Equalizer does with `smoothing` and a Starfield does not.

import { Button, NumberField, TextField } from "@tosklight/ui/forms";
import { useState } from "react";
import type {
	UpdateVisualizer,
	VisualizerParametersView,
	VisualizerView,
} from "../../shared/api/generated/media-wire";
import { GridLandscapeSceneryFields } from "./GridLandscapeSceneryFields";

export interface VisualizerEditorProps {
	visualizer: VisualizerView;
	busy: boolean;
	onSave: (edit: UpdateVisualizer) => void;
	onCancel: () => void;
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
	onSave,
	onCancel,
}: VisualizerEditorProps) {
	const [name, setName] = useState(visualizer.name);
	const [parameters, setParameters] = useState(visualizer.parameters);

	const set = (
		field: keyof VisualizerParametersView,
		value: number | boolean,
	) => setParameters((current) => ({ ...current, [field]: value }));

	return (
		<form
			className="media-visualizer-editor"
			onSubmit={(event) => {
				event.preventDefault();
				// The request id is what makes a retry safe: the same edit sent twice is one edit.
				onSave({ requestId: crypto.randomUUID(), name, parameters });
			}}
		>
			<TextField
				label="Name"
				value={name}
				onChange={(event) => setName(event.target.value)}
			/>

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
					return (
						<label key={control} className="media-visualizer-flag">
							<input
								type="checkbox"
								checked={Boolean(parameters[flag.field])}
								onChange={(event) => set(flag.field, event.target.checked)}
							/>
							{flag.label}
						</label>
					);
				}
				if (control === "primary" || control === "secondary") {
					return (
						<Colour
							key={control}
							label={control === "primary" ? "Colour" : "Second colour"}
							parameters={parameters}
							prefix={control}
							onChange={set}
						/>
					);
				}
				return null;
			})}

			<div className="media-visualizer-actions">
				<Button type="submit" variant="primary" loading={busy}>
					Save
				</Button>
				<Button onClick={onCancel}>Cancel</Button>
			</div>
		</form>
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
	onChange: (field: keyof VisualizerParametersView, value: number) => void;
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
		<label className="media-visualizer-colour">
			{label}
			<input
				type="color"
				value={value}
				onChange={(event) => {
					const [r, g, b] = fromHex(event.target.value);
					onChange(red, r);
					onChange(green, g);
					onChange(blue, b);
				}}
			/>
		</label>
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
