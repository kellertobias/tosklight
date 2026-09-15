/**
 * What the CAD add buttons offer: the shipped Venue profiles a truss, a stage element or a curtain is
 * placed from, grouped the way the operator chooses them.
 *
 * A truss is chosen by its section first and then the part — the straight run at any length, or one
 * of the corner pieces made for that section. A stage element is chosen by what it stands on and then
 * its platform size. A curtain is one parametric profile and needs no choosing.
 *
 * Profiles are named by their fixed ids, so a renamed profile still lands in the right place. A part
 * whose profile is missing from this machine's library is offered but cannot be placed.
 */
import type { FixtureDefinition } from "@tosklight/patch";

export interface VenuePart {
	id: string;
	label: string;
	profileId: string;
	/** A second line under the label, such as a deck's leg height. */
	detail?: string;
}

/** The corner pieces made for a 3- or 4-point section, each a fixed block with 500 mm arms. */
function corners(section: "three" | "four", ids: readonly string[]): VenuePart[] {
	const names = [
		"Corner 2-way",
		"T-piece 3-way",
		"Corner 3-way down",
		"Cross 4-way",
		"T-piece 4-way down",
		"Cross 5-way down",
		"Node 6-way",
	];
	return names.map((label, index) => ({
		id: `${section}-${index}`,
		label,
		profileId: ids[index],
	}));
}

/** A stage deck on fixed legs, one profile per platform size and leg height. */
function legged(size: string, ids: readonly string[]): VenuePart[] {
	return ["0.2", "0.4", "0.6", "0.8", "1"].map((legs, index) => ({
		id: `${size}-${legs}`,
		label: `${size} m`,
		detail: `Legs ${legs} m`,
		profileId: ids[index],
	}));
}

export interface VenuePartGroup {
	id: string;
	label: string;
	/** What the second step asks for. */
	partsLabel: string;
	parts: readonly VenuePart[];
}

export const TRUSS_TYPES: readonly VenuePartGroup[] = [
	{
		id: "pipe",
		label: "Pipe",
		partsLabel: "Part",
		parts: [{ id: "straight", label: "Straight pipe", profileId: "6eb48efc-34c9-568a-be7a-c4611fb94996" }],
	},
	{
		id: "two-point",
		label: "2-point",
		partsLabel: "Part",
		parts: [{ id: "straight", label: "Straight truss", profileId: "f2a972f0-5e5e-5dae-bbd9-381bc94a52b1" }],
	},
	{
		id: "three-point-deco",
		label: "3-point deco",
		partsLabel: "Part",
		parts: [{ id: "straight", label: "Straight truss", profileId: "4d7757f0-2e5f-5f33-9eca-5370ab580322" }],
	},
	{
		id: "three-point",
		label: "3-point regular",
		partsLabel: "Part",
		parts: [
			{ id: "straight", label: "Straight truss", profileId: "44097b39-11b4-5bd4-af61-8adb97d426b1" },
			...corners("three", [
				"952c967c-db0b-5e41-9c18-21fee6ac079c",
				"57b9d32f-3491-5135-ab88-f352e21ee8a6",
				"d0f4106e-3dca-5022-9be0-4bf3b26a8194",
				"db344e61-fde1-5cfd-8b71-2f036d4a5d0f",
				"7a34b0ce-f2ec-5978-954a-1cb51361ef7e",
				"5f4c75c8-aa92-500c-92cc-6feacd4f40a7",
				"00b48d4b-0120-5bdb-8551-ea904f42b039",
			]),
		],
	},
	{
		id: "four-point",
		label: "4-point",
		partsLabel: "Part",
		parts: [
			{ id: "straight", label: "Straight truss", profileId: "562e7947-8284-5ec8-9750-3cd3fe6c1c6d" },
			...corners("four", [
				"3ea0f8ad-c38d-5ec6-a4f7-6d918a1e974e",
				"bb8dca1c-3719-5329-abd7-4d9799daaced",
				"fa67b29e-15db-5933-a27c-d2bbdc4849cd",
				"1ad3789e-a787-57a4-8919-172bc5fbe8d6",
				"18e05ea6-30fb-51c8-9de3-4be6288d98e9",
				"b7080bd2-0515-50cf-9f3e-4ff26fca0884",
				"21a6a058-ab9e-5b84-abf5-6d4c93371a86",
			]),
		],
	},
	{
		id: "four-point-large",
		label: "4-point large",
		partsLabel: "Part",
		parts: [{ id: "straight", label: "Straight truss", profileId: "67fda092-ef0a-5d50-b28e-da0ac39d5a99" }],
	},
];

export const STAGE_TYPES: readonly VenuePartGroup[] = [
	{
		id: "regular",
		label: "Regular feet",
		partsLabel: "Platform size",
		parts: [
			...legged("2 × 1", [
				"f5cb3a55-4e4f-5dfd-8c0e-43cf7924b096",
				"3cf7a16e-95e8-5cf3-bd54-e65743883acf",
				"9f510d06-6bb8-5dd1-bd7c-6774226d1586",
				"a1b0a402-7953-562c-8c57-6346df823ce3",
				"6541286a-f448-55c5-98ef-9e707b8e5a36",
			]),
			...legged("1 × 1", [
				"bf818699-247f-5db9-b5b7-daad4137e57c",
				"115d33ff-1189-5f89-a9eb-3c19993f491a",
				"e3fdb557-e013-5c40-8efa-1d24cb1a7714",
				"7edb68c9-efcc-547c-80f3-bdcb1fa03003",
				"fe6992bc-9981-52e4-916d-9b1b8fb17c1e",
			]),
			...legged("1 × 0.5", [
				"c1d26de5-4fe2-594d-99e2-812145650314",
				"6476799e-fc6f-5ddc-b4f6-fa1325881aa2",
				"64e0ee52-d22e-5073-991c-1c93272ea285",
				"b2bf9af3-36f3-5bf4-8a19-0f58e9c034f1",
				"811e02d4-82e0-5962-85a7-3efd81a226ff",
			]),
		],
	},
	{
		id: "scissor",
		label: "Scissor feet",
		partsLabel: "Platform size",
		parts: [
			{ id: "2x1", label: "2 × 1 m", profileId: "6ad1c9f3-5024-543e-8478-f34ae63d338b" },
			{ id: "1x1", label: "1 × 1 m", profileId: "c7e53038-043c-5202-9720-85ad06a6d68a" },
			{ id: "1x0.5", label: "1 × 0.5 m", profileId: "bec6cfb0-5c4f-5507-9bdb-808d4381339d" },
		],
	},
	{
		id: "stairs",
		label: "Stairs",
		partsLabel: "Platform size",
		parts: [{ id: "stairs", label: "1 m wide", profileId: "d5982d33-9723-5749-ade6-7be0e6b4adf1" }],
	},
];

/** The parametric curtain: placed at once, then sized in Info. */
export const PARAMETRIC_CURTAIN_PROFILE_ID = "6f34b81e-3f71-5d35-b8fb-b4b0b7cce859";

/** The newest revision of a profile in the library, or undefined when this machine does not have it. */
export function definitionForProfile(
	definitions: readonly FixtureDefinition[],
	profileId: string,
): FixtureDefinition | undefined {
	return definitions
		.filter((definition) => (definition.profile_snapshot?.id ?? definition.id) === profileId)
		.sort((a, b) => b.revision - a.revision)[0];
}

/** A picture of a profile to show on a dark ground, when its package carries one. */
export function previewOf(definition: FixtureDefinition | undefined): string | null {
	return definition?.profile_snapshot?.photograph_asset ?? null;
}

/** The first virtual fixture number no placed object uses: 0.1, then 0.2 and so on. */
export function nextVirtualNumber(used: Iterable<number | null | undefined>): number {
	const taken = new Set<number>();
	for (const number of used) if (number != null) taken.add(number);
	let next = 1;
	while (taken.has(next)) next++;
	return next;
}
