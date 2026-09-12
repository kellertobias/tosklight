import type { FixtureProfileScenery } from "@tosklight/patch";
import type { PatchedFixture } from "../../../api/types";

export type SceneryAxis = "width" | "height" | "depth";

export const SCENERY_AXES = [
	{ axis: "width", edit: "scenery_width", key: "x", label: "Width" },
	{ axis: "height", edit: "scenery_height", key: "y", label: "Height" },
	{ axis: "depth", edit: "scenery_depth", key: "z", label: "Depth" },
] as const;

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
