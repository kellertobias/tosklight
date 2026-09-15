import type { FixtureDefinition } from "@tosklight/patch";
import { describe, expect, it } from "vitest";
import {
	definitionForProfile,
	nextVirtualNumber,
	PRIMITIVE_TYPES,
	previewOf,
	STAGE_TYPES,
	TRUSS_TYPES,
} from "./venueParts";

describe("the Add primitive dialog", () => {
	it("offers a box, a cylinder and a ball, each placed from its own profile in one step", () => {
		expect(PRIMITIVE_TYPES.map((type) => type.label)).toEqual(["Box", "Cylinder", "Ball"]);
		for (const type of PRIMITIVE_TYPES) expect(type.parts).toHaveLength(1);
		expect(PRIMITIVE_TYPES.map((type) => type.parts[0].profileId)).toEqual([
			"0087038f-6a2f-5d74-9185-8d14d7e1ee48",
			"a692c6db-7456-5b70-b681-50af57db2c28",
			"269ae83e-4ea8-5639-9d34-418fc8a08d23",
		]);
		const ids = [...TRUSS_TYPES, ...STAGE_TYPES, ...PRIMITIVE_TYPES].flatMap((type) =>
			type.parts.map((part) => part.profileId),
		);
		expect(new Set(ids).size).toBe(ids.length);
	});
});

const definition = (profileId: string, revision: number, photograph: string | null = null) =>
	({
		id: `${profileId}:mode`,
		revision,
		name: `Profile r${revision}`,
		profile_snapshot: { id: profileId, photograph_asset: photograph },
	}) as unknown as FixtureDefinition;

describe("the CAD add dialogs' parts", () => {
	it("offers the truss sections and stage feet in the order the operator chooses them", () => {
		expect(TRUSS_TYPES.map((type) => type.label)).toEqual([
			"Pipe",
			"2-point",
			"3-point deco",
			"3-point regular",
			"4-point",
			"4-point large",
		]);
		for (const type of TRUSS_TYPES) expect(type.parts[0].label).toMatch(/^Straight/u);
		expect(STAGE_TYPES.map((type) => type.label)).toEqual([
			"Regular feet",
			"Scissor feet",
			"Stairs",
		]);
		// The sections corner pieces are made for list them after the straight truss.
		const partsOf = (id: string) =>
			TRUSS_TYPES.find((type) => type.id === id)?.parts.map((part) => part.label);
		for (const id of ["three-point", "four-point"])
			expect(partsOf(id)).toEqual([
				"Straight truss",
				"Corner 2-way",
				"T-piece 3-way",
				"Corner 3-way down",
				"Cross 4-way",
				"T-piece 4-way down",
				"Cross 5-way down",
				"Node 6-way",
			]);
		expect(partsOf("two-point")).toEqual(["Straight truss"]);
		// Regular feet come in every platform size at each leg height; every part names its own profile.
		expect(STAGE_TYPES[0].parts).toHaveLength(15);
		expect(STAGE_TYPES[0].parts[1]).toMatchObject({ label: "2 × 1 m", detail: "Legs 0.4 m" });
		const ids = [...TRUSS_TYPES, ...STAGE_TYPES].flatMap((type) => type.parts.map((part) => part.profileId));
		expect(new Set(ids).size).toBe(ids.length);
		expect(STAGE_TYPES[1].parts.map((part) => part.label)).toEqual([
			"2 × 1 m",
			"1 × 1 m",
			"1 × 0.5 m",
		]);
	});

	it("places a part from the newest revision of its profile, and nothing when it is missing", () => {
		const library = [definition("a", 1), definition("a", 3, "data:image/png;base64,AA"), definition("b", 2)];
		expect(definitionForProfile(library, "a")?.revision).toBe(3);
		expect(previewOf(definitionForProfile(library, "a"))).toBe("data:image/png;base64,AA");
		expect(previewOf(definitionForProfile(library, "b"))).toBeNull();
		expect(definitionForProfile(library, "missing")).toBeUndefined();
	});

	it("numbers a new object with the first free virtual ID", () => {
		expect(nextVirtualNumber([])).toBe(1);
		expect(nextVirtualNumber([1, 2, null, 4])).toBe(3);
		expect(nextVirtualNumber([2, undefined])).toBe(1);
	});
});
