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
	/** Controls with the same key are packed together on width-derived pages. */
	compound_group?: string | null;
	push_turn_of?: string | null;
	push_turn_attribute?: string | null;
	push_turn_label?: string | null;
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

export function projectPushTurnPlacements<
	Descriptor extends AttributeEncoderPlacement,
>(registry: readonly Descriptor[]) {
	const companions = new Map(
		registry.flatMap((descriptor) =>
			descriptor.push_turn_of
				? [[descriptor.push_turn_of, descriptor] as const]
				: [],
		),
	);
	return registry.flatMap((descriptor) => {
		if (descriptor.push_turn_of) return [];
		const companion = companions.get(descriptor.id);
		return [
			{
				...descriptor,
				push_turn_attribute: companion?.id ?? null,
				push_turn_label: companion?.label ?? null,
			},
		];
	});
}

export function resolveAnchoredEncoderPage<
	Descriptor extends AttributeEncoderPlacement,
>(
	group: AttributeEncoderGroup<Descriptor> | undefined,
	requestedPage: number,
	anchor: string | null,
) {
	if (!group?.pages.length) return 1;
	if (anchor) {
		const anchored = group.pages.findIndex((page) =>
			page.slots.some((descriptor) => descriptor?.id === anchor),
		);
		if (anchored >= 0) return anchored + 1;
	}
	return Math.max(1, Math.min(requestedPage, group.pages.length));
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
	slotCount: 4 | 5 | 6 = 6,
): Array<AttributeEncoderGroup<Descriptor>> {
	const placements = validatePlacements(registry);
	if (slotCount !== 6)
		return repackedEncoderGroups(placements, supportedAttributes, slotCount);
	return ATTRIBUTE_ENCODER_GROUPS.map(({ id, label }) => {
		const pages = new Map<number, Array<Descriptor | null>>();
		for (const descriptor of placements) {
			if (
				descriptor.encoder_group !== id ||
				!descriptorIsSupported(descriptor, supportedAttributes)
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

function repackedEncoderGroups<Descriptor extends AttributeEncoderPlacement>(
	placements: readonly Descriptor[],
	supportedAttributes: ReadonlySet<string>,
	slotCount: 4 | 5,
): Array<AttributeEncoderGroup<Descriptor>> {
	return ATTRIBUTE_ENCODER_GROUPS.map(({ id, label }) => {
		const ordered = placements
			.filter((descriptor) => descriptor.encoder_group === id)
			.sort(compareSemanticPlacement);
		const units = compoundUnits(ordered);
		const packed: Array<Array<Descriptor | null>> = [];
		let slots: Array<Descriptor | null> = [];
		for (const unit of units) {
			if (unit.length > slotCount)
				throw new Error(
					`Compound encoder group ${unit[0]?.compound_group ?? "unknown"} needs ${unit.length} slots on a ${slotCount}-encoder surface`,
				);
			if (slots.length && slots.length + unit.length > slotCount) {
				packed.push(padSlots(slots, slotCount));
				slots = [];
			}
			slots.push(...unit);
		}
		if (slots.length) packed.push(padSlots(slots, slotCount));
		const pages = packed
			.map((pageSlots, index) => ({
				number: index + 1,
				slots: pageSlots.map((descriptor) =>
					descriptor && descriptorIsSupported(descriptor, supportedAttributes)
						? descriptor
						: null,
				),
			}))
			.filter((page) => page.slots.some(Boolean));
		return { id, label, pages };
	});
}

function descriptorIsSupported(
	descriptor: AttributeEncoderPlacement,
	supportedAttributes: ReadonlySet<string>,
) {
	return (
		supportedAttributes.has(descriptor.id) ||
		Boolean(
			descriptor.push_turn_attribute &&
				supportedAttributes.has(descriptor.push_turn_attribute),
		)
	);
}

function compoundUnits<Descriptor extends AttributeEncoderPlacement>(
	ordered: readonly Descriptor[],
): Descriptor[][] {
	const units: Descriptor[][] = [];
	for (const descriptor of ordered) {
		const previous = units.at(-1);
		if (
			descriptor.compound_group &&
			previous?.[0]?.compound_group === descriptor.compound_group
		) {
			previous.push(descriptor);
		} else {
			units.push([descriptor]);
		}
	}
	return units;
}

function compareSemanticPlacement(
	left: AttributeEncoderPlacement,
	right: AttributeEncoderPlacement,
) {
	return (
		left.encoder_page - right.encoder_page ||
		left.encoder_slot - right.encoder_slot ||
		left.id.localeCompare(right.id)
	);
}

function padSlots<Descriptor>(slots: Descriptor[], slotCount: number) {
	return [
		...slots,
		...Array.from<null>({ length: slotCount - slots.length }).fill(null),
	];
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
