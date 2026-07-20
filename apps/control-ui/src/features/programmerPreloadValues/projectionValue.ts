import type { AttributeValue } from "../../api/types/playback";
import type {
	ProgrammerPreloadFixtureValue,
	ProgrammerPreloadGroupValue,
	ProgrammerPreloadValuesProjection,
} from "./contracts";
import { ProgrammerPreloadValuesProtocolError } from "./transport";

export function canonicalPreloadProjection(
	projection: ProgrammerPreloadValuesProjection,
): ProgrammerPreloadValuesProjection {
	assertNonNegativeInteger(projection.revision, "revision");
	if (!projection.userId)
		throw new ProgrammerPreloadValuesProtocolError(
			"Preload Programmer values projection is missing its user",
		);
	const fixtureValues = projection.fixtureValues.map(canonicalFixtureValue);
	const groupValues = projection.groupValues.map(canonicalGroupValue);
	assertUnique(
		fixtureValues,
		(entry) => `${entry.fixtureId}\u0000${entry.attribute}`,
	);
	assertUnique(
		groupValues,
		(entry) => `${entry.groupId}\u0000${entry.attribute}`,
	);
	fixtureValues.sort(compareFixtureValues);
	groupValues.sort(compareGroupValues);
	return Object.freeze({
		userId: projection.userId,
		revision: projection.revision,
		fixtureValues: Object.freeze(fixtureValues),
		groupValues: Object.freeze(groupValues),
	});
}

export function samePreloadProjection(
	left: ProgrammerPreloadValuesProjection,
	right: ProgrammerPreloadValuesProjection,
) {
	return sameValue(left, right);
}

export function assertPreloadCursor(cursor: number) {
	assertNonNegativeInteger(cursor, "event cursor");
}

function canonicalFixtureValue(
	entry: ProgrammerPreloadFixtureValue,
): ProgrammerPreloadFixtureValue {
	assertAddress(entry.fixtureId, entry.attribute, "fixture");
	assertTiming(entry);
	return Object.freeze({
		...entry,
		value: canonicalAttributeValue(entry.value),
	});
}

function canonicalGroupValue(
	entry: ProgrammerPreloadGroupValue,
): ProgrammerPreloadGroupValue {
	assertAddress(entry.groupId, entry.attribute, "group");
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
		throw new ProgrammerPreloadValuesProtocolError(
			`Preload Programmer ${label} value has an empty address`,
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

function assertNonNegativeInteger(value: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 0)
		throw new ProgrammerPreloadValuesProtocolError(
			`Preload Programmer values ${label} must be a non-negative integer`,
		);
}

function assertUnique<T>(values: readonly T[], key: (value: T) => string) {
	const addresses = new Set<string>();
	for (const value of values) {
		const address = key(value);
		if (addresses.has(address))
			throw new ProgrammerPreloadValuesProtocolError(
				"Preload Programmer values projection contains a duplicate address",
			);
		addresses.add(address);
	}
}

function compareFixtureValues(
	left: ProgrammerPreloadFixtureValue,
	right: ProgrammerPreloadFixtureValue,
) {
	return (
		left.programmerOrder - right.programmerOrder ||
		left.fixtureId.localeCompare(right.fixtureId) ||
		left.attribute.localeCompare(right.attribute)
	);
}

function compareGroupValues(
	left: ProgrammerPreloadGroupValue,
	right: ProgrammerPreloadGroupValue,
) {
	return (
		left.programmerOrder - right.programmerOrder ||
		left.groupId.localeCompare(right.groupId) ||
		left.attribute.localeCompare(right.attribute)
	);
}

function sameValue(left: unknown, right: unknown): boolean {
	if (Object.is(left, right)) return true;
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
