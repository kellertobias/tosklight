import type { FixtureProfileScenery } from "../../fixtureProfile";
import type { PatchedFixture } from "../../wire";

export type SceneryAxis = "width" | "height" | "depth";

/** The three measurements a generated Venue object can be made to, and the edits that set them. */
export const SCENERY_AXES = [
	{ axis: "width", edit: "scenery_width", key: "x", label: "Width" },
	{ axis: "height", edit: "scenery_height", key: "y", label: "Height" },
	{ axis: "depth", edit: "scenery_depth", key: "z", label: "Depth" },
] as const;

export type SceneryEdit = (typeof SCENERY_AXES)[number]["edit"];

export function sceneryAxisOf(edit: string | null | undefined) {
	return SCENERY_AXES.find((entry) => entry.edit === edit);
}

/** The shape a fixture is generated from, if it is generated at all. */
export function sceneryOf(fixture: PatchedFixture): FixtureProfileScenery | null {
	return fixture.definition.profile_snapshot?.scenery ?? null;
}

/**
 * The size this one is placed at, in metres.
 *
 * The patch stores millimetres like every other measurement it carries; an object placed before
 * anyone set a size reads as the size its profile arrives at.
 */
export function placedSceneryMetres(
	fixture: PatchedFixture,
	scenery: FixtureProfileScenery,
) {
	const stored = fixture.scenery_size_metres;
	const fallback = scenery.default_size_metres;
	const axis = (key: "x" | "y" | "z") =>
		stored && Number.isFinite(stored[key]) && stored[key] > 0
			? stored[key] / 1_000
			: fallback[key];
	return { x: axis("x"), y: axis("y"), z: axis("z") };
}

/**
 * The stored size after setting one measurement, or the reason it is refused.
 *
 * A measurement outside what the profile allows is refused with that range rather than clamped, so
 * a typed 40 m truss never quietly becomes a 24 m one.
 */
export function sceneryMeasurement(
	fixture: PatchedFixture,
	edit: SceneryEdit,
	value: string,
):
	| { size: { x: number; y: number; z: number } }
	| { error: string }
	| null {
	const scenery = sceneryOf(fixture);
	const axis = sceneryAxisOf(edit);
	if (!scenery || !axis) return null;
	const parsed = Number(value);
	const low = scenery.minimum_size_metres[axis.key];
	const high = scenery.maximum_size_metres[axis.key];
	if (!value.trim() || !Number.isFinite(parsed) || parsed < low || parsed > high)
		return {
			error: `Enter a ${axis.label.toLowerCase()} from ${low} to ${high} metres.`,
		};
	const placed = { ...placedSceneryMetres(fixture, scenery), [axis.key]: parsed };
	return {
		size: {
			x: Math.round(placed.x * 1_000),
			y: Math.round(placed.y * 1_000),
			z: Math.round(placed.z * 1_000),
		},
	};
}
