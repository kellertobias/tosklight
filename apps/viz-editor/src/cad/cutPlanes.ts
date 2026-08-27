import { useMemo } from "react";
import { rotateModelPoint } from "./projection";
import type { CadEntity, CadViewDirection } from "./types";

/**
 * How far into the drawing an element sits, from the viewer of one view.
 *
 * A plan is flat: two of the three axes become the page and the third disappears into it. That
 * third axis is what a cut plane works on, so it is named here rather than left implicit in the
 * projection.
 */
export function viewDepth(
	point: readonly [number, number, number],
	view: CadViewDirection,
): number {
	switch (view) {
		// Looking down, so the floor is far and the grid is near.
		case "top_down":
			return -point[2];
		case "left_to_right":
			return point[0];
		case "right_to_left":
			return -point[0];
		case "front_to_back":
			return point[1];
		case "back_to_front":
			return -point[1];
	}
}

export interface DepthRange {
	near: number;
	far: number;
}

/**
 * The near and far depth an element occupies.
 *
 * An element has thickness, so a cut plane that passes through it still shows it. Only an element
 * lying wholly beyond the cut is dropped.
 */
export function entityDepthRange(
	entity: Pick<
		CadEntity,
		"positionMillimetres" | "rotationDegrees" | "sizeMillimetres"
	>,
	view: CadViewDirection,
): DepthRange {
	const [width, depth, height] = entity.sizeMillimetres;
	const centre = viewDepth(entity.positionMillimetres, view);
	let half = 0;
	for (const x of [-width / 2, width / 2])
		for (const y of [-depth / 2, depth / 2])
			for (const z of [-height / 2, height / 2])
				half = Math.max(
					half,
					Math.abs(viewDepth(rotateModelPoint([x, y, z], entity.rotationDegrees), view)),
				);
	return { near: centre - half, far: centre + half };
}

/**
 * A slice of the drawing, in millimetres of depth from the viewer.
 *
 * Either end may be left open, which is what an operator means by "everything in front of the
 * upstage curtain" — a near limit and no far one.
 */
export interface CutPlanes {
	nearMillimetres: number | null;
	farMillimetres: number | null;
}

export const NO_CUT_PLANES: CutPlanes = {
	nearMillimetres: null,
	farMillimetres: null,
};

export function cutPlanesAreOpen(planes: CutPlanes | undefined): boolean {
	return (
		!planes ||
		(planes.nearMillimetres === null && planes.farMillimetres === null)
	);
}

/** Whether any part of an element falls inside the slice. */
export function withinCutPlanes(
	range: DepthRange,
	planes: CutPlanes | undefined,
): boolean {
	if (cutPlanesAreOpen(planes) || !planes) return true;
	const near = planes.nearMillimetres;
	const far = planes.farMillimetres;
	// A pair given the wrong way round is the same slice read backwards.
	const low = near !== null && far !== null ? Math.min(near, far) : near;
	const high = near !== null && far !== null ? Math.max(near, far) : far;
	if (low !== null && range.far < low) return false;
	if (high !== null && range.near > high) return false;
	return true;
}

/** The elements one view shows, once its cut planes are applied. */
export function visibleEntities<
	Entity extends Pick<
		CadEntity,
		"positionMillimetres" | "rotationDegrees" | "sizeMillimetres"
	>,
>(
	entities: readonly Entity[],
	view: CadViewDirection,
	planes: CutPlanes | undefined,
): readonly Entity[] {
	if (cutPlanesAreOpen(planes)) return entities;
	return entities.filter((entity) =>
		withinCutPlanes(entityDepthRange(entity, view), planes),
	);
}

/** The depth every element in a view spans, so a control can offer sensible ends. */
export function depthExtent(
	entities: readonly Pick<
		CadEntity,
		"positionMillimetres" | "rotationDegrees" | "sizeMillimetres"
	>[],
	view: CadViewDirection,
): DepthRange | null {
	let near = Number.POSITIVE_INFINITY;
	let far = Number.NEGATIVE_INFINITY;
	for (const entity of entities) {
		const range = entityDepthRange(entity, view);
		near = Math.min(near, range.near);
		far = Math.max(far, range.far);
	}
	return Number.isFinite(near) && Number.isFinite(far) ? { near, far } : null;
}

/** The elements a view shows, held steady while its cut planes do not change. */
export function useVisibleEntities<
	Entity extends Pick<
		CadEntity,
		"positionMillimetres" | "rotationDegrees" | "sizeMillimetres"
	>,
>(
	entities: readonly Entity[],
	view: CadViewDirection,
	planes: CutPlanes | undefined,
): readonly Entity[] {
	return useMemo(
		() => visibleEntities(entities, view, planes),
		[entities, view, planes],
	);
}
