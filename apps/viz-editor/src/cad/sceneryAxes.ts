/**
 * The measurements of a generated Venue object, and how one is read off a placed element.
 *
 * A profile says which of its three dimensions the operator sets and the range each may take; the
 * patch stores what was chosen under the axis keys, and an element left alone stores nothing at all
 * and stands at its profile's default. Info reads a size the same way for one element and for a
 * whole selection of the same model, so the reading lives here rather than in either panel.
 */
import type { FixtureProfile, FixtureProfileScenery, PatchFixtureWrite } from "@tosklight/patch";

/** The measurements of a generated Venue object, with the key the patch stores each under. */
export const SIZE_AXES = [
	{ axis: "width", key: "x", label: "Width" },
	{ axis: "height", key: "y", label: "Height" },
	{ axis: "depth", key: "z", label: "Depth" },
] as const;

/** The key one measurement is stored under: `x`, `y` or `z`. */
export type SizeAxisKey = (typeof SIZE_AXES)[number]["key"];

/** The footprint a crowd area may be given, in metres on each side, as its profile validates it. */
const CROWD_SIDE_METRES = { minimum: 1, maximum: 250 };

/**
 * The measurements Info offers for a profile: a generated object's own, or a crowd area's footprint.
 *
 * A crowd area generates its people over the width and depth it is given, so those two are set like
 * a deck's; its height is the people's, never set. It is described with the same shape as generated
 * scenery (its `kind` is not read for sizing), so one element and a whole selection size it the
 * same way and the patch stores it where every other placed size lives.
 */
export function sizedScenery(
	profile: Pick<FixtureProfile, "scenery" | "crowd" | "physical"> | null | undefined,
): FixtureProfileScenery | null {
	if (profile?.scenery) return profile.scenery;
	const crowd = profile?.crowd;
	if (!crowd) return null;
	const height = (profile.physical?.height_millimetres ?? 1780) / 1000;
	return {
		kind: "prop",
		chords: 0,
		default_size_metres: { x: crowd.default_width_metres, y: height, z: crowd.default_depth_metres },
		adjustable: { width: true, height: false, depth: true },
		minimum_size_metres: { x: CROWD_SIDE_METRES.minimum, y: height, z: CROWD_SIDE_METRES.minimum },
		maximum_size_metres: { x: CROWD_SIDE_METRES.maximum, y: height, z: CROWD_SIDE_METRES.maximum },
	};
}

/** Whether a generated object has any measurement the operator sets. */
export function hasAdjustableSize(scenery: FixtureProfileScenery | null | undefined) {
	return Boolean(scenery && SIZE_AXES.some(({ axis }) => scenery.adjustable[axis]));
}

/** The size an object is placed at in metres: what the patch stores, else its profile's default. */
export function placedSize(
	fixture: Pick<PatchFixtureWrite, "scenerySizeMetres">,
	scenery: FixtureProfileScenery,
): Record<SizeAxisKey, number> {
	const stored = fixture.scenerySizeMetres;
	const axis = (key: SizeAxisKey) =>
		stored && Number.isFinite(stored[key]) && stored[key] > 0
			? stored[key] / 1000
			: scenery.default_size_metres[key];
	return { x: axis("x"), y: axis("y"), z: axis("z") };
}

/** One measurement held to the range its profile allows, so a spread cannot push it out of shape. */
export function clampToRange(
	scenery: FixtureProfileScenery,
	key: SizeAxisKey,
	metres: number,
): number {
	return Math.min(Math.max(metres, scenery.minimum_size_metres[key]), scenery.maximum_size_metres[key]);
}
