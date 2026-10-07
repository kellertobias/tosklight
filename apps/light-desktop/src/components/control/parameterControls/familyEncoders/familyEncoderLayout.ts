import type {
	FamilyEncoderComponentSlot,
	FamilyEncoderFamily,
	FamilyEncoderGroup,
	FamilyEncoderPagesSnapshot,
	FamilyEncoderSlot,
} from "../../../../api/familyEncoderModels";
import type {
	AttributeEncoderGroup,
	AttributeEncoderPlacement,
} from "../attributeEncoderPages";
import type { ParameterFamily } from "../model";

/**
 * Semantic family encoder layout (TL-549/550/551 UI foundation).
 *
 * Given the server's published family pages, composes the encoder pages of one family tab:
 * the semantic pages first, then the family's remaining registry pages with every replaced
 * attribute removed. A 4-encoder layout keeps the published pages; a wider layout (6 encoders)
 * fills pages sequentially with the assigned semantic slots. Reserved pages (Direct Color 3/4,
 * TL-554) are filled from the caller's `nativePages` (the reference head's native controls).
 *
 * Returns `null` — the legacy normalized pages stay in force, unchanged — whenever the backend
 * does not report the semantic contract, the family has no semantic pages, or no selected
 * fixture carries the family.
 */

export const SEMANTIC_PARAMETER_FAMILIES: Readonly<
	Partial<Record<ParameterFamily, FamilyEncoderFamily>>
> = { Position: "position", Color: "color", Focus: "focus" };

export type FamilyLayoutSlot =
	| {
			kind: "component";
			family: FamilyEncoderFamily;
			slot: FamilyEncoderComponentSlot;
	  }
	| { kind: "attribute"; attribute: string; pushTurnAttribute: string | null }
	| null;

export interface FamilyLayout {
	family: FamilyEncoderFamily;
	group: FamilyEncoderGroup;
	pages: FamilyLayoutSlot[][];
}

export interface FamilyLayoutInput {
	snapshot: FamilyEncoderPagesSnapshot | null;
	family: ParameterFamily;
	registryGroup: AttributeEncoderGroup<AttributeEncoderPlacement> | undefined;
	visibleEncoderCount: number;
	/** Registry attributes the selection supports (the legacy path's own filter). */
	supportsAttribute(attribute: string): boolean;
	/**
	 * TL-554: Direct Color pages of the reference head, four slots per page. They follow the
	 * semantic Color pages: on a 4-encoder layout they are pages 3 and 4 (an absent page 2 stays
	 * an empty page so the numbering never moves with Easy/Advanced); wider layouts fill pages
	 * sequentially after the semantic slots.
	 */
	nativePages?: FamilyLayoutSlot[][];
}

function replaces(group: FamilyEncoderGroup, attribute: string) {
	return (
		group.replaces_attributes.includes(attribute) ||
		group.replaces_attribute_prefixes.some((prefix) =>
			attribute.startsWith(prefix),
		)
	);
}

function semanticSlot(
	family: FamilyEncoderFamily,
	slot: FamilyEncoderSlot | null,
	supportsAttribute: (attribute: string) => boolean,
): FamilyLayoutSlot {
	if (!slot) return null;
	if (slot.kind === "attribute")
		return supportsAttribute(slot.attribute)
			? { kind: "attribute", attribute: slot.attribute, pushTurnAttribute: null }
			: null;
	const { kind: _kind, ...component } = slot;
	return component.fixture_ids.length
		? { kind: "component", family, slot: component }
		: null;
}

function pad(slots: FamilyLayoutSlot[], count: number) {
	const padded = slots.slice(0, count);
	while (padded.length < count) padded.push(null);
	return padded;
}

function chunk(slots: FamilyLayoutSlot[], count: number) {
	const pages: FamilyLayoutSlot[][] = [];
	for (let index = 0; index < slots.length; index += count)
		pages.push(pad(slots.slice(index, index + count), count));
	return pages;
}

const assigned = (page: FamilyLayoutSlot[]) => page.some(Boolean);

export function familyEncoderLayout(input: FamilyLayoutInput): FamilyLayout | null {
	const familyId = SEMANTIC_PARAMETER_FAMILIES[input.family];
	if (!input.snapshot?.semantic || !familyId) return null;
	const group = input.snapshot.families.find(
		(candidate) => candidate.family === familyId,
	);
	if (!group?.pages.length || !group.fixture_ids.length) return null;
	const count = Math.max(1, input.visibleEncoderCount);
	const semanticPages = group.pages.map((page) =>
		page.slots.map((slot) =>
			semanticSlot(familyId, slot, input.supportsAttribute),
		),
	);
	const semantic =
		count > 4
			? chunk(semanticPages.flat().filter(Boolean), count)
			: semanticPages.map((page) => pad(page, count)).filter(assigned);
	const remainder = (input.registryGroup?.pages ?? [])
		.map((page) =>
			pad(
				page.slots.map((placement): FamilyLayoutSlot =>
					placement && !replaces(group, placement.id)
						? {
								kind: "attribute",
								attribute: placement.id,
								pushTurnAttribute: placement.push_turn_attribute ?? null,
							}
						: null,
				),
				count,
			),
		)
		.filter(assigned);
	const pages = [
		...withNativePages(semantic, input.nativePages ?? [], count),
		...remainder,
	];
	return pages.length ? { family: familyId, group, pages } : null;
}

/** Native pages after the semantic ones; see `FamilyLayoutInput.nativePages`. */
function withNativePages(
	semantic: FamilyLayoutSlot[][],
	native: FamilyLayoutSlot[][],
	count: number,
) {
	const slots = native.flat().filter(Boolean);
	if (!slots.length) return semantic;
	if (count > 4) return [...semantic, ...chunk(slots, count)];
	const pages = [...semantic];
	while (pages.length < NATIVE_FIRST_PAGE - 1) pages.push(pad([], count));
	return [...pages, ...native.map((page) => pad(page, count)).filter(assigned)];
}

/** Direct Color starts on page 3 of a 4-encoder layout (TL-554). */
export const NATIVE_FIRST_PAGE = 3;

/** The legacy controller fields a semantic layout replaces for one page. */
export function familyLayoutSlots(layout: FamilyLayout, page: number) {
	const slots = layout.pages[Math.min(Math.max(page, 1), layout.pages.length) - 1] ?? [];
	return {
		encoderSlots: slots.map((slot) =>
			slot?.kind === "attribute" ? slot.attribute : null,
		),
		encoderPushTurnSlots: slots.map((slot) =>
			slot?.kind === "attribute" ? slot.pushTurnAttribute : null,
		),
		componentSlots: slots.map((slot) =>
			slot?.kind === "component" ? slot : null,
		),
	};
}
