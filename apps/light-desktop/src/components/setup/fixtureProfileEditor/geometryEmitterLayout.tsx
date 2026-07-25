import type { GeometryEmitter } from "../../../api/types";
import { Button, NumberField, SelectField } from "../../common";
import { VectorFields } from "./geometryPreview";

type Layout = GeometryEmitter["layout"];

function defaultLayout(type: Layout["type"]): Layout {
	if (type === "point") return { type };
	if (type === "matrix")
		return {
			type,
			columns: 4,
			rows: 4,
			spacing: { x: 50, y: 50, z: 0 },
		};
	if (type === "ring") return { type, count: 12, radius_millimetres: 100 };
	if (type === "strip") return { type, count: 8, spacing_millimetres: 50 };
	return { type, positions: [] };
}

function MatrixLayoutFields({
	layout,
	onChange,
}: {
	layout: Extract<Layout, { type: "matrix" }>;
	onChange: (layout: Layout) => void;
}) {
	return (
		<>
			<NumberField
				label="Matrix columns"
				min={1}
				value={layout.columns}
				onChange={(event) =>
					onChange({ ...layout, columns: Number(event.target.value) })
				}
			/>
			<NumberField
				label="Matrix rows"
				min={1}
				value={layout.rows}
				onChange={(event) =>
					onChange({ ...layout, rows: Number(event.target.value) })
				}
			/>
			<VectorFields
				label="Matrix spacing (mm)"
				value={layout.spacing}
				onChange={(spacing) => onChange({ ...layout, spacing })}
			/>
		</>
	);
}

function CountLayoutFields({
	layout,
	onChange,
}: {
	layout: Extract<Layout, { type: "ring" | "strip" }>;
	onChange: (layout: Layout) => void;
}) {
	if (layout.type === "ring")
		return (
			<>
				<NumberField
					label="Ring source count"
					min={1}
					value={layout.count}
					onChange={(event) =>
						onChange({ ...layout, count: Number(event.target.value) })
					}
				/>
				<NumberField
					label="Ring radius (mm)"
					allowDecimal
					min={0}
					value={layout.radius_millimetres}
					onChange={(event) =>
						onChange({
							...layout,
							radius_millimetres: Number(event.target.value),
						})
					}
				/>
			</>
		);
	return (
		<>
			<NumberField
				label="Strip source count"
				min={1}
				value={layout.count}
				onChange={(event) =>
					onChange({ ...layout, count: Number(event.target.value) })
				}
			/>
			<NumberField
				label="Strip spacing (mm)"
				allowDecimal
				min={0}
				value={layout.spacing_millimetres}
				onChange={(event) =>
					onChange({
						...layout,
						spacing_millimetres: Number(event.target.value),
					})
				}
			/>
		</>
	);
}

function ExplicitPixelFields({
	layout,
	onChange,
}: {
	layout: Extract<Layout, { type: "explicit_pixels" }>;
	onChange: (layout: Layout) => void;
}) {
	return (
		<div className="geometry-explicit-pixels">
			{layout.positions.map((position, index) => (
				<article key={index}>
					<VectorFields
						label={`Pixel ${index + 1} position`}
						value={position}
						onChange={(next) =>
							onChange({
								...layout,
								positions: layout.positions.map((candidate, itemIndex) =>
									itemIndex === index ? next : candidate,
								),
							})
						}
					/>
					<Button
						onClick={() =>
							onChange({
								...layout,
								positions: layout.positions.filter(
									(_, itemIndex) => itemIndex !== index,
								),
							})
						}
					>
						Remove pixel
					</Button>
				</article>
			))}
			<Button
				onClick={() =>
					onChange({
						...layout,
						positions: [...layout.positions, { x: 0, y: 0, z: 0 }],
					})
				}
			>
				Add pixel position
			</Button>
		</div>
	);
}

export function GeometryEmitterLayout({
	layout,
	onChange,
}: {
	layout: Layout;
	onChange: (layout: Layout) => void;
}) {
	return (
		<>
			<SelectField
				label="Source layout"
				value={layout.type}
				options={[
					{ value: "point", label: "Point" },
					{ value: "matrix", label: "Matrix" },
					{ value: "ring", label: "Ring" },
					{ value: "strip", label: "Strip" },
					{ value: "explicit_pixels", label: "Explicit pixels" },
				]}
				onChange={(type) => onChange(defaultLayout(type))}
			/>
			{layout.type === "matrix" && (
				<MatrixLayoutFields layout={layout} onChange={onChange} />
			)}
			{(layout.type === "ring" || layout.type === "strip") && (
				<CountLayoutFields layout={layout} onChange={onChange} />
			)}
			{layout.type === "explicit_pixels" && (
				<ExplicitPixelFields layout={layout} onChange={onChange} />
			)}
		</>
	);
}
