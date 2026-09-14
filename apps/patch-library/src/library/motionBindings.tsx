import { Button } from "@tosklight/ui";
import { useState } from "react";
import type { AttributeDescriptor, FixtureMode, GeometryGraph } from "../wire";
import { derivePrimarySlots } from "../sheet/fixtureProfileModel";
import { channelLabel } from "./channelLabels";
import { OptionPickerModal, type PickerOption } from "./optionPicker";

const NOT_DRIVEN = "";

/**
 * What can move a part in this mode: the attributes its channels actually carry.
 *
 * A part can only follow a value the mode sends, so the choice is from the mode's own channels —
 * each attribute once, with the slots and heads that carry it — rather than from every attribute
 * the desk knows.
 */
function channelOptions(
	mode: FixtureMode,
	registry: readonly AttributeDescriptor[],
): PickerOption[] {
	const slots = derivePrimarySlots(mode).slots;
	const byAttribute = new Map<string, { label: string; where: string[] }>();
	for (const channel of mode.channels) {
		if (channel.behavior === "static" || !channel.attribute) continue;
		const head = mode.heads.find((candidate) => candidate.id === channel.head_id);
		const entry = byAttribute.get(channel.attribute) ?? {
			label: channelLabel(channel, registry),
			where: [],
		};
		const slot = slots.get(channel.id);
		const place = mode.splits.length > 1 ? `split ${channel.split} slot ${slot}` : `slot ${slot}`;
		entry.where.push(head && mode.heads.length > 1 ? `${place} (${head.name})` : place);
		byAttribute.set(channel.attribute, entry);
	}
	return [
		{ value: NOT_DRIVEN, label: "Not driven", detail: "Rests where it is drawn" },
		...[...byAttribute.entries()].map(([attribute, entry]) => ({
			value: attribute,
			label: entry.label,
			detail: entry.where.join(", "),
		})),
	];
}

/**
 * Which of this mode's channels moves each of the fixture's moving parts.
 *
 * The part, its axis, its range and its speed are the lantern's, described once under Geometry.
 * What drives it is the personality's: the same yoke follows Pan in one mode and stays still in
 * another, so a part this mode does not bind rests where it is drawn.
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
	const options = channelOptions(mode, attributeRegistry);
	const bound = (nodeId: string) =>
		mode.motion_attributes?.find((binding) => binding.node_id === nodeId)
			?.attribute ?? NOT_DRIVEN;
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
	// A binding to an attribute no channel carries any more is shown as such, so it can be fixed.
	const describe = (attribute: string) =>
		attribute === NOT_DRIVEN
			? "Not driven"
			: (options.find((option) => option.value === attribute)?.label ??
				`${attribute} (no channel)`);
	const editedNode = moving.find((node) => node.id === editing);
	return (
		<section className="fixture-motion-bindings">
			<h3>Moving parts</h3>
			<p className="field-hint">
				Choose the channel that moves each part. Only attributes this mode's channels carry
				are offered.
			</p>
			{moving.map((node) => {
				const attribute = bound(node.id);
				const name = node.name || "part";
				return (
					<div key={node.id} className="fixture-motion-binding">
						<span>{node.name || "Part"}</span>
						<Button
							className="fixture-cell-value"
							aria-label={`Channel moving ${name}: ${describe(attribute)}`}
							onClick={() => setEditing(node.id)}
						>
							{describe(attribute)}
						</Button>
					</div>
				);
			})}
			{editedNode && (
				<OptionPickerModal
					title={`Channel · ${editedNode.name || "part"}`}
					value={bound(editedNode.id)}
					options={options}
					onSelect={(attribute) => setBinding(editedNode.id, attribute)}
					onClose={() => setEditing(null)}
				/>
			)}
		</section>
	);
}
