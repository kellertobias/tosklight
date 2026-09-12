/**
 * Where a placed drawing lands on a plan.
 *
 * A drawing arrives Y-up in millimetres, in the space it was drawn in. Its placement — an origin,
 * a scale and a rotation — moves it into the show, and the view it was assigned to decides which
 * plan it appears on. A rotated top-down tile turns its drawings with it, exactly as it turns the
 * rig, so a plan and the fixtures over it never come apart.
 *
 * Cut planes deliberately do not apply: a drawing is paper on the projection plane, not rig
 * geometry at a depth, so slicing a view to the stage still shows the venue it stands in.
 */
import type { PlanPoint } from "./projection";
import type { CadPrintPage, CadViewDirection } from "./types";
import { projectPoint } from "./types";
import type { CadUnderlay } from "./underlays";

/** The drawings on a view, in the order they were placed. */
export function underlaysForView(
	underlays: readonly CadUnderlay[],
	view: CadViewDirection,
): CadUnderlay[] {
	return underlays.filter(
		(underlay) => underlay.visible && underlay.view === view,
	);
}

/**
 * The drawings a page prints: everything on its axis that the page has not switched off.
 *
 * A page saved before drawings existed hides none, which is why the field is optional and absent
 * means "print them all".
 */
export function underlaysForPage(
	underlays: readonly CadUnderlay[],
	page: Pick<CadPrintPage, "view" | "hiddenUnderlayIds">,
): CadUnderlay[] {
	const hidden = new Set(page.hiddenUnderlayIds ?? []);
	return underlaysForView(underlays, page.view).filter(
		(underlay) => !hidden.has(underlay.id),
	);
}

/** Whether a page prints one particular drawing. */
export function pageShowsUnderlay(
	page: Pick<CadPrintPage, "hiddenUnderlayIds">,
	id: string,
): boolean {
	return !(page.hiddenUnderlayIds ?? []).includes(id);
}

/** The page's hidden list with one drawing switched on or off. */
export function withUnderlayShown(
	page: Pick<CadPrintPage, "hiddenUnderlayIds">,
	id: string,
	shown: boolean,
): string[] {
	const hidden = new Set(page.hiddenUnderlayIds ?? []);
	if (shown) hidden.delete(id);
	else hidden.add(id);
	return [...hidden];
}

/**
 * One placed drawing as plan-space runs of points.
 *
 * A closed run comes back with its first point repeated, so a caller can draw every run as an open
 * polyline and still see a closed shape.
 */
export function placedPolylines(
	underlay: CadUnderlay,
	rotationQuarterTurns = 0,
): PlanPoint[][] {
	const angle = (underlay.rotationDegrees * Math.PI) / 180;
	const cosine = Math.cos(angle);
	const sine = Math.sin(angle);
	const scale = Number.isFinite(underlay.scale) ? underlay.scale : 1;
	const place = (point: readonly [number, number]): PlanPoint => {
		const x = point[0] * scale;
		const y = point[1] * scale;
		const placed: PlanPoint = [
			underlay.originMillimetres[0] + x * cosine - y * sine,
			underlay.originMillimetres[1] + x * sine + y * cosine,
		];
		// Only a top-down tile can be turned, and it turns its drawings with the rig.
		return underlay.view === "top_down"
			? projectPoint(
					[placed[0], placed[1], 0],
					underlay.view,
					rotationQuarterTurns,
				)
			: placed;
	};
	return underlay.geometry.polylines
		.filter((polyline) => polyline.points.length >= 2)
		.map((polyline) => {
			const points = polyline.points.map(place);
			if (polyline.closed && points.length > 2) points.push(points[0]);
			return points;
		});
}

/** A cache key that changes whenever a drawing's placement or content would draw differently. */
export function underlayKey(
	underlay: CadUnderlay,
	rotationQuarterTurns: number,
): string {
	return [
		underlay.id,
		underlay.originMillimetres.join(","),
		underlay.scale,
		underlay.rotationDegrees,
		rotationQuarterTurns,
		underlay.geometry.polylines.length,
	].join(":");
}

/** The box a placed drawing covers on the plan, in millimetres. */
export function placedExtents(
	underlay: CadUnderlay,
	rotationQuarterTurns = 0,
): [number, number, number, number] | null {
	const runs = placedPolylines(underlay, rotationQuarterTurns);
	if (!runs.length) return null;
	let minX = Number.POSITIVE_INFINITY;
	let minY = Number.POSITIVE_INFINITY;
	let maxX = Number.NEGATIVE_INFINITY;
	let maxY = Number.NEGATIVE_INFINITY;
	for (const run of runs)
		for (const point of run) {
			minX = Math.min(minX, point[0]);
			minY = Math.min(minY, point[1]);
			maxX = Math.max(maxX, point[0]);
			maxY = Math.max(maxY, point[1]);
		}
	return [minX, minY, maxX, maxY];
}
