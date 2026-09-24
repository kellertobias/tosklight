/**
 * The measurements of a generated Venue object, and how one is read off a placed element.
 *
 * A profile says which of its three dimensions the operator sets and the range each may take; the
 * patch stores what was chosen under the axis keys, and an element left alone stores nothing at all
 * and stands at its profile's default. Info reads a size the same way for one element and for a
 * whole selection of the same model, so the reading lives here rather than in either panel.
 */
import type { FixtureProfile, FixtureProfileScenery, PatchFixtureWrite } from "@tosklight/patch";
import {
	lineArrayElements,
	lineArrayHeight,
	PA_CABINET,
	paPole,
	rackHeight,
	rackUnits,
} from "./equipmentPlan";

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

/**
 * One measurement Info offers for a generated object, typed the way the object is bought.
 *
 * Most are a side in metres. Equipment reads its one count off its height instead: a flight rack
 * by the rack units it holds, a line array by its elements, and a PA speaker by the pole under its
 * cabinet (0 when it stands on the cabinet alone). Each still stores the height, so the patch, the
 * desk and the Visualizer keep one size for it.
 */
export interface SizeMeasure {
	id: string;
	label: string;
	unit: string;
	digits: number;
	min: number;
	max: number;
	read(size: Record<SizeAxisKey, number>): number;
	/** The size with this measurement set, in metres. */
	write(size: Record<SizeAxisKey, number>, value: number): Record<SizeAxisKey, number>;
}

/** The measurements Info offers for a generated object, in the order it lists them. */
export function sizeMeasures(scenery: FixtureProfileScenery): SizeMeasure[] {
	return SIZE_AXES.filter(({ axis }) => scenery.adjustable[axis]).map(({ key, label }) => {
		const low = scenery.minimum_size_metres[key];
		const high = scenery.maximum_size_metres[key];
		const count =
			key === "y" && (scenery.kind === "flight_rack" || scenery.kind === "line_array")
				? scenery.kind === "flight_rack"
					? { label: "Units", unit: "U", of: rackUnits, height: rackHeight }
					: { label: "Elements", unit: "", of: lineArrayElements, height: lineArrayHeight }
				: null;
		if (count)
			return {
				id: `size-${key}`,
				label: count.label,
				unit: count.unit,
				digits: 0,
				min: count.of(low * 1000),
				max: count.of(high * 1000),
				read: (size) => count.of(size.y * 1000),
				write: (size, value) => ({ ...size, y: count.height(Math.round(value)) / 1000 }),
			};
		// A disco ball is as wide and deep as the ball, and as tall as the ball and its chain: its
		// diameter keeps the chain it hangs on, and its chain is the height above the ball.
		if (scenery.kind === "mirror_ball" && key === "x")
			return {
				id: "size-x",
				label: "Diameter",
				unit: "m",
				digits: 2,
				min: low,
				max: high,
				read: (size) => size.x,
				write: (size, diameter) => {
					const chain = Math.max(0, size.y - size.x);
					const next = Math.min(Math.max(diameter, low), high);
					return { x: next, y: next + chain, z: next };
				},
			};
		if (scenery.kind === "mirror_ball" && key === "y")
			return {
				id: "size-y",
				label: "Chain",
				unit: "m",
				digits: 2,
				min: 0,
				max: Math.max(0, high - scenery.maximum_size_metres.x),
				read: (size) => Math.max(0, size.y - size.x),
				write: (size, chain) => ({ ...size, y: size.x + Math.max(0, chain) }),
			};
		if (key === "y" && scenery.kind === "pa_top")
			return {
				id: "size-y",
				label: "Pole",
				unit: "m",
				digits: 2,
				min: 0,
				max: Math.max(0, high - PA_CABINET / 1000),
				read: (size) => paPole(size.y * 1000) / 1000,
				write: (size, pole) => ({ ...size, y: PA_CABINET / 1000 + (pole >= 0.05 ? pole : 0) }),
			};
		return {
			id: `size-${key}`,
			label,
			unit: "m",
			digits: 3,
			min: low,
			max: high,
			read: (size) => size[key],
			write: (size, metres) => ({ ...size, [key]: clampToRange(scenery, key, metres) }),
		};
	});
}

/** A size in metres as the patch stores it, in whole millimetres. */
export function storedSize(size: Record<SizeAxisKey, number>): { x: number; y: number; z: number } {
	return { x: Math.round(size.x * 1000), y: Math.round(size.y * 1000), z: Math.round(size.z * 1000) };
}
