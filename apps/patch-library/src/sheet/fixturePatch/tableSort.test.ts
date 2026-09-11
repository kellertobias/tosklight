import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../wire";
import {
	DEFAULT_PATCH_SORT,
	nextPatchSort,
	type PatchSort,
	sortPatchFixtures,
} from "./tableSort";

function fixture(
	id: string,
	overrides: Partial<PatchedFixture> = {},
): PatchedFixture {
	return {
		fixture_id: id,
		fixture_number: null,
		name: id,
		definition: {
			manufacturer: "Acme",
			device_type: "wash",
			name: "Wash",
			model: "Wash",
			mode: "Default",
			footprint: 4,
		} as PatchedFixture["definition"],
		universe: null,
		address: null,
		split_patches: [],
		layer_id: "default",
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logical_heads: [],
		multipatch: [],
		...overrides,
	} as PatchedFixture;
}

const ids = (fixtures: readonly PatchedFixture[]) =>
	fixtures.map((item) => item.fixture_id);

const ascending = (column: PatchSort["column"]): PatchSort => ({
	column,
	direction: "ascending",
});
const descending = (column: PatchSort["column"]): PatchSort => ({
	column,
	direction: "descending",
});

describe("patch sheet ordering", () => {
	it("orders by universe then address, with unpatched fixtures last both ways", () => {
		const rig = [
			fixture("second-universe", { fixture_number: 1, universe: 2, address: 1 }),
			fixture("unpatched", { fixture_number: 2 }),
			fixture("address-ten", { fixture_number: 3, universe: 1, address: 10 }),
			fixture("address-two", { fixture_number: 4, universe: 1, address: 2 }),
		];
		expect(ids(sortPatchFixtures(rig, ascending("Patch")))).toEqual([
			"address-two",
			"address-ten",
			"second-universe",
			"unpatched",
		]);
		expect(ids(sortPatchFixtures(rig, descending("Patch")))).toEqual([
			"second-universe",
			"address-ten",
			"address-two",
			"unpatched",
		]);
	});

	it("reverses fixture IDs but keeps fixtures without one last", () => {
		const rig = [
			fixture("1102", { fixture_number: 1102 }),
			fixture("none"),
			fixture("1100", { fixture_number: 1100 }),
			fixture("1101", { fixture_number: 1101 }),
		];
		expect(ids(sortPatchFixtures(rig, DEFAULT_PATCH_SORT))).toEqual([
			"1100",
			"1101",
			"1102",
			"none",
		]);
		expect(ids(sortPatchFixtures(rig, descending("Fixture ID")))).toEqual([
			"1102",
			"1101",
			"1100",
			"none",
		]);
	});

	it("sorts names naturally and keeps fixture ID order between equal names", () => {
		const rig = [
			fixture("wash-10", { fixture_number: 3, name: "Wash 10" }),
			fixture("second-wash-2", { fixture_number: 2, name: "Wash 2" }),
			fixture("first-wash-2", { fixture_number: 1, name: "Wash 2" }),
		];
		expect(ids(sortPatchFixtures(rig, ascending("Name")))).toEqual([
			"first-wash-2",
			"second-wash-2",
			"wash-10",
		]);
		expect(ids(sortPatchFixtures(rig, descending("Name")))).toEqual([
			"wash-10",
			"first-wash-2",
			"second-wash-2",
		]);
	});

	it("orders layers by their stored order and puts fixtures without a note last", () => {
		const rig = [
			fixture("floor", { fixture_number: 1, layer_id: "floor" }),
			fixture("truss", { fixture_number: 2, layer_id: "truss" }),
			fixture("stage", { fixture_number: 3, layer_id: "" }),
		];
		const layerOrder = new Map([
			["truss", 1],
			["floor", 2],
		]);
		expect(
			ids(sortPatchFixtures(rig, ascending("Layer"), { layerOrder })),
		).toEqual(["stage", "truss", "floor"]);

		const notes = new Map([
			["floor", "Zoom check"],
			["truss", "Aim"],
		]);
		expect(
			ids(
				sortPatchFixtures(rig, descending("Note"), {
					note: (id) => notes.get(id),
				}),
			),
		).toEqual(["floor", "truss", "stage"]);
	});

	it("reverses on a second click and starts any other column ascending", () => {
		const byPatch = nextPatchSort(DEFAULT_PATCH_SORT, "Patch");
		expect(byPatch).toEqual(ascending("Patch"));
		expect(nextPatchSort(byPatch, "Patch")).toEqual(descending("Patch"));
		expect(nextPatchSort(descending("Patch"), "Patch")).toEqual(
			ascending("Patch"),
		);
		expect(nextPatchSort(descending("Patch"), "Name")).toEqual(
			ascending("Name"),
		);
	});
});
