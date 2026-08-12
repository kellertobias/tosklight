import type { AttributeValue } from "../../api/types/playback";
import type {
	ProgrammerDynamicValue,
	ProgrammerFixtureValue,
	ProgrammerGroupValue,
	ProgrammerValuesProjection,
} from "./contracts";
import { ProgrammerValuesProtocolError } from "./transport";

export function canonicalProjection(
	projection: ProgrammerValuesProjection,
): ProgrammerValuesProjection {
	assertRevision(projection.revision);
	if (!projection.userId)
		throw new ProgrammerValuesProtocolError(
			"Programmer values projection is missing its user",
		);
	const fixtureValues = projection.fixtureValues.map(canonicalFixtureValue);
	const groupValues = projection.groupValues.map(canonicalGroupValue);
	const dynamicValues = (projection.dynamicValues ?? []).map((entry) =>
		Object.freeze({ ...entry }),
	);
	assertUnique(
		fixtureValues,
		(entry) => `${entry.fixtureId}\u0000${entry.attribute}`,
		(entry) =>
			`Fixture ${entry.fixtureId} has more than one Programmer value for ${entry.attribute}. This is an internal Programmer authority duplication, not a DMX patch overlap. Reload the desk state; if it returns, inspect that fixture's profile, heads, and multipatch data.`,
	);
	assertUnique(
		groupValues,
		(entry) => `${entry.groupId}\u0000${entry.attribute}`,
		(entry) =>
			`Group ${entry.groupId} has more than one Programmer value for ${entry.attribute}. Reload the desk state and remove the duplicate stored Group value if it returns.`,
	);
	assertUnique(
		dynamicValues,
		(entry) => dynamicAddress(entry),
		(entry) =>
			`Dynamic control track ${dynamicInstanceLabel(entry)} projected ${entry.attribute} more than once for fixture ${entry.fixtureId}. Stop and restart the affected Dynamic; inspect that instance track if the duplicate returns.`,
	);
	fixtureValues.sort(compareFixtureValues);
	groupValues.sort(compareGroupValues);
	dynamicValues.sort(compareDynamicValues);
	return Object.freeze({
		userId: projection.userId,
		revision: projection.revision,
		fixtureValues: Object.freeze(fixtureValues),
		groupValues: Object.freeze(groupValues),
		...(dynamicValues.length > 0
			? { dynamicValues: Object.freeze(dynamicValues) }
			: {}),
	});
}

function compareDynamicValues(
	left: ProgrammerDynamicValue,
	right: ProgrammerDynamicValue,
) {
	return (
		left.programmerOrder - right.programmerOrder ||
		left.fixtureId.localeCompare(right.fixtureId) ||
		left.attribute.localeCompare(right.attribute) ||
		dynamicInstanceLabel(left).localeCompare(dynamicInstanceLabel(right))
	);
}

export function dynamicInstanceLink(
	entry: ProgrammerDynamicValue,
): string | null {
	return entry.value.type === "dynamic_on" || entry.value.type === "dynamic_off"
		? entry.value.instance_link
		: null;
}

function dynamicAddress(entry: ProgrammerDynamicValue) {
	return `${entry.fixtureId}\u0000${entry.attribute}\u0000${dynamicInstanceLabel(entry)}`;
}

function dynamicInstanceLabel(entry: ProgrammerDynamicValue) {
	return dynamicInstanceLink(entry) ?? "static";
}

export function sameProjection(
	left: ProgrammerValuesProjection,
	right: ProgrammerValuesProjection,
) {
	return sameValue(left, right);
}

function canonicalFixtureValue(
	entry: ProgrammerFixtureValue,
): ProgrammerFixtureValue {
	assertAddress(entry.fixtureId, entry.attribute, "fixture");
	assertTiming(entry);
	return Object.freeze({
		...entry,
		value: canonicalAttributeValue(entry.value),
	});
}

function canonicalGroupValue(
	entry: ProgrammerGroupValue,
): ProgrammerGroupValue {
	assertAddress(entry.groupId, entry.attribute, "Group");
	assertTiming(entry);
	return Object.freeze({
		...entry,
		value: canonicalAttributeValue(entry.value),
	});
}

function canonicalAttributeValue(value: AttributeValue): AttributeValue {
	switch (value.kind) {
		case "spread": {
			const spread = [...value.value];
			Object.freeze(spread);
			return Object.freeze({ ...value, value: spread });
		}
		case "color_xyz":
			return Object.freeze({
				...value,
				value: Object.freeze({ ...value.value }),
			});
		default:
			return Object.freeze({ ...value });
	}
}

function assertAddress(id: string, attribute: string, label: string) {
	if (!id || !attribute)
		throw new ProgrammerValuesProtocolError(
			`Programmer ${label} value has an empty address`,
		);
}

function assertTiming(entry: {
	programmerOrder: number;
	fadeMillis: number | null;
	delayMillis: number | null;
}) {
	assertNonNegativeInteger(entry.programmerOrder, "programmer order");
	if (entry.fadeMillis !== null)
		assertNonNegativeInteger(entry.fadeMillis, "fade duration");
	if (entry.delayMillis !== null)
		assertNonNegativeInteger(entry.delayMillis, "delay duration");
}

function assertRevision(revision: number) {
	assertNonNegativeInteger(revision, "revision");
}

export function assertCursor(cursor: number) {
	assertNonNegativeInteger(cursor, "event cursor");
}

function assertNonNegativeInteger(value: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 0)
		throw new ProgrammerValuesProtocolError(
			`Programmer values ${label} must be a non-negative integer`,
		);
}

function assertUnique<T>(
	values: readonly T[],
	key: (value: T) => string,
	diagnostic: (value: T) => string,
) {
	const addresses = new Set<string>();
	for (const value of values) {
		const address = key(value);
		if (addresses.has(address))
			throw new ProgrammerValuesProtocolError(diagnostic(value));
		addresses.add(address);
	}
}

function compareFixtureValues(
	left: ProgrammerFixtureValue,
	right: ProgrammerFixtureValue,
) {
	return (
		left.programmerOrder - right.programmerOrder ||
		left.fixtureId.localeCompare(right.fixtureId) ||
		left.attribute.localeCompare(right.attribute)
	);
}

function compareGroupValues(
	left: ProgrammerGroupValue,
	right: ProgrammerGroupValue,
) {
	return (
		left.programmerOrder - right.programmerOrder ||
		left.groupId.localeCompare(right.groupId) ||
		left.attribute.localeCompare(right.attribute)
	);
}

function sameValue(left: unknown, right: unknown): boolean {
	if (Object.is(left, right)) return true;
	if (typeof left === "number" && typeof right === "number") {
		// Programmer normalized, spread, and color components originate as Rust f32 values.
		// The command facade can serialize the widened JavaScript double while event and snapshot
		// routes use the compact f32 spelling. Treat those wire spellings as the same authority,
		// while retaining exact comparison for revisions, ordering, and timing integers.
		return (
			!Number.isInteger(left) &&
			!Number.isInteger(right) &&
			Math.fround(left) === Math.fround(right)
		);
	}
	if (Array.isArray(left) || Array.isArray(right))
		return (
			Array.isArray(left) &&
			Array.isArray(right) &&
			left.length === right.length &&
			left.every((value, index) => sameValue(value, right[index]))
		);
	if (!isRecord(left) || !isRecord(right)) return false;
	const leftKeys = Object.keys(left).sort();
	const rightKeys = Object.keys(right).sort();
	return (
		leftKeys.length === rightKeys.length &&
		leftKeys.every(
			(key, index) =>
				key === rightKeys[index] && sameValue(left[key], right[key]),
		)
	);
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}
