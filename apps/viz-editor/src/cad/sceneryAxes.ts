/**
 * The measurements of a generated Venue object, and how one is read off a placed element.
 *
 * A profile says which of its three dimensions the operator sets and the range each may take; the
 * patch stores what was chosen under the axis keys, and an element left alone stores nothing at all
 * and stands at its profile's default. Info reads a size the same way for one element and for a
 * whole selection of the same model, so the reading lives here rather than in either panel.
 */
import type { FixtureProfileScenery, PatchFixtureWrite } from "@tosklight/patch";

/** The measurements of a generated Venue object, with the key the patch stores each under. */
export const SIZE_AXES = [
	{ axis: "width", key: "x", label: "Width" },
	{ axis: "height", key: "y", label: "Height" },
	{ axis: "depth", key: "z", label: "Depth" },
] as const;

/** The key one measurement is stored under: `x`, `y` or `z`. */
export type SizeAxisKey = (typeof SIZE_AXES)[number]["key"];

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
