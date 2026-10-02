import type {
	FamilyEncoderGroup,
	FamilyEncoderPage,
	FamilyEncoderPagesSnapshot,
	FamilyEncoderSlot,
} from "./generated/light-wire";
import {
	arrayAt,
	booleanAt,
	enumAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { decodeProgrammingComponent } from "./programmingIntentWire";
import { WireValidationError } from "./wireValidation";

/**
 * `GET /api/v2/programming/family-encoder-pages` (TL-549/550/551 UI foundation).
 *
 * The decoder validates every field the encoder binding relies on and tolerates unknown fields
 * (api-rules §5). Slots keep the generated wire types unchanged.
 */

export const FAMILY_ENCODER_PAGES_PATH = "/api/v2/programming/family-encoder-pages";

/** Owners per request; matches the server bound. Larger selections are truncated by the caller. */
export const MAX_FAMILY_ENCODER_FIXTURES = 512;

export function familyEncoderPagesPath(fixtureIds: readonly string[]) {
	const owners = fixtureIds
		.slice(0, MAX_FAMILY_ENCODER_FIXTURES)
		.map(encodeURIComponent)
		.join(",");
	return `${FAMILY_ENCODER_PAGES_PATH}?fixture_ids=${owners}`;
}

const FAMILIES = ["position", "color", "focus"] as const;
const PRESENTATIONS = ["easy_rgbw", "easy_rgbwauv", "advanced"] as const;
const EDIT_KINDS = ["scalar", "target_reference", "unavailable"] as const;
const LIMITS_SOURCES = [
	"descriptor",
	"selection",
	"mixed",
	"unknown",
	"unbounded",
] as const;

function stringsAt(value: unknown, path: string) {
	return arrayAt(value, path).map((entry, index) =>
		stringAt(entry, `${path}[${index}]`),
	);
}

function decodeSlot(value: unknown, path: string): FamilyEncoderSlot | null {
	if (value === null) return null;
	const slot = recordAt(value, path);
	const kind = enumAt(slot.kind, `${path}.kind`, ["component", "attribute"]);
	if (kind === "attribute") {
		stringAt(slot.attribute, `${path}.attribute`);
		stringAt(slot.label, `${path}.label`);
		return slot as unknown as FamilyEncoderSlot;
	}
	stringAt(slot.id, `${path}.id`);
	stringAt(slot.label, `${path}.label`);
	decodeProgrammingComponent(slot.component, `${path}.component`);
	const descriptor = recordAt(slot.descriptor, `${path}.descriptor`);
	for (const field of ["step", "fine_step", "display_scale"] as const)
		if (typeof descriptor[field] !== "number" || !Number.isFinite(descriptor[field]))
			throw new WireValidationError(
				`${path}.descriptor.${field}`,
				"finite number",
				descriptor[field],
			);
	stringAt(descriptor.unit, `${path}.descriptor.unit`);
	stringAt(descriptor.owner, `${path}.descriptor.owner`);
	enumAt(slot.edit, `${path}.edit`, EDIT_KINDS);
	enumAt(slot.limits_source, `${path}.limits_source`, LIMITS_SOURCES);
	if (slot.limits != null) {
		const limits = recordAt(slot.limits, `${path}.limits`);
		if (typeof limits.min !== "number" || typeof limits.max !== "number")
			throw new WireValidationError(`${path}.limits`, "{min, max}", slot.limits);
	}
	stringsAt(slot.fixture_ids, `${path}.fixture_ids`);
	return slot as unknown as FamilyEncoderSlot;
}

function decodePage(value: unknown, path: string): FamilyEncoderPage {
	const page = recordAt(value, path);
	integerAt(page.number, `${path}.number`);
	stringAt(page.label, `${path}.label`);
	arrayAt(page.slots, `${path}.slots`).forEach((slot, index) =>
		decodeSlot(slot, `${path}.slots[${index}]`),
	);
	return page as unknown as FamilyEncoderPage;
}

function decodeGroup(value: unknown, path: string): FamilyEncoderGroup {
	const group = recordAt(value, path);
	enumAt(group.family, `${path}.family`, FAMILIES);
	stringsAt(group.fixture_ids, `${path}.fixture_ids`);
	stringsAt(group.replaces_attributes, `${path}.replaces_attributes`);
	stringsAt(
		group.replaces_attribute_prefixes,
		`${path}.replaces_attribute_prefixes`,
	);
	arrayAt(group.pages, `${path}.pages`).forEach((page, index) =>
		decodePage(page, `${path}.pages[${index}]`),
	);
	arrayAt(group.reserved_pages, `${path}.reserved_pages`).forEach((page, index) =>
		integerAt(
			recordAt(page, `${path}.reserved_pages[${index}]`).number,
			`${path}.reserved_pages[${index}].number`,
		),
	);
	return group as unknown as FamilyEncoderGroup;
}

export function decodeFamilyEncoderPagesSnapshot(
	value: unknown,
): FamilyEncoderPagesSnapshot {
	const snapshot = recordAt(value, "$");
	booleanAt(snapshot.semantic, "$.semantic");
	integerAt(
		snapshot.supported_programming_contract,
		"$.supported_programming_contract",
	);
	enumAt(snapshot.color_presentation, "$.color_presentation", PRESENTATIONS);
	stringsAt(snapshot.fixture_ids, "$.fixture_ids");
	arrayAt(snapshot.families, "$.families").forEach((group, index) =>
		decodeGroup(group, `$.families[${index}]`),
	);
	return snapshot as unknown as FamilyEncoderPagesSnapshot;
}
