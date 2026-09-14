import { Button } from "@tosklight/ui";
import { useState } from "react";
import type { AttributeDescriptor, FixtureMode, GeometryGraph } from "../wire";
import { AttributePickerModal } from "./attributePicker";

/**
 * Which attribute moves each of the fixture's moving parts in this mode.
 *
 * The part, its axis, its range and its speed are the lantern's, described once under Geometry.
 * What drives it is the personality's: the same yoke is Pan in one mode and fixed in another, so a
 * part this mode does not bind rests at its centre.
 */
export function MotionBindings({
	mode,
	geometry,
	attributeRegistry,
	onChange,
}: {
	mode: FixtureMode;
	geometry: GeometryGraph;
	attributeRegistry: AttributeDescriptor[];
	onChange: (mode: FixtureMode) => void;
}) {
	const [editing, setEditing] = useState<string | null>(null);
	const moving = geometry.nodes.filter((node) => node.motion);
	if (!moving.length) return null;
	const bound = (nodeId: string) =>
		mode.motion_attributes?.find((binding) => binding.node_id === nodeId)
			?.attribute ?? "";
	const setBinding = (nodeId: string, attribute: string) => {
		const kept = (mode.motion_attributes ?? []).filter(
			(binding) => binding.node_id !== nodeId,
		);
		onChange({
			...mode,
			motion_attributes: attribute
				? [...kept, { node_id: nodeId, attribute }]
				: kept,
		});
	};
	const label = (attribute: string) =>
		attributeRegistry.find((descriptor) => descriptor.id === attribute)
			?.label ?? attribute;
	const editedNode = moving.find((node) => node.id === editing);
	return (
		<section className="fixture-motion-bindings">
			<h3>Moving parts</h3>
			<p className="field-hint">
				A moving part no attribute drives rests at its centre in this mode.
			</p>
			{moving.map((node) => {
				const attribute = bound(node.id);
				return (
					<div key={node.id} className="fixture-motion-binding">
						<span>{node.name || "Part"}</span>
						<Button
							className="fixture-cell-value"
							aria-label={`Attribute moving ${node.name || "part"}: ${attribute ? label(attribute) : "Not driven"}`}
							onClick={() => setEditing(node.id)}
						>
							{attribute ? label(attribute) : "Not driven"}
						</Button>
						<Button
							disabled={!attribute}
							aria-label={`Stop driving ${node.name || "part"}`}
							onClick={() => setBinding(node.id, "")}
						>
							Not driven
						</Button>
					</div>
				);
			})}
			{editedNode && (
				<AttributePickerModal
					title={`Attribute · ${editedNode.name || "part"}`}
					value={bound(editedNode.id)}
					registry={attributeRegistry}
					includeStatic={false}
					onSelect={(attribute) => {
						setBinding(editedNode.id, attribute);
						setEditing(null);
					}}
					onClose={() => setEditing(null)}
				/>
			)}
		</section>
	);
}
