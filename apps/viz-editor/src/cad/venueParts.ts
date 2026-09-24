/**
 * What the CAD add buttons offer: the shipped Venue profiles a truss, a stage element or a curtain is
 * placed from, grouped the way the operator chooses them.
 *
 * A truss is listed by its section and then the part — the straight run at any length, or one of the
 * corner pieces made for that section. A stage element is listed by what it stands on and then its
 * platform size, and is raised to the height it is placed at. The scenic elements — the parametric
 * curtain, chain, disco ball, stage railing and the flight rack — share one button.
 * Each add button places one of its parts at once, and its caret menu chooses which.
 *
 * Profiles are named by their fixed ids, so a renamed profile still lands in the right place. A part
 * whose profile is missing from this machine's library is offered but cannot be placed.
 */
import type { FixtureDefinition, PatchFixtureWrite } from "@tosklight/patch";

export interface VenuePart {
	id: string;
	label: string;
	profileId: string;
	/** A second line under the label, such as a deck's leg height. */
	detail?: string;
	/**
	 * What the part is placed with beyond its profile, such as the sides a flight of stairs carries
	 * handrails on. Parts that share a profile are told apart by their `key`.
	 */
	sceneryOptions?: NonNullable<PatchFixtureWrite["sceneryOptions"]>;
	/** The size the part is placed at in metres, when it is not its profile's default. */
	sizeMetres?: { x: number; y: number; z: number };
	/** How the part is chosen and remembered when its profile alone does not say; else its profile. */
	key?: string;
}

/** How a part is chosen and remembered: its own key, or its profile when that alone names it. */
export function partKey(part: VenuePart): string {
	return part.key ?? part.profileId;
}

/** The one flight of stairs, placed with the handrails chosen for it. */
const STAIRS_PROFILE_ID = "d5982d33-9723-5749-ade6-7be0e6b4adf1";

/**
 * Profiles the add buttons and the venue element dialog no longer offer, because one generated part
 * now places what they did: the flight of stairs made with handrails is the one Stairs with rails on
 * both sides, a fixed-width curtain is the parametric curtain at that width, and the racks, PA tops
 * and line array modelled at one size are the generated Flight Rack, PA Speaker and Line Array. A
 * show that placed one still draws it; it is just not offered again.
 */
const RETIRED_PART_PROFILE_IDS: ReadonlySet<string> = new Set([
	"47662838-33b1-5fcc-9323-bb5840c1783f",
	// The curtains made at one fixed width: the parametric curtain is any of them.
	"6c1cd6ef-d230-5afd-974c-4d18698b81a2",
	"db730b89-b3f0-5920-8c79-177193785459",
	"2def7a39-ebb2-5028-9f81-1f2d916597fe",
	"69cd8d74-d92b-5b9e-b004-bb179d95a5e1",
	"b2e36256-402c-5fc1-bab8-fb63c329ceb6",
	// The racks modelled at one height: the generated flight rack holds any number of units.
	"b642001a-9ff2-574f-9c05-b1a1696af660",
	"441aa8d0-d1a1-56bd-9415-4469fafea459",
	"fac71b58-3ca7-527c-ba80-4e5ce70bfcfa",
	"93bdb44a-ad3b-5e10-9bab-a2a1188d0a3b",
	"a1e077f0-fe95-5b35-a0f4-5335bc1e6132",
	"7bba0182-5046-5b15-b6c5-93eec8cbc65b",
	// The disco ball modelled at 50 cm: the generated Disco Ball is any diameter on any chain.
	"6dd53026-195e-4224-939b-352615b3bce9",
	// The PA modelled with and without its pole, and the line array modelled at one length: the
	// generated PA Speaker and Line Array are set to either in Info.
	"847d02b1-a0c5-5fb2-ab28-61898823542d",
	"bed75682-8441-5233-a9a5-7c6449ffce8f",
	"5354c05e-266a-521c-9200-e4671a6b30a2",
]);

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
			{ id: "2x1", label: "2 × 1 m", profileId: "ae45dcb3-cd94-59db-b3b1-0e8a5adb9141" },
			{ id: "1x1", label: "1 × 1 m", profileId: "b833bc89-b320-58df-946a-cd4728bd6421" },
			{ id: "1x0.5", label: "1 × 0.5 m", profileId: "b0eb846a-c813-5596-bd2e-3f741f8437df" },
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
		// One flight of stairs: its handrails are chosen in Info once it is placed, not from the menu.
		id: "stairs",
		label: "Stairs",
		partsLabel: "Part",
		parts: [
			{
				id: "stairs",
				label: "Stairs",
				detail: "Handrails are chosen in Info",
				profileId: STAIRS_PROFILE_ID,
			},
		],
	},
	{
		id: "handrail",
		label: "Handrail",
		partsLabel: "Part",
		parts: [
			{
				id: "handrail",
				label: "Handrail",
				detail: "Any length, 1 m high",
				profileId: "4397aea8-6e20-520c-aca4-a67de52bee92",
			},
		],
	},
];

/** The parametric curtain: placed at any width, then sized in Info. */
export const PARAMETRIC_CURTAIN_PROFILE_ID = "6f34b81e-3f71-5d35-b8fb-b4b0b7cce859";

/** The generated flight rack, placed at 8 U and 0.6 m deep and sized in Info. */
export const FLIGHT_RACK_PROFILE_ID = "448af0db-7419-557e-a621-26eadcd05eed";

/** The rack units a flight rack is placed with, before Info sets its own. */
export const FLIGHT_RACK_DEFAULT_UNITS = 8;

/** A rack case's height for the 19-inch units it holds, as `push_flight_rack` reads it back. */
export function rackHeightMetres(units: number): number {
	return 0.12 + 0.04445 * units;
}

/**
 * The scenic elements the stage is dressed with, together: the parametric curtain (a fixed-width
 * drape is just this at another width), the chain, the disco ball, the stage railing and the
 * generated flight rack, whose rack units are set in Info.
 */
export const SCENIC_TYPES: readonly VenuePartGroup[] = [
	{
		id: "curtain",
		label: "Curtain",
		partsLabel: "Curtain",
		parts: [
			{
				id: "parametric",
				label: "curtain",
				detail: "Any width, sized in Info",
				profileId: PARAMETRIC_CURTAIN_PROFILE_ID,
			},
		],
	},
	...[
		["chain", "Chain", "30e46e3d-69c6-5233-8acc-da3835abc316", "Rigging, any length"],
		["disco-ball", "Disco ball", "76579462-8afa-5ce8-a047-b543fab61538", "Any diameter, sized in Info"],
		["railing", "Stage railing", "9fc82162-c31c-4a34-bb2c-01fcc2254e37", "2 m"],
	].map(([id, label, profileId, detail]) => ({
		id,
		label,
		partsLabel: "Part",
		parts: [{ id, label: label.toLowerCase(), detail, profileId }],
	})),
	{
		// One flight rack: its rack units and depth are set in Info once it is placed, not chosen here.
		id: "flight-rack",
		label: "Flight rack",
		partsLabel: "Part",
		parts: [
			{
				id: "flight-rack",
				label: "flight rack",
				detail: "Units and depth are set in Info",
				profileId: FLIGHT_RACK_PROFILE_ID,
				sizeMetres: { x: 0.6, y: rackHeightMetres(FLIGHT_RACK_DEFAULT_UNITS), z: 0.6 },
			},
		],
	},
];

/**
 * The primitive shapes: each one parametric profile, placed at once from its tile and sized in Info.
 * Each fills its width, height and depth; a cylinder stands upright.
 */
export const PRIMITIVE_TYPES: readonly VenuePartGroup[] = [
	["box", "Box", "0087038f-6a2f-5d74-9185-8d14d7e1ee48"],
	["cylinder", "Cylinder", "a692c6db-7456-5b70-b681-50af57db2c28"],
	["ball", "Ball", "269ae83e-4ea8-5639-9d34-418fc8a08d23"],
].map(([id, label, profileId]) => ({
	id,
	label,
	partsLabel: "Shape",
	parts: [{ id, label: label.toLowerCase(), detail: "Sized in Info", profileId }],
}));

/** The title buttons that add a chosen part, each with a caret choosing which part it adds. */
export type CadPartKind = "truss" | "stage" | "curtain" | "primitive";

export const CAD_PART_CATALOGUE: Readonly<Record<CadPartKind, readonly VenuePartGroup[]>> = {
	truss: TRUSS_TYPES,
	stage: STAGE_TYPES,
	curtain: SCENIC_TYPES,
	primitive: PRIMITIVE_TYPES,
};

/** What each button adds until the operator chooses otherwise. */
export const DEFAULT_PART_PROFILE_IDS: Readonly<Record<CadPartKind, string>> = {
	truss: "44097b39-11b4-5bd4-af61-8adb97d426b1", // 3-point regular, straight
	stage: "6ad1c9f3-5024-543e-8478-f34ae63d338b", // 2 × 1 m on scissor feet
	curtain: PARAMETRIC_CURTAIN_PROFILE_ID,
	primitive: "0087038f-6a2f-5d74-9185-8d14d7e1ee48", // box
};

export interface FoundPart {
	group: VenuePartGroup;
	part: VenuePart;
}

/** Every profile one of the add buttons already offers or has retired, so no other list repeats it. */
export const PART_MENU_PROFILE_IDS: ReadonlySet<string> = new Set([
	...Object.values(CAD_PART_CATALOGUE).flatMap((groups) =>
		groups.flatMap((group) => group.parts.map((part) => part.profileId)),
	),
	...RETIRED_PART_PROFILE_IDS,
]);

/**
 * Whether a part is placed many at once with **Place Multiple**: every truss, laid as a run, and
 * every stage element, laid as a grid — but not the stairs or handrail the stage menu also lists,
 * which are placed one at a time.
 */
export function placesMultiple(kind: CadPartKind, group: VenuePartGroup): boolean {
	return kind === "truss" || (kind === "stage" && group.id !== "stairs" && group.id !== "handrail");
}

/** A button's part by its key, or undefined when the button does not offer that part. */
export function findPart(kind: CadPartKind, key: string): FoundPart | undefined {
	// A flight chosen with its handrails, remembered before the handrails moved to Info, is the one
	// Stairs part now.
	key = key.replace(/^(.*):handrails-(?:none|left|right|both)$/u, "$1");
	// So is a flight rack remembered at one of the rack-unit sizes the menu used to list.
	key = key.replace(/^(.*):units-\d+$/u, "$1");
	const groups = CAD_PART_CATALOGUE[kind];
	// A key names one part; a bare profile, as a choice remembered before keys existed, its first.
	for (const matches of [(part: VenuePart) => partKey(part) === key, (part: VenuePart) => part.profileId === key])
		for (const group of groups) {
			const part = group.parts.find(matches);
			if (part) return { group, part };
		}
	return undefined;
}

/** How a message names a part: a group's only part by itself, any other with its group. */
export function partLabel({ group, part }: FoundPart): string {
	return group.parts.length === 1 ? part.label : `${part.label} (${group.label})`;
}

/**
 * Whether a library profile is a Venue object: the shipped Venue manufacturer, or any profile that
 * is placed but never patched to DMX, such as an imported venue model.
 */
export function isVenueDefinition(definition: FixtureDefinition): boolean {
	const profile = definition.profile_snapshot;
	return (
		(profile?.manufacturer ?? definition.manufacturer) === "Venue" ||
		profile?.patch_policy === "visual_only"
	);
}

export interface VenueProfile {
	profileId: string;
	definition: FixtureDefinition;
}

/**
 * Every Venue profile the add buttons do not already offer, once, at its newest revision, in name
 * order: the trusses, decks, curtains and primitives belong to their own buttons and their part
 * menus, so this list holds what only it can place — railings, crowds, mirror balls, chain, PA and
 * backline, figures, and imported venue models.
 */
export function venueProfiles(definitions: readonly FixtureDefinition[]): VenueProfile[] {
	const newest = new Map<string, FixtureDefinition>();
	for (const definition of definitions) {
		if (!isVenueDefinition(definition)) continue;
		const profileId = definition.profile_snapshot?.id ?? definition.id;
		if (PART_MENU_PROFILE_IDS.has(profileId)) continue;
		const current = newest.get(profileId);
		if (!current || definition.revision > current.revision) newest.set(profileId, definition);
	}
	return [...newest]
		.map(([profileId, definition]) => ({ profileId, definition }))
		.sort((a, b) => a.definition.name.localeCompare(b.definition.name));
}

/** Whether a Venue profile matches what the operator typed, by name, short name or type. */
export function matchesVenueQuery(definition: FixtureDefinition, query: string): boolean {
	const wanted = query.trim().toLowerCase();
	if (!wanted) return true;
	const profile = definition.profile_snapshot;
	return [definition.name, profile?.short_name, profile?.fixture_type].some((text) =>
		text?.toLowerCase().includes(wanted),
	);
}

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
