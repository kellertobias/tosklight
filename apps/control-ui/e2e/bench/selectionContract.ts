export interface FixtureReference {
	kind: "fixture";
	number: number;
	/** Omit for the complete fixture, use 0 for its master, or a one-based child head. */
	head?: number;
}

export interface FixtureRangeReference {
	kind: "fixture_range";
	first: number;
	last: number;
	head?: number;
}

export interface GroupReference {
	kind: "group";
	number: number;
}

export interface DereferencedGroupReference {
	kind: "dereferenced_group";
	number: number;
}

export interface GroupRangeReference {
	kind: "group_range";
	first: number;
	last: number;
}

export type SelectionTarget =
	| FixtureReference
	| FixtureRangeReference
	| GroupReference
	| DereferencedGroupReference
	| GroupRangeReference;

export type SelectionPoint =
	| FixtureReference
	| GroupReference
	| DereferencedGroupReference;

export function fixture(number: number, head?: number): FixtureReference {
	assertPositiveInteger(number, "Fixture number");
	if (head !== undefined) assertNonNegativeInteger(head, "Fixture head");
	return head === undefined
		? { kind: "fixture", number }
		: { kind: "fixture", number, head };
}

export function fixtureRange(
	first: number,
	last: number,
	head?: number,
): FixtureRangeReference {
	assertPositiveInteger(first, "First Fixture number");
	assertPositiveInteger(last, "Last Fixture number");
	if (head !== undefined) assertNonNegativeInteger(head, "Fixture head");
	return head === undefined
		? { kind: "fixture_range", first, last }
		: { kind: "fixture_range", first, last, head };
}

export function group(number: number): GroupReference {
	assertPositiveInteger(number, "Group number");
	return { kind: "group", number };
}

export function dereferencedGroup(number: number): DereferencedGroupReference {
	assertPositiveInteger(number, "Group number");
	return { kind: "dereferenced_group", number };
}

export function groupRange(first: number, last: number): GroupRangeReference {
	assertPositiveInteger(first, "First Group number");
	assertPositiveInteger(last, "Last Group number");
	return { kind: "group_range", first, last };
}

export function selectionRange(
	first: SelectionPoint,
	last: SelectionPoint,
): SelectionTarget {
	if (first.kind !== last.kind)
		throw new Error("Selection range endpoints must have matching typed kinds");
	if (first.kind === "fixture" && last.kind === "fixture") {
		if (first.head !== last.head)
			throw new Error("Fixture range endpoints must address the same head");
		return fixtureRange(first.number, last.number, first.head);
	}
	if (first.kind === "group" && last.kind === "group")
		return groupRange(first.number, last.number);
	throw new Error(
		"Dereferenced Group ranges are not a supported selection chunk",
	);
}

export function inclusiveSelectionNumbers(first: number, last: number) {
	const direction = first <= last ? 1 : -1;
	return Array.from(
		{ length: Math.abs(last - first) + 1 },
		(_, index) => first + index * direction,
	);
}

function assertPositiveInteger(value: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 1)
		throw new Error(`${label} must be a positive safe integer`);
}

function assertNonNegativeInteger(value: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 0)
		throw new Error(`${label} must be a non-negative safe integer`);
}
