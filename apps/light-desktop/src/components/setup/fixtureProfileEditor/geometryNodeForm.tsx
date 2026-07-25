import type { GeometryNode } from "../../../api/types";
import {
	Button,
	CheckboxField,
	NumberField,
	SelectField,
	TextField,
} from "../../common";
import { VectorFields } from "./geometryPreview";

type Motion = NonNullable<GeometryNode["motion"]>;

function GeometryMotionFields({
	motion,
	onChange,
}: {
	motion: Motion;
	onChange: (motion: Motion) => void;
}) {
	return (
		<div className="geometry-motion">
			<TextField
				label="Motion attribute"
				list="fixture-attribute-registry"
				value={motion.attribute}
				onChange={(event) =>
					onChange({ ...motion, attribute: event.target.value })
				}
			/>
			<SelectField
				label="Motion kind"
				value={motion.kind}
				options={[
					{ value: "rotation", label: "Rotation" },
					{ value: "translation", label: "Translation" },
				]}
				onChange={(kind) => onChange({ ...motion, kind })}
			/>
			<VectorFields
				label="Motion axis"
				value={motion.axis}
				onChange={(axis) => onChange({ ...motion, axis })}
			/>
			<NumberField
				label="Physical minimum"
				allowDecimal
				value={motion.physical_min}
				onChange={(event) =>
					onChange({ ...motion, physical_min: Number(event.target.value) })
				}
			/>
			<NumberField
				label="Physical maximum"
				allowDecimal
				value={motion.physical_max}
				onChange={(event) =>
					onChange({ ...motion, physical_max: Number(event.target.value) })
				}
			/>
		</div>
	);
}

export function GeometryNodeForm({
	node,
	nodes,
	onChange,
	onRemove,
}: {
	node: GeometryNode;
	nodes: GeometryNode[];
	onChange: (node: GeometryNode) => void;
	onRemove: () => void;
}) {
	return (
		<div>
			<h3>Part properties</h3>
			<TextField
				label="Part name"
				value={node.name}
				onChange={(event) => onChange({ ...node, name: event.target.value })}
			/>
			<SelectField
				label="Parent part"
				value={node.parent_id ?? ""}
				options={[
					{ value: "", label: "Root" },
					...nodes
						.filter((candidate) => candidate.id !== node.id)
						.map((candidate) => ({
							value: candidate.id,
							label: candidate.name,
						})),
				]}
				onChange={(parent_id) =>
					onChange({ ...node, parent_id: parent_id || null })
				}
			/>
			<TextField
				label="GLB node binding"
				value={node.glb_node ?? ""}
				onChange={(event) =>
					onChange({ ...node, glb_node: event.target.value || null })
				}
			/>
			<VectorFields
				label="Translation"
				value={node.transform.translation}
				onChange={(translation) =>
					onChange({ ...node, transform: { ...node.transform, translation } })
				}
			/>
			<VectorFields
				label="Base rotation °"
				value={node.transform.rotation_degrees}
				onChange={(rotation_degrees) =>
					onChange({
						...node,
						transform: { ...node.transform, rotation_degrees },
					})
				}
			/>
			<VectorFields
				label="Scale"
				value={node.transform.scale}
				onChange={(scale) =>
					onChange({ ...node, transform: { ...node.transform, scale } })
				}
			/>
			<VectorFields
				label="Pivot"
				value={node.pivot}
				onChange={(pivot) => onChange({ ...node, pivot })}
			/>
			<CheckboxField
				label="Attribute-driven motion"
				checked={Boolean(node.motion)}
				onChange={(event) =>
					onChange({
						...node,
						motion: event.target.checked
							? {
									attribute: "pan",
									kind: "rotation",
									axis: { x: 0, y: 1, z: 0 },
									physical_min: -270,
									physical_max: 270,
								}
							: null,
					})
				}
			/>
			{node.motion && (
				<GeometryMotionFields
					motion={node.motion}
					onChange={(motion) => onChange({ ...node, motion })}
				/>
			)}
			<Button variant="danger" disabled={nodes.length === 1} onClick={onRemove}>
				Remove part
			</Button>
		</div>
	);
}
