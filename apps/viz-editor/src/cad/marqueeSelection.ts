import { rotateModelPoint } from "./projection";
import { type CadEntity, type CadViewDirection, projectPoint } from "./types";

export interface PlaneBounds {
	minimum: [number, number];
	maximum: [number, number];
}

/**
 * How a marquee decides what it caught.
 *
 * Dragging away from the start to the right takes only what fits entirely inside the rectangle,
 * which is how an operator picks a run of fixtures out of a crowded plot. Dragging back to the left
 * takes everything the rectangle touches, which is how the same operator sweeps a whole truss
 * without having to enclose its ends.
 */
export type MarqueeMode = "enclose" | "touch";

/** Which way the drag went, in what the operator sees rather than in plane coordinates. */
export function marqueeMode(startX: number, endX: number): MarqueeMode {
	return endX >= startX ? "enclose" : "touch";
}

export function boundsOf(
	start: readonly [number, number],
	end: readonly [number, number],
): PlaneBounds {
	return {
		minimum: [Math.min(start[0], end[0]), Math.min(start[1], end[1])],
		maximum: [Math.max(start[0], end[0]), Math.max(start[1], end[1])],
	};
}

/**
 * The rectangle an entity covers in the drawing plane.
 *
 * The eight corners of the entity's box are turned by its own rotation and then projected, so a
 * truss lying across the plot covers the run it is drawn along rather than the square it would
 * occupy unturned.
 */
export function entityBounds(
	entity: Pick<
		CadEntity,
		"positionMillimetres" | "rotationDegrees" | "sizeMillimetres"
	>,
	view: CadViewDirection,
	rotationQuarterTurns = 0,
): PlaneBounds {
	const [width, depth, height] = entity.sizeMillimetres;
	const minimum: [number, number] = [
		Number.POSITIVE_INFINITY,
		Number.POSITIVE_INFINITY,
	];
	const maximum: [number, number] = [
		Number.NEGATIVE_INFINITY,
		Number.NEGATIVE_INFINITY,
	];
	for (const x of [-width / 2, width / 2])
		for (const y of [-depth / 2, depth / 2])
			for (const z of [-height / 2, height / 2]) {
				const turned = rotateModelPoint([x, y, z], entity.rotationDegrees);
				const corner = projectPoint(
					[
						entity.positionMillimetres[0] + turned[0],
						entity.positionMillimetres[1] + turned[1],
						entity.positionMillimetres[2] + turned[2],
					],
					view,
					rotationQuarterTurns,
				);
				minimum[0] = Math.min(minimum[0], corner[0]);
				minimum[1] = Math.min(minimum[1], corner[1]);
				maximum[0] = Math.max(maximum[0], corner[0]);
				maximum[1] = Math.max(maximum[1], corner[1]);
			}
	return { minimum, maximum };
}

export function marqueeCatches(
	entity: PlaneBounds,
	marquee: PlaneBounds,
	mode: MarqueeMode,
): boolean {
	if (mode === "enclose")
		return (
			entity.minimum[0] >= marquee.minimum[0] &&
			entity.minimum[1] >= marquee.minimum[1] &&
			entity.maximum[0] <= marquee.maximum[0] &&
			entity.maximum[1] <= marquee.maximum[1]
		);
	return (
		entity.minimum[0] <= marquee.maximum[0] &&
		entity.maximum[0] >= marquee.minimum[0] &&
		entity.minimum[1] <= marquee.maximum[1] &&
		entity.maximum[1] >= marquee.minimum[1]
	);
}
