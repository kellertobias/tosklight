import type {
	AttributeConfiguration,
	AttributeEncoderGroup,
	ConfiguredAttributeDescriptor,
} from "../../api/client/attributeConfiguration";

export type EncoderSlotTarget = {
	group: AttributeEncoderGroup;
	page: number;
	slot: number;
};

/** A group's encoders are one ordered list; page and slot follow from the desk width. */
function orderedGroupIds(
	descriptors: ConfiguredAttributeDescriptor[],
	group: AttributeEncoderGroup,
) {
	return descriptors
		.filter((descriptor) => descriptor.encoder_group === group)
		.map((descriptor) => descriptor.id);
}

function renumbered(
	configuration: AttributeConfiguration,
	group: AttributeEncoderGroup,
	orderedIds: string[],
	width: number,
) {
	const moved = new Set(orderedIds);
	return [
		...configuration.placements.filter(
			(placement) => !moved.has(placement.attribute),
		),
		...orderedIds.map((attribute, position) => ({
			attribute,
			encoder_group: group,
			encoder_page: Math.floor(position / width) + 1,
			encoder_slot: (position % width) + 1,
			push_turn_of:
				configuration.placements.find(
					(placement) => placement.attribute === attribute,
				)?.push_turn_of ?? null,
		})),
	];
}

/**
 * Moves one attribute onto an explicit encoder slot. Dropping past the end of a group
 * appends; a move between groups renumbers both so no slot is left with a hole.
 */
export function moveAttributeToSlot(
	configuration: AttributeConfiguration,
	descriptors: ConfiguredAttributeDescriptor[],
	attribute: string,
	target: EncoderSlotTarget,
	width: number,
): AttributeConfiguration {
	if (width < 1) return configuration;
	const source = descriptors.find((descriptor) => descriptor.id === attribute);
	const targetIndex = Math.max(0, (target.page - 1) * width + target.slot - 1);
	const targetIds = orderedGroupIds(descriptors, target.group).filter(
		(id) => id !== attribute,
	);
	targetIds.splice(Math.min(targetIndex, targetIds.length), 0, attribute);
	let placements = renumbered(configuration, target.group, targetIds, width);
	if (source && source.encoder_group !== target.group) {
		const sourceIds = orderedGroupIds(descriptors, source.encoder_group).filter(
			(id) => id !== attribute,
		);
		placements = renumbered(
			{ ...configuration, placements },
			source.encoder_group,
			sourceIds,
			width,
		);
	}
	return { ...configuration, placements };
}

/** Every attribute that currently has no encoder slot at all. */
export function unplacedDescriptors(
	descriptors: ConfiguredAttributeDescriptor[],
	configuration: AttributeConfiguration,
) {
	const placed = new Set(
		configuration.placements.map((placement) => placement.attribute),
	);
	return descriptors.filter(
		(descriptor) => !descriptor.retired && !placed.has(descriptor.id),
	);
}
