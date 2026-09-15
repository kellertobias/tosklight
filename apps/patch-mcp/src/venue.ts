/**
 * Venue objects — curtains, truss, risers, chains — as the tools place and size them.
 *
 * A Venue object is a fixture whose profile is generated from a `scenery` description rather than
 * from a DMX chart. Its profile says which of its measurements an operator may set and within what
 * bounds; the patch stores the placed size in millimetres like every other measurement it carries.
 * The rules here are the patch sheet's own, so a size the sheet would refuse is refused here with
 * the same range instead of being clamped somewhere downstream.
 */

import type { PatchSnapshot, PatchedFixture } from "./backend";

type Profile = Record<string, any>;

export interface Scenery {
	kind: string;
	default_size_metres: { x: number; y: number; z: number };
	minimum_size_metres: { x: number; y: number; z: number };
	maximum_size_metres: { x: number; y: number; z: number };
	adjustable?: { width?: boolean; height?: boolean; depth?: boolean };
}

export const SCENERY_AXES = [
	{ axis: "width", key: "x" },
	{ axis: "height", key: "y" },
	{ axis: "depth", key: "z" },
] as const;

export const CHAIN_TOP_ENDS = ["motor", "direct", "steelflex_loop"];
export const CHAIN_BOTTOM_ENDS = ["direct", "steelflex_loop", "motor"];
/** How the patch sheet names a chain's rigging, and the end fittings each stores. */
export const CHAIN_MODES: Record<string, { top: string; bottom: string }> = {
	plain: { top: "direct", bottom: "direct" },
	motor_top: { top: "motor", bottom: "steelflex_loop" },
	motor_bottom: { top: "steelflex_loop", bottom: "motor" },
};
export const MIN_MODEL_SCALE = 0.01;
export const MAX_MODEL_SCALE = 100;

const lower = (value: unknown) => String(value ?? "").trim().toLowerCase();

/**
 * The profile and mode an `add_fixture` call means.
 *
 * Ids win when they are given. Otherwise the profile is found by name — and manufacturer, when two
 * makers use the same name — at its newest revision, in the mode asked for or its first one.
 */
export function resolveProfile(
	profiles: Profile[],
	input: Record<string, any>,
): { profile: Profile; mode: Profile } {
	let candidates: Profile[];
	if (input.profile_id) {
		candidates = profiles.filter((profile) => profile.id === input.profile_id);
		if (candidates.length === 0)
			throw new Error(`no profile with id ${input.profile_id} in the fixture library`);
		const exact = candidates.find(
			(profile) => profile.revision === input.profile_revision,
		);
		if (input.profile_revision !== undefined && !exact)
			throw new Error(
				`profile ${input.profile_id} has no revision ${input.profile_revision}; it has ${candidates.map((profile) => profile.revision).join(", ")}`,
			);
		if (exact) candidates = [exact];
	} else if (input.profile_name) {
		candidates = profiles.filter(
			(profile) =>
				lower(profile.name) === lower(input.profile_name) &&
				(!input.manufacturer || lower(profile.manufacturer) === lower(input.manufacturer)),
		);
		if (candidates.length === 0) {
			const near = profiles
				.filter((profile) => lower(profile.name).includes(lower(input.profile_name)))
				.slice(0, 8)
				.map((profile) => `${profile.manufacturer} ${profile.name}`);
			throw new Error(
				`no profile named ${input.profile_name}${input.manufacturer ? ` by ${input.manufacturer}` : ""}${near.length ? `; similar: ${[...new Set(near)].join(", ")}` : ""}`,
			);
		}
		const makers = new Set(candidates.map((profile) => profile.id));
		if (makers.size > 1)
			throw new Error(
				`${makers.size} profiles are named ${input.profile_name}: ${[...new Set(candidates.map((profile) => `${profile.manufacturer} ${profile.name} (${profile.id})`))].join(", ")}. Name the manufacturer or pass profile_id.`,
			);
	} else {
		throw new Error("name the profile with profile_name, or profile_id from search_fixture_library");
	}
	const profile = candidates.reduce((newest, candidate) =>
		candidate.revision > newest.revision ? candidate : newest,
	);
	const modes: Profile[] = profile.modes ?? [];
	const mode = input.mode_id
		? modes.find((candidate) => candidate.id === input.mode_id)
		: input.mode_name
			? modes.find((candidate) => lower(candidate.name) === lower(input.mode_name))
			: modes[0];
	if (!mode)
		throw new Error(
			`${profile.manufacturer} ${profile.name} has no mode ${input.mode_id ?? input.mode_name ?? ""}; modes: ${modes.map((candidate) => candidate.name).join(", ") || "none"}`,
		);
	return { profile, mode };
}

/** The profile a placed fixture was patched from: its own revision, or failing that the newest. */
export function profileOf(profiles: Profile[], fixture: PatchedFixture): Profile | undefined {
	const same = profiles.filter((profile) => profile.id === fixture.profile_id);
	return (
		same.find((profile) => profile.revision === fixture.profile_revision) ??
		same.sort((left, right) => right.revision - left.revision)[0]
	);
}

export function sceneryOf(profile: Profile | undefined): Scenery | null {
	return (profile?.scenery as Scenery | undefined) ?? null;
}

/** The size a Venue object stands at, in metres: what the patch stores, or the profile default. */
export function placedMetres(fixture: PatchedFixture, scenery: Scenery) {
	const stored = fixture.scenery_size_metres as
		| { x: number; y: number; z: number }
		| null
		| undefined;
	const axis = (key: "x" | "y" | "z") =>
		stored && Number.isFinite(stored[key]) && stored[key] > 0
			? stored[key] / 1_000
			: scenery.default_size_metres[key];
	return { x: axis("x"), y: axis("y"), z: axis("z") };
}

/**
 * The stored size, in millimetres, after setting some measurements.
 *
 * Only a measurement the profile makes adjustable can be set — a truss keeps its cross-section —
 * and one outside the profile's bounds is refused with them.
 */
export function sizedMillimetres(
	scenery: Scenery,
	placed: { x: number; y: number; z: number },
	requested: Record<string, unknown>,
) {
	const next = { ...placed };
	const adjustable = SCENERY_AXES.filter(({ axis }) => scenery.adjustable?.[axis]).map(
		({ axis }) => axis,
	);
	let changed = false;
	for (const { axis, key } of SCENERY_AXES) {
		const value = requested[axis];
		if (value === undefined || value === null) continue;
		if (!adjustable.includes(axis))
			throw new Error(
				`a ${scenery.kind}'s ${axis} is fixed by its profile; ${adjustable.length ? `only ${adjustable.join(", ")} can be set` : "none of its measurements can be set"}`,
			);
		const low = scenery.minimum_size_metres[key];
		const high = scenery.maximum_size_metres[key];
		const metres = Number(value);
		if (!Number.isFinite(metres) || metres < low || metres > high)
			throw new Error(
				`enter a ${axis} from ${low} to ${high} metres for this ${scenery.kind}; ${String(value)} is outside it`,
			);
		next[key] = metres;
		changed = true;
	}
	if (!changed)
		throw new Error(`give at least one of: ${adjustable.join(", ") || "(nothing is adjustable)"}`);
	return {
		x: Math.round(next.x * 1_000),
		y: Math.round(next.y * 1_000),
		z: Math.round(next.z * 1_000),
	};
}

/**
 * A Venue object's colour and chain ends after an edit, or null when nothing is chosen.
 *
 * `colour: null` gives the object its kind's own material back. Chain ends only mean something on a
 * chain, so asking for them on a curtain is refused rather than stored and ignored.
 */
export function editedSceneryOptions(
	current: Record<string, unknown> | null | undefined,
	input: Record<string, any>,
	scenery: Scenery,
): Record<string, unknown> | null {
	const options: Record<string, unknown> = { ...(current ?? {}) };
	if ("colour" in input) {
		if (input.colour === null) delete options.colour_srgb;
		else if (typeof input.colour === "string" && /^#[0-9a-fA-F]{6}$/.test(input.colour))
			options.colour_srgb = input.colour.toUpperCase();
		else throw new Error(`colour ${String(input.colour)} must be #RRGGBB, or null to reset it`);
	}
	const chain = input.chain_mode ?? input.chain_top ?? input.chain_bottom;
	if (chain !== undefined && scenery.kind !== "chain")
		throw new Error(`chain ends only apply to a chain; this is a ${scenery.kind}`);
	if (input.chain_mode !== undefined) {
		const mode = CHAIN_MODES[input.chain_mode];
		if (!mode)
			throw new Error(`chain_mode must be one of ${Object.keys(CHAIN_MODES).join(", ")}`);
		options.chain_top = mode.top;
		options.chain_bottom = mode.bottom;
	}
	for (const [field, allowed] of [
		["chain_top", CHAIN_TOP_ENDS],
		["chain_bottom", CHAIN_BOTTOM_ENDS],
	] as const) {
		if (input[field] === undefined) continue;
		if (!allowed.includes(input[field]))
			throw new Error(`${field} must be one of ${allowed.join(", ")}`);
		options[field] = input[field];
	}
	return Object.keys(options).length > 0 ? options : null;
}

export function checkedModelScale(value: unknown): number | null {
	if (value === null) return null;
	const scale = Number(value);
	if (!Number.isFinite(scale) || scale < MIN_MODEL_SCALE || scale > MAX_MODEL_SCALE)
		throw new Error(
			`model scale ${String(value)} must be from ${MIN_MODEL_SCALE} to ${MAX_MODEL_SCALE}, or null for the size it was built at`,
		);
	return scale;
}

/** What an agent needs to see about a placed Venue object. */
export function venueSummary(fixture: PatchedFixture, scenery: Scenery) {
	const placed = placedMetres(fixture, scenery);
	const options = (fixture.scenery_options as Record<string, unknown> | null) ?? {};
	return {
		kind: scenery.kind,
		size_metres: { width: placed.x, height: placed.y, depth: placed.z },
		adjustable: SCENERY_AXES.filter(({ axis }) => scenery.adjustable?.[axis]).map(
			({ axis }) => axis,
		),
		colour: options.colour_srgb ?? null,
		...(scenery.kind === "chain"
			? { chain_top: options.chain_top ?? null, chain_bottom: options.chain_bottom ?? null }
			: {}),
	};
}

export function nextFixtureNumber(snapshot: PatchSnapshot) {
	return (
		snapshot.fixtures.reduce(
			(highest, fixture) => Math.max(highest, fixture.fixture_number ?? 0),
			0,
		) + 1
	);
}

export function nextVirtualFixtureNumber(snapshot: PatchSnapshot) {
	return (
		snapshot.fixtures.reduce(
			(highest, fixture) => Math.max(highest, fixture.virtual_fixture_number ?? 0),
			0,
		) + 1
	);
}
