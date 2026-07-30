export const ATTRIBUTE_ENCODER_GROUPS = [
	{ id: "intensity", label: "Intensity" },
	{ id: "color", label: "Color" },
	{ id: "position", label: "Position" },
	{ id: "beam", label: "Beam" },
	{ id: "shapers", label: "Shapers" },
	{ id: "focus", label: "Focus" },
	{ id: "control", label: "Control" },
	{ id: "media", label: "Media" },
] as const;

export type AttributeEncoderGroupId =
	(typeof ATTRIBUTE_ENCODER_GROUPS)[number]["id"];

export interface AttributeEncoderPlacement {
	id: string;
	label: string;
	encoder_group: AttributeEncoderGroupId;
	encoder_page: number;
	encoder_slot: number;
}

export interface AttributeEncoderPage<
	Descriptor extends AttributeEncoderPlacement = AttributeEncoderPlacement,
> {
	number: number;
	slots: Array<Descriptor | null>;
}

export interface AttributeEncoderGroup<
	Descriptor extends AttributeEncoderPlacement = AttributeEncoderPlacement,
> {
	id: AttributeEncoderGroupId;
	label: string;
	pages: Array<AttributeEncoderPage<Descriptor>>;
}

/**
 * Projects registry placement metadata into the eight stable encoder groups.
 *
 * The complete registry is validated before selection filtering. This makes a broken registry
 * deterministic even when the colliding attribute is not supported by the current selection.
 * Supported IDs absent from the registry are intentionally omitted: an unknown attribute needs an
 * explicit, valid placement before it can occupy an encoder.
 */
export function attributeEncoderGroups<
	Descriptor extends AttributeEncoderPlacement,
>(
	registry: readonly Descriptor[],
	supportedAttributes: ReadonlySet<string>,
): Array<AttributeEncoderGroup<Descriptor>> {
	const placements = validatePlacements(registry);
	return ATTRIBUTE_ENCODER_GROUPS.map(({ id, label }) => {
		const pages = new Map<number, Array<Descriptor | null>>();
		for (const descriptor of placements) {
			if (
				descriptor.encoder_group !== id ||
				!supportedAttributes.has(descriptor.id)
			) {
				continue;
			}
			const slots =
				pages.get(descriptor.encoder_page) ??
				Array.from<Descriptor | null>({ length: 6 }).fill(null);
			slots[descriptor.encoder_slot - 1] = descriptor;
			pages.set(descriptor.encoder_page, slots);
		}
		return {
			id,
			label,
			pages: [...pages]
				.sort(([left], [right]) => left - right)
				.map(([number, slots]) => ({ number, slots })),
		};
	});
}

function validatePlacements<Descriptor extends AttributeEncoderPlacement>(
	registry: readonly Descriptor[],
): Descriptor[] {
	const ordered = [...registry].sort((left, right) =>
		left.id.localeCompare(right.id),
	);
	const ids = new Set<string>();
	const occupied = new Map<string, string>();
	for (const descriptor of ordered) {
		if (ids.has(descriptor.id))
			throw new Error(`Duplicate attribute descriptor ID: ${descriptor.id}`);
		ids.add(descriptor.id);
		if (!isEncoderGroup(descriptor.encoder_group))
			throw new Error(
				`Invalid encoder group for ${descriptor.id}: ${descriptor.encoder_group}`,
			);
		if (
			!Number.isSafeInteger(descriptor.encoder_page) ||
			descriptor.encoder_page < 1
		) {
			throw new Error(
				`Invalid encoder page for ${descriptor.id}: ${descriptor.encoder_page}`,
			);
		}
		if (
			!Number.isSafeInteger(descriptor.encoder_slot) ||
			descriptor.encoder_slot < 1 ||
			descriptor.encoder_slot > 6
		) {
			throw new Error(
				`Invalid encoder slot for ${descriptor.id}: ${descriptor.encoder_slot}`,
			);
		}
		const key = `${descriptor.encoder_group}:${descriptor.encoder_page}:${descriptor.encoder_slot}`;
		const previous = occupied.get(key);
		if (previous)
			throw new Error(
				`Duplicate encoder placement ${key}: ${previous}, ${descriptor.id}`,
			);
		occupied.set(key, descriptor.id);
	}
	return ordered;
}

function isEncoderGroup(value: string): value is AttributeEncoderGroupId {
	return ATTRIBUTE_ENCODER_GROUPS.some((group) => group.id === value);
}
