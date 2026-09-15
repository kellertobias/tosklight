/**
 * Adding and listing fixtures, and the tools that size and dress Venue objects.
 *
 * Venue objects are fixtures too — they live in the same patch and are removed, moved and layered
 * with the same tools — so adding one is `add_fixture`, not a separate path. What differs is their
 * number (a visual-only object takes a `0.N` number and no DMX address) and what they carry beyond
 * a placement: a size, a colour, chain end fittings and a model scale.
 */

import {
	fixtureLabel,
	type PatchBackend,
	type PatchedFixture,
	parseFixtureRef,
	findFixture,
} from "./backend";
import { appearanceWithGel, fixtureNumber, gel, type Tool } from "./toolSchema";
import {
	CHAIN_BOTTOM_ENDS,
	CHAIN_MODES,
	CHAIN_TOP_ENDS,
	checkedModelScale,
	editedSceneryOptions,
	nextFixtureNumber,
	nextVirtualFixtureNumber,
	placedMetres,
	profileOf,
	resolveProfile,
	type Scenery,
	sceneryOf,
	sizedMillimetres,
	venueSummary,
} from "./venue";

const metres = (label: string) => ({ type: "number", description: `${label}, in metres.` });

export const venueInputs = {
	size_metres: {
		type: "object",
		description:
			"A Venue object's size in metres. Only the measurements its profile makes adjustable can be set, within the profile's minimum and maximum.",
		properties: {
			width: metres("Width"),
			height: metres("Height"),
			depth: metres("Depth"),
		},
	},
	colour: {
		type: ["string", "null"],
		description: "A Venue object's colour as #RRGGBB. Null keeps its kind's own material.",
	},
	chain_mode: {
		type: "string",
		enum: Object.keys(CHAIN_MODES),
		description: "How a chain is rigged; sets both ends the way the patch sheet does.",
	},
	chain_top: { type: "string", enum: CHAIN_TOP_ENDS },
	chain_bottom: { type: "string", enum: CHAIN_BOTTOM_ENDS },
	model_scale: {
		type: ["number", "null"],
		description: "How many times its built size the object is drawn, 0.01 to 100. Null is 1.",
	},
};

const hasVenueInput = (input: Record<string, any>) =>
	["size_metres", "colour", "chain_mode", "chain_top", "chain_bottom"].some(
		(field) => input[field] !== undefined,
	);

/** The number a new fixture stands at, following the numbering its profile's patch policy uses. */
function numberFor(
	snapshot: Awaited<ReturnType<PatchBackend["patch"]>>,
	profile: Record<string, any>,
	input: Record<string, any>,
) {
	const parsed = input.fixture_number !== undefined ? parseFixtureRef(input.fixture_number) : null;
	if (profile.patch_policy === "visual_only") {
		if (input.universe != null || input.address != null)
			throw new Error(`${profile.name} is a visual-only Venue object and takes no DMX address`);
		if (parsed?.fixture_number !== undefined)
			throw new Error(
				`${profile.name} is a visual-only Venue object; it takes a 0.N number, not ${parsed.fixture_number}`,
			);
		return {
			fixture_number: null,
			virtual_fixture_number: parsed?.virtual_fixture_number ?? nextVirtualFixtureNumber(snapshot),
		};
	}
	if (parsed?.virtual_fixture_number !== undefined)
		throw new Error(`only a visual-only Venue object takes a 0.N number; ${profile.name} is patchable`);
	return {
		fixture_number: parsed?.fixture_number ?? nextFixtureNumber(snapshot),
		virtual_fixture_number: null,
	};
}

export async function addFixture(desk: PatchBackend, input: Record<string, any>) {
	const [snapshot, { profiles }] = await Promise.all([desk.patch(), desk.profiles()]);
	const { profile, mode } = resolveProfile(profiles, input);
	const scenery = sceneryOf(profile);
	const numbers = numberFor(snapshot, profile, input);
	const label = fixtureLabel(numbers) as string;
	const taken = findFixture(snapshot, label);
	if (taken) throw new Error(`fixture ${label} is already in use by ${taken.name}`);
	if (!scenery && hasVenueInput(input))
		throw new Error(`${profile.name} is not a Venue object; it has no size, colour or chain ends`);
	const fixture = {
		fixture_id: crypto.randomUUID(),
		...numbers,
		name: input.name ?? profile.name,
		note: input.note ?? null,
		profile_id: profile.id,
		profile_revision: profile.revision,
		mode_id: mode.id,
		split_patches: [
			{ split: 1, universe: input.universe ?? null, address: input.address ?? null },
		],
		layer_id: input.layer_id ?? "default",
		direct_control: null,
		location: { x: input.x ?? 0, y: input.y ?? 0, z: input.z ?? 0 },
		rotation: {
			x: input.rotation_x ?? 0,
			y: input.rotation_y ?? 0,
			z: input.rotation_z ?? 0,
		},
		multipatch: [],
		move_in_black_enabled: false,
		move_in_black_delay_millis: 0,
		highlight_overrides: [],
	} as unknown as PatchedFixture;
	if (input.gel) fixture.installed_appearance = appearanceWithGel(fixture, input);
	if (scenery && input.size_metres)
		fixture.scenery_size_metres = sizedMillimetres(
			scenery,
			scenery.default_size_metres,
			input.size_metres,
		);
	const options = scenery ? editedSceneryOptions(null, input, scenery) : null;
	if (options) fixture.scenery_options = options;
	if (input.model_scale !== undefined) fixture.model_scale = checkedModelScale(input.model_scale);
	await desk.putFixtures(snapshot.patch_revision, [fixture]);
	return {
		id: label,
		...numbers,
		name: fixture.name,
		profile: `${profile.manufacturer} ${profile.name}`,
		profile_id: profile.id,
		profile_revision: profile.revision,
		mode: mode.name,
		added: true,
		model_scale: fixture.model_scale ?? null,
		venue: scenery ? venueSummary(fixture, scenery) : null,
	};
}

export async function listFixtures(desk: PatchBackend) {
	const [{ fixtures }, library] = await Promise.all([
		desk.patch(),
		// The list is still worth having when the library cannot be read; it only loses the names.
		desk.profiles().catch(() => ({ profiles: [] as Array<Record<string, unknown>> })),
	]);
	return fixtures.map((fixture) => {
		const profile = profileOf(library.profiles, fixture);
		const scenery = sceneryOf(profile);
		return {
			id: fixtureLabel(fixture),
			fixture_number: fixture.fixture_number,
			virtual_fixture_number: fixture.virtual_fixture_number ?? null,
			name: fixture.name,
			note: fixture.note ?? null,
			profile: profile ? `${profile.manufacturer} ${profile.name}` : null,
			layer_id: fixture.layer_id,
			location: fixture.location,
			rotation: fixture.rotation,
			bracket_angle: fixture.bracket_angle,
			shaper_angle: fixture.shaper_angle,
			invert_pan: fixture.invert_pan,
			invert_tilt: fixture.invert_tilt,
			position_master: fixture.position_master ?? null,
			split_patches: fixture.split_patches,
			multipatch: (fixture.multipatch as unknown[])?.length ?? 0,
			model_scale: fixture.model_scale ?? null,
			venue: scenery ? venueSummary(fixture, scenery) : null,
		};
	});
}

/** Edit one Venue object, refusing a fixture that is not one before anything is written. */
async function editVenue(
	desk: PatchBackend,
	ref: string | number,
	change: (fixture: PatchedFixture, scenery: Scenery) => void,
) {
	const { profiles } = await desk.profiles();
	const found: { scenery?: Scenery } = {};
	const edited = await desk.editFixture(ref, (fixture) => {
		const scenery = sceneryOf(profileOf(profiles, fixture));
		if (!scenery) throw new Error(`fixture ${ref} (${fixture.name}) is not a Venue object`);
		found.scenery = scenery;
		change(fixture, scenery);
		return fixture;
	});
	return { id: fixtureLabel(edited), ...venueSummary(edited, found.scenery as Scenery) };
}

export const venueTools: Tool[] = [
	{
		name: "set_venue_size",
		description:
			"Set a Venue object's size in metres — a curtain's width and height, a truss's length, a chain's drop. Only the measurements its profile makes adjustable can be set; a size outside the profile's range is refused with that range. Omitted measurements are left alone.",
		inputSchema: {
			type: "object",
			properties: {
				fixture_number: fixtureNumber,
				width: metres("Width"),
				height: metres("Height"),
				depth: metres("Depth"),
			},
			required: ["fixture_number"],
		},
		run: (desk, input) =>
			editVenue(desk, input.fixture_number, (fixture, scenery) => {
				fixture.scenery_size_metres = sizedMillimetres(
					scenery,
					placedMetres(fixture, scenery),
					input,
				);
			}),
	},
	{
		name: "set_venue_options",
		description:
			"Set a Venue object's colour, and a chain's end fittings. Colour null gives the object its kind's own material back. Omitted options are left alone.",
		inputSchema: {
			type: "object",
			properties: {
				fixture_number: fixtureNumber,
				colour: venueInputs.colour,
				chain_mode: venueInputs.chain_mode,
				chain_top: venueInputs.chain_top,
				chain_bottom: venueInputs.chain_bottom,
			},
			required: ["fixture_number"],
		},
		run: (desk, input) =>
			editVenue(desk, input.fixture_number, (fixture, scenery) => {
				fixture.scenery_options = editedSceneryOptions(
					fixture.scenery_options as Record<string, unknown> | null,
					input,
					scenery,
				);
			}),
	},
	{
		name: "set_model_scale",
		description:
			"Draw a placed object at a multiple of the size it was built at, from 0.01 to 100. Null draws it at its built size.",
		inputSchema: {
			type: "object",
			properties: { fixture_number: fixtureNumber, model_scale: venueInputs.model_scale },
			required: ["fixture_number", "model_scale"],
		},
		async run(desk, input) {
			const scale = checkedModelScale(input.model_scale);
			const edited = await desk.editFixture(input.fixture_number, (fixture) => {
				fixture.model_scale = scale;
				return fixture;
			});
			return { id: fixtureLabel(edited), model_scale: edited.model_scale ?? null };
		},
	},
];

/** The add_fixture input schema, kept here beside the code that reads it. */
export const addFixtureSchema: Tool["inputSchema"] = {
	type: "object",
	properties: {
		profile_name: {
			type: "string",
			description:
				"The profile's name, e.g. \"Curtain\". Resolves to its newest revision. Use this or profile_id.",
		},
		manufacturer: { type: "string", description: "Narrows profile_name when two makers share it." },
		profile_id: { type: "string" },
		profile_revision: { type: "number", description: "Defaults to the newest revision." },
		mode_id: { type: "string" },
		mode_name: { type: "string", description: "Defaults to the profile's first mode." },
		fixture_number: {
			...fixtureNumber,
			description:
				"Omit for the next free number. A visual-only Venue object takes a 0.N number; a patchable fixture a positive one.",
		},
		name: { type: "string", description: "Defaults to the profile's name." },
		note: { type: "string" },
		layer_id: { type: "string", description: "Defaults to `default`." },
		universe: { type: "number", description: "Not for a visual-only Venue object." },
		address: { type: "number" },
		x: { type: "number", description: "Millimetres across the stage." },
		y: { type: "number", description: "Millimetres upstage." },
		z: { type: "number", description: "Millimetres up." },
		rotation_x: { type: "number" },
		rotation_y: { type: "number" },
		rotation_z: { type: "number" },
		gel,
		...venueInputs,
	},
};
