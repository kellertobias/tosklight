import type { ReactNode } from "react";
import type { MediaSurfaceSection } from "./document/session";

/** The controls every Media editor builds its forms from. */

export function NumberInput({
	value,
	onChange,
	min,
	max,
	step = 0.01,
}: {
	value: number;
	onChange: (value: number) => void;
	min?: number;
	max?: number;
	step?: number;
}) {
	return (
		<input
			type="number"
			value={value}
			min={min}
			max={max}
			step={step}
			onChange={(event) => onChange(Number(event.target.value))}
		/>
	);
}

export function Field({
	label,
	children,
}: {
	label: string;
	children: ReactNode;
}) {
	return (
		// biome-ignore lint/a11y/noLabelWithoutControl: Every caller supplies its input control as the nested child.
		<label className="viz-media-field">
			<span>{label}</span>
			{children}
		</label>
	);
}

export function TransformEditor({
	value,
	onChange,
}: {
	value: MediaSurfaceSection["transform"];
	onChange: (value: MediaSurfaceSection["transform"]) => void;
}) {
	return (
		<fieldset className="viz-media-transform">
			<legend>3D transform</legend>
			{(["X", "Y", "Z"] as const).map((axis, index) => (
				<Field key={`position-${axis}`} label={`${axis} (m)`}>
					<NumberInput
						value={value.positionMetres[index]}
						onChange={(next) => {
							const positionMetres = [...value.positionMetres] as [
								number,
								number,
								number,
							];
							positionMetres[index] = next;
							onChange({ ...value, positionMetres });
						}}
					/>
				</Field>
			))}
			{(["X", "Y", "Z"] as const).map((axis, index) => (
				<Field key={`rotation-${axis}`} label={`${axis} rotation`}>
					<NumberInput
						step={1}
						value={value.rotationDegrees[index]}
						onChange={(next) => {
							const rotationDegrees = [...value.rotationDegrees] as [
								number,
								number,
								number,
							];
							rotationDegrees[index] = next;
							onChange({ ...value, rotationDegrees });
						}}
					/>
				</Field>
			))}
		</fieldset>
	);
}
