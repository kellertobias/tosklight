/**
 * Where a point of a drawn line or box lands, so a plan can be drawn to exact lengths.
 *
 * In order, the first that applies wins:
 *
 * - A corner of a truss (the corners of its box and its connectors) or of a stage element within
 *   reach: the point lands exactly on it, and a marker shows the fit.
 * - A line's next point near level with, or plumb over, the point before it: that coordinate is
 *   locked to the previous point's, so the segment is exactly horizontal or vertical on the tile.
 * - Everything else: whole 10 cm on the plan.
 *
 * Every test is made on the tile's own plan, so a rotated top-down view snaps along its screen.
 */
import type { PlanPoint } from "./projection";
import { type CadEntity, type CadViewDirection, projectPoint } from "./types";
import {
	isStageElement,
	isTruss,
	stageCorners,
	trussConnectors,
	trussCornerArms,
	type Vec3,
} from "./venueShapes";

/** The drawing grid a free point lands on, in millimetres. */
export const DRAW_GRID_MILLIMETRES = 100;

export interface DrawingSnap {
	point: PlanPoint;
	/** Where a corner fit was found; null when the point only locked to an axis or the grid. */
	marker: PlanPoint | null;
}

/** The corners a drawn point snaps onto: every truss's and stage element's. */
export function drawingCorners(entities: readonly CadEntity[]): Vec3[] {
	return entities.flatMap((entity) => {
		if (isStageElement(entity)) return stageCorners(entity);
		if (isTruss(entity) || trussCornerArms(entity))
			return [...stageCorners(entity), ...trussConnectors(entity)];
		return [];
	});
}

function onGrid(value: number): number {
	return Math.round(value / DRAW_GRID_MILLIMETRES) * DRAW_GRID_MILLIMETRES + 0;
}

/**
 * The point `point` put on a tile snapped onto a corner within `threshold` plan millimetres, else
 * locked level or plumb with `anchor` (a line's previous point) when within `threshold` of it, and
 * onto the drawing grid on every coordinate left free.
 */
export function snapDrawingPoint(
	corners: readonly Vec3[],
	point: PlanPoint,
	view: CadViewDirection,
	rotationQuarterTurns: number,
	threshold: number,
	anchor: PlanPoint | null,
): DrawingSnap {
	let best: { point: PlanPoint; distance: number } | null = null;
	for (const corner of corners) {
		const projected = projectPoint(corner, view, rotationQuarterTurns);
		const distance = Math.hypot(projected[0] - point[0], projected[1] - point[1]);
		if (distance <= threshold && (!best || distance < best.distance))
			best = { point: projected, distance };
	}
	if (best) return { point: best.point, marker: best.point };
	const snapped: PlanPoint = [onGrid(point[0]), onGrid(point[1])];
	if (anchor) {
		const level = Math.abs(point[1] - anchor[1]);
		const plumb = Math.abs(point[0] - anchor[0]);
		if (level <= threshold && level <= plumb) snapped[1] = anchor[1];
		else if (plumb <= threshold) snapped[0] = anchor[0];
	}
	return { point: snapped, marker: null };
}
