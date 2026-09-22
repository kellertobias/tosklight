import type { FixtureDefinition } from "@tosklight/patch";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { chosenPart, rememberPart } from "./cadAddChoice";
import {
	CAD_PART_CATALOGUE,
	CURTAIN_TYPES,
	DEFAULT_PART_PROFILE_IDS,
	definitionForProfile,
	findPart,
	matchesVenueQuery,
	nextVirtualNumber,
	PART_MENU_PROFILE_IDS,
	PARAMETRIC_CURTAIN_PROFILE_ID,
	PRIMITIVE_TYPES,
	partLabel,
	previewOf,
	STAGE_TYPES,
	TRUSS_TYPES,
	venueProfiles,
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

describe("the CAD part buttons' catalogue", () => {
	it("offers the parametric curtain first, then the fixed widths", () => {
		expect(CURTAIN_TYPES.map((type) => type.label)).toEqual(["Any width", "Fixed width"]);
		expect(CURTAIN_TYPES[0].parts[0].profileId).toBe(PARAMETRIC_CURTAIN_PROFILE_ID);
		expect(CURTAIN_TYPES[1].parts.map((part) => part.label)).toEqual(["1 m", "2 m", "3 m", "5 m", "6 m"]);
		const ids = Object.values(CAD_PART_CATALOGUE).flatMap((groups) =>
			groups.flatMap((group) => group.parts.map((part) => part.profileId)),
		);
		expect(new Set(ids).size).toBe(ids.length);
	});

	it("defaults every button to one of its own parts and names parts with their group", () => {
		for (const [kind, profileId] of Object.entries(DEFAULT_PART_PROFILE_IDS))
			expect(findPart(kind as keyof typeof DEFAULT_PART_PROFILE_IDS, profileId)).toBeDefined();
		expect(findPart("truss", PARAMETRIC_CURTAIN_PROFILE_ID)).toBeUndefined();
		const corner = findPart("truss", "3ea0f8ad-c38d-5ec6-a4f7-6d918a1e974e");
		expect(corner && partLabel(corner)).toBe("Corner 2-way (4-point)");
		const pipe = findPart("truss", "6eb48efc-34c9-568a-be7a-c4611fb94996");
		expect(pipe && partLabel(pipe)).toBe("Straight pipe");
	});
});

describe("the remembered part of each button", () => {
	const store = new Map<string, string>();
	beforeEach(() => {
		store.clear();
		vi.stubGlobal("localStorage", {
			getItem: (key: string) => store.get(key) ?? null,
			setItem: (key: string, value: string) => store.set(key, value),
		});
	});

	it("keeps a choice per button and falls back to the default for anything else", () => {
		expect(chosenPart("truss").part.profileId).toBe(DEFAULT_PART_PROFILE_IDS.truss);
		rememberPart("truss", "3ea0f8ad-c38d-5ec6-a4f7-6d918a1e974e");
		expect(chosenPart("truss").part.label).toBe("Corner 2-way");
		expect(chosenPart("stage").part.profileId).toBe(DEFAULT_PART_PROFILE_IDS.stage);
		// A profile the button does not offer is not the button's part.
		rememberPart("curtain", DEFAULT_PART_PROFILE_IDS.truss);
		expect(chosenPart("curtain").part.profileId).toBe(PARAMETRIC_CURTAIN_PROFILE_ID);
	});

	it("still answers when storage refuses", () => {
		vi.stubGlobal("localStorage", {
			getItem: () => {
				throw new Error("blocked");
			},
			setItem: () => {
				throw new Error("blocked");
			},
		});
		expect(() => rememberPart("primitive", PRIMITIVE_TYPES[2].parts[0].profileId)).not.toThrow();
		expect(chosenPart("primitive").part.profileId).toBe(DEFAULT_PART_PROFILE_IDS.primitive);
	});
});

describe("the Add venue element list", () => {
	const profiled = (
		profileId: string,
		name: string,
		revision: number,
		profile: Record<string, unknown>,
	) =>
		({
			id: `${profileId}:mode`,
			revision,
			name,
			manufacturer: String(profile.manufacturer ?? ""),
			profile_snapshot: { id: profileId, short_name: name, ...profile },
		}) as unknown as FixtureDefinition;

	it("lists each Venue or visual-only profile once at its newest revision, by name, and searches it", () => {
		const library = [
			profiled("truss", "Truss", 1, { manufacturer: "Venue", fixture_type: "rigging" }),
			profiled("truss", "Truss", 2, { manufacturer: "Venue", fixture_type: "rigging" }),
			profiled("model", "Balcony model", 1, { manufacturer: "Imported", patch_policy: "visual_only", fixture_type: "venue" }),
			profiled("par", "LED Par", 1, { manufacturer: "Generic", patch_policy: "dmx", fixture_type: "par" }),
		];
		const listed = venueProfiles(library);
		expect(listed.map((entry) => [entry.profileId, entry.definition.revision])).toEqual([
			["model", 1],
			["truss", 2],
		]);
		expect(matchesVenueQuery(listed[0].definition, "  BALC ")).toBe(true);
		expect(matchesVenueQuery(listed[1].definition, "rigging")).toBe(true);
		expect(matchesVenueQuery(listed[1].definition, "balcony")).toBe(false);
		expect(matchesVenueQuery(listed[1].definition, "")).toBe(true);
	});

	it("leaves out every profile an add button and its part menu already offer", () => {
		const shipped = Object.entries(CAD_PART_CATALOGUE).flatMap(([kind, groups]) =>
			groups.flatMap((group) =>
				group.parts.map((part) =>
					profiled(part.profileId, `${kind} ${part.id}`, 1, {
						manufacturer: "Venue",
						fixture_type: "venue",
					}),
				),
			),
		);
		const railing = profiled("railing", "Stage Railing 2 m", 1, {
			manufacturer: "Venue",
			fixture_type: "venue",
		});
		expect(venueProfiles([...shipped, railing]).map((entry) => entry.profileId)).toEqual([
			"railing",
		]);
		for (const part of shipped) {
			expect(PART_MENU_PROFILE_IDS.has(part.profile_snapshot?.id ?? "")).toBe(true);
		}
	});
});
