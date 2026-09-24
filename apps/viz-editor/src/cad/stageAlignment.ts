/**
 * Stage elements laid side by side: a dragged deck's sides butt against a neighbour's, or line up
 * flush with them, one plan axis at a time.
 *
 * Corner to corner is the special case where both axes fit at once; a deck slid along the front of
 * a wider one still butts against it and snaps, wherever along that side it is let go. Only decks
 * turned square to the plan (in quarter turns) are aligned this way, since their sides are then the
 * sides of their footprint.
 */
import type { FreeAxes } from "./snapping";
import type { CadEntity } from "./types";
import { addVec, isStageElement, stageCorners, type Vec3 } from "./venueShapes";

export interface StageAlignment {
	correction: Vec3;
	/** The aligned side on each axis that snapped: a line from one end of the joined run to the other. */
	guides: [Vec3, Vec3][];
}

interface Box {
	min: Vec3;
	max: Vec3;
}

interface AxisFit {
	gap: number;
	guide: [Vec3, Vec3];
}

/** Whether the element stands level and turned in whole quarter turns. */
function squareToPlan(entity: CadEntity): boolean {
	const [x, y, z] = entity.rotationDegrees;
	const quarter = (degrees: number) => Math.abs(degrees / 90 - Math.round(degrees / 90)) < 0.01;
	return Math.abs(x) < 0.5 && Math.abs(y) < 0.5 && quarter(z);
}

function box(entity: CadEntity, delta: Vec3): Box {
	const corners = stageCorners(entity).map((corner) => addVec(corner, delta));
	const on = (axis: number, pick: (...values: number[]) => number) =>
		pick(...corners.map((corner) => corner[axis]));
	return {
		min: [on(0, Math.min), on(1, Math.min), on(2, Math.min)],
		max: [on(0, Math.max), on(1, Math.max), on(2, Math.max)],
	};
}

/** How far apart two boxes are along an axis; zero or less where they overlap on it. */
function separation(a: Box, b: Box, axis: number): number {
	return Math.max(a.min[axis] - b.max[axis], b.min[axis] - a.max[axis]);
}

/** The best side-to-side fit of `own` against `other` along the horizontal `axis`, if one is in reach. */
function axisFit(own: Box, other: Box, axis: 0 | 1, along: number, threshold: number): AxisFit | null {
	// Only a neighbour lines up: one beside it across the other horizontal axis, touching or close.
	if (separation(own, other, 1 - axis) > threshold) return null;
	let best: { gap: number; face: number } | null = null;
	for (const [from, to] of [
		[own.max[axis], other.min[axis]],
		[own.min[axis], other.max[axis]],
		[own.min[axis], other.min[axis]],
		[own.max[axis], other.max[axis]],
	]) {
		const gap = to - from;
		if (Math.abs(gap) <= threshold && (!best || Math.abs(gap) < Math.abs(best.gap)))
			best = { gap, face: to };
	}
	if (!best) return null;
	// The guide runs along the joined side, across both elements, at the higher of their tops.
	const start: Vec3 = [0, 0, 0];
	const end: Vec3 = [0, 0, 0];
	start[axis] = end[axis] = best.face;
	const top = Math.max(own.max[2], other.max[2]);
	start[along] = Math.min(own.min[along], other.min[along]);
	end[along] = Math.max(own.max[along], other.max[along]);
	if (along !== 2) start[2] = end[2] = top;
	return { gap: best.gap, guide: [start, end] };
}

/**
 * The nearest side-by-side fit on each free plan axis for the stage elements that move, against the
 * ones that stay; null when no side is in reach.
 */
export function nearestStageAlignment(
	movers: readonly CadEntity[],
	still: readonly CadEntity[],
	delta: Vec3,
	free: FreeAxes,
	threshold: number,
): StageAlignment | null {
	const square = (entities: readonly CadEntity[]) =>
		entities.filter((entity) => isStageElement(entity) && squareToPlan(entity));
	const moving = square(movers);
	const others = square(still).map((entity) => box(entity, [0, 0, 0]));
	if (!moving.length || !others.length) return null;
	const fits = (at: Vec3) => {
		const own = moving.map((entity) => box(entity, at));
		return ([0, 1] as const).map((axis) => {
			if (!free[axis]) return null;
			// The guide runs across the view: along the other plan axis in a plan, up in an elevation.
			const along = free[1 - axis] ? 1 - axis : free[2] ? 2 : 1 - axis;
			let best: AxisFit | null = null;
			for (const mine of own)
				for (const other of others) {
					const fit = axisFit(mine, other, axis, along, threshold);
					if (fit && (!best || Math.abs(fit.gap) < Math.abs(best.gap))) best = fit;
				}
			return best;
		});
	};
	const found = fits(delta);
	if (!found.some(Boolean)) return null;
	const correction: Vec3 = [found[0]?.gap ?? 0, found[1]?.gap ?? 0, 0];
	// The guides are drawn where the elements land, once both axes have been put right.
	const landed = fits(addVec(delta, correction));
	const guides = found.flatMap((fit, axis) => (fit ? [(landed[axis] ?? fit).guide] : []));
	return { correction, guides };
}
