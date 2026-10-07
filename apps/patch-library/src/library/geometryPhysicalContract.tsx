import {
	Button,
	FormLayout,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui";
import type {
	GeometryGraph,
	GeometryPhysicalContract,
	Vector3Value,
} from "../fixtureProfile";
import { VectorFields } from "./geometryPreview";
export function GeometryPhysicalContractEditor({
	geometry,
	onChange,
}: {
	geometry: GeometryGraph;
	onChange(geometry: GeometryGraph): void;
}) {
	const value = geometry.physical_contract;
	const set = (physical_contract: GeometryPhysicalContract | null) =>
		onChange({ ...geometry, physical_contract });
	if (!value)
		return (
			<section>
				<Button
					disabled={!geometry.nodes.length}
					onClick={() =>
						set({
							version: 1,
							provenance: { quality: "unknown", revision: 0 },
							bracket: { kind: "unknown" },
						})
					}
				>
					Declare physical geometry
				</Button>
				<p className="field-hint">
					Existing artwork is unverified. Declare its coordinate contract before
					binding physical Position.
				</p>
			</section>
		);
	const bracket = value.bracket;
	const vector = (key: "pivot" | "axis", v: Vector3Value) => {
		if (bracket.kind === "hinge")
			set({ ...value, bracket: { ...bracket, [key]: v } });
	};
	return (
		<fieldset>
			<legend>Physical geometry</legend>
			<p className="field-hint">
				Local millimetres, right-handed Y up; a neutral lens points along −Y.
				Physical paths use identity scale. These saved declarations do not yet
				replace live Stage calculations.
			</p>
			<FormLayout columns={3} minColumnWidth={160}>
				<SelectField
					label="Geometry quality"
					ariaLabel="Geometry quality"
					value={value.provenance.quality}
					options={[
						{ value: "unknown", label: "Unknown" },
						{ value: "estimated", label: "Estimated" },
						{ value: "manufacturer", label: "Manufacturer" },
						{ value: "measured", label: "Measured" },
					]}
					onChange={(quality) =>
						set({
							...value,
							provenance: {
								...value.provenance,
								quality:
									quality as GeometryPhysicalContract["provenance"]["quality"],
							},
						})
					}
				/>
				<TextField
					label="Geometry evidence"
					value={value.provenance.source ?? ""}
					onChange={(e) =>
						set({
							...value,
							provenance: {
								...value.provenance,
								source: e.target.value || null,
							},
						})
					}
				/>
				<NumberField
					label="Geometry evidence revision"
					min={0}
					max={0xffff_ffff}
					value={value.provenance.revision}
					onChange={(e) =>
						set({
							...value,
							provenance: {
								...value.provenance,
								revision: Number(e.target.value),
							},
						})
					}
				/>
				<SelectField
					label="Bracket geometry"
					ariaLabel="Bracket geometry"
					value={bracket.kind}
					options={[
						{ value: "unknown", label: "Unknown" },
						{ value: "fixed", label: "Fixed" },
						{ value: "hinge", label: "Hinge" },
					]}
					onChange={(kind) =>
						set({
							...value,
							bracket:
								kind === "hinge"
									? {
											kind: "hinge",
											node_id: geometry.nodes[0]?.id ?? "",
											pivot: { x: 0, y: 0, z: 0 },
											axis: { x: 1, y: 0, z: 0 },
										}
									: { kind: kind as "unknown" | "fixed" },
						})
					}
				/>
			</FormLayout>
			{bracket.kind === "hinge" && (
				<>
					<SelectField
						label="Bracket body"
						ariaLabel="Bracket body"
						value={bracket.node_id}
						options={geometry.nodes.map((n) => ({
							value: n.id,
							label: n.name,
						}))}
						onChange={(node_id) =>
							set({ ...value, bracket: { ...bracket, node_id } })
						}
					/>
					<VectorFields
						label="Bracket pivot in parent frame (mm)"
						value={bracket.pivot}
						onChange={(v) => vector("pivot", v)}
					/>
					<VectorFields
						label="Bracket axis in parent frame"
						value={bracket.axis}
						onChange={(v) => vector("axis", v)}
					/>
				</>
			)}
			<Button onClick={() => set(null)}>Remove physical declaration</Button>
		</fieldset>
	);
}
