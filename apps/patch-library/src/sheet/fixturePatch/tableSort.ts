import type { PatchedFixture } from "../../wire";
import { isDmxPatchable } from "../patchUtils";
import { compareFixtureIds } from "./fixtureIds";
import { effectiveSplitPatches } from "./patchModel";

/** The patch sheet columns whose header orders the table. */
export const SORTABLE_PATCH_COLUMNS = [
	"Type",
	"Fixture ID",
	"Name",
	"Manufacturer",
	"Product / mode",
	"Patch",
	"Location X",
	"Location Y",
	"Location Z",
	"Rotation X",
	"Rotation Y",
	"Rotation Z",
	"Layer",
	"Note",
] as const;

export type PatchSortColumn = (typeof SORTABLE_PATCH_COLUMNS)[number];
export type PatchSort = {
	column: PatchSortColumn;
	direction: "ascending" | "descending";
};

export const DEFAULT_PATCH_SORT: PatchSort = {
	column: "Fixture ID",
	direction: "ascending",
};

export function isPatchSortColumn(column: string): column is PatchSortColumn {
	return (SORTABLE_PATCH_COLUMNS as readonly string[]).includes(column);
}

/** A second click on the ordering column reverses it; any other column starts ascending. */
export function nextPatchSort(
	current: PatchSort,
	column: PatchSortColumn,
): PatchSort {
	if (current.column !== column) return { column, direction: "ascending" };
	return {
		column,
		direction: current.direction === "ascending" ? "descending" : "ascending",
	};
}

export type PatchSortContext = {
	/** Stored layer order by layer ID; the implicit default layer comes first. */
	layerOrder?: ReadonlyMap<string, number>;
	note?: (fixtureId: string) => string | undefined;
};

type SortKey = string | number | null;

const collator = new Intl.Collator(undefined, {
	numeric: true,
	sensitivity: "base",
});

/**
 * The fixtures in the order `sort` asks for. A fixture with nothing in the column — unpatched, no
 * fixture ID, no note — sorts after every fixture that has a value, in either direction, and ties
 * keep fixture ID order so the table never shuffles between renders.
 */
export function sortPatchFixtures(
	fixtures: readonly PatchedFixture[],
	sort: PatchSort,
	context: PatchSortContext = {},
): PatchedFixture[] {
	const sign = sort.direction === "ascending" ? 1 : -1;
	if (sort.column === "Fixture ID")
		return [...fixtures].sort((a, b) => {
			if (hasFixtureId(a) !== hasFixtureId(b)) return hasFixtureId(a) ? -1 : 1;
			return sign * compareFixtureIds(a, b);
		});
	return fixtures
		.map((fixture) => ({
			fixture,
			key: sortKey(fixture, sort.column, context),
		}))
		.sort(
			(a, b) =>
				compareKeys(a.key, b.key, sign) ||
				compareFixtureIds(a.fixture, b.fixture),
		)
		.map((item) => item.fixture);
}

function hasFixtureId(fixture: PatchedFixture) {
	return (
		fixture.virtual_fixture_number != null || fixture.fixture_number != null
	);
}

function compareKeys(a: SortKey, b: SortKey, sign: number) {
	if (a === null || b === null) return a === b ? 0 : a === null ? 1 : -1;
	const order =
		typeof a === "number" && typeof b === "number"
			? a - b
			: collator.compare(String(a), String(b));
	return sign * order;
}

function sortKey(
	fixture: PatchedFixture,
	column: PatchSortColumn,
	context: PatchSortContext,
): SortKey {
	const definition = fixture.definition;
	switch (column) {
		case "Type":
			return definition.device_type || null;
		case "Name":
			return fixture.name || definition.name || null;
		case "Manufacturer":
			return definition.manufacturer || null;
		case "Product / mode":
			return `${definition.model} · ${definition.mode}`;
		case "Patch":
			return patchKey(fixture);
		case "Layer":
			return context.layerOrder?.get(fixture.layer_id || "default") ?? -1;
		case "Note":
			return context.note?.(fixture.fixture_id)?.trim() || null;
		case "Location X":
		case "Location Y":
		case "Location Z":
			return fixture.location?.[axisOf(column)] ?? 0;
		case "Rotation X":
		case "Rotation Y":
		case "Rotation Z":
			return fixture.rotation?.[axisOf(column)] ?? 0;
		default:
			return null;
	}
}

function axisOf(column: string) {
	return column.slice(-1).toLowerCase() as "x" | "y" | "z";
}

/** Universe, then address, of the fixture's first patched split; nothing when it is unpatched. */
function patchKey(fixture: PatchedFixture): number | null {
	if (!isDmxPatchable(fixture.definition)) return null;
	const first = effectiveSplitPatches(
		fixture.definition,
		fixture.split_patches,
		fixture.universe,
		fixture.address,
	).find((patch) => patch.universe != null && patch.address != null);
	return first?.universe != null && first.address != null
		? first.universe * 1024 + first.address
		: null;
}
