/**
 * Where a drawn item lands on a plan, what it draws, and what the pointer is over.
 *
 * Items are stored before a top-down tile's rotation, exactly like placed venue drawings, so a
 * rotated plan turns them with the rig; elevations are never rotated.
 */
import type { CadAnnotation } from "./annotations";
import type { PlanPoint } from "./projection";
import { type CadViewDirection, projectPoint } from "./types";

/** How long a measurement's end ticks reach either side of its line, in millimetres. */
export const MEASURE_TICK_MILLIMETRES = 150;

/** The items drawn on a view, in the order they were drawn. */
export function annotationsForView(
	annotations: readonly CadAnnotation[],
	view: CadViewDirection,
): CadAnnotation[] {
	return annotations.filter((annotation) => annotation.view === view);
}

/** A rotation can turn a zero negative; a stored or drawn point never carries one. */
function unsigned(point: readonly [number, number]): [number, number] {
	return [point[0] + 0, point[1] + 0];
}

/** A stored point as a tile draws it. */
export function viewPoint(
	view: CadViewDirection,
	point: readonly [number, number],
	rotationQuarterTurns = 0,
): PlanPoint {
	return unsigned(
		view === "top_down"
			? projectPoint([point[0], point[1], 0], view, rotationQuarterTurns)
			: point,
	);
}

/** Where a point the operator put on a tile is stored: the inverse of `viewPoint`. */
export function storedPoint(
	view: CadViewDirection,
	point: readonly [number, number],
	rotationQuarterTurns = 0,
): [number, number] {
	return unsigned(
		view === "top_down"
			? projectPoint([point[0], point[1], 0], view, -rotationQuarterTurns)
			: point,
	);
}

/** The straight runs an item draws, in the tile's plan millimetres. Text draws none. */
export function annotationRuns(
	annotation: CadAnnotation,
	rotationQuarterTurns = 0,
): PlanPoint[][] {
	const place = (point: readonly [number, number]) =>
		viewPoint(annotation.view, point, rotationQuarterTurns);
	const { points } = annotation;
	switch (annotation.kind) {
		case "polyline": {
			if (points.length < 2) return [];
			const run = points.map(place);
			if (annotation.closed && run.length > 2) run.push(run[0]);
			return [run];
		}
		case "box": {
			if (points.length < 2) return [];
			const [[x0, y0], [x1, y1]] = points;
			return [
				[
					[x0, y0],
					[x1, y0],
					[x1, y1],
					[x0, y1],
					[x0, y0],
				].map((corner) => place(corner as [number, number])),
			];
		}
		case "measure":
			return points.length < 2 ? [] : measureRuns(place(points[0]), place(points[1]));
		case "text":
			return [];
	}
}

/** A dimension line and a tick across each end. */
function measureRuns(start: PlanPoint, end: PlanPoint): PlanPoint[][] {
	const length = Math.hypot(end[0] - start[0], end[1] - start[1]);
	if (length === 0) return [];
	const normal: PlanPoint = [
		(-(end[1] - start[1]) / length) * MEASURE_TICK_MILLIMETRES,
		((end[0] - start[0]) / length) * MEASURE_TICK_MILLIMETRES,
	];
	const tick = (point: PlanPoint): PlanPoint[] => [
		[point[0] + normal[0], point[1] + normal[1]],
		[point[0] - normal[0], point[1] - normal[1]],
	];
	return [[start, end], tick(start), tick(end)];
}

/** The distance a measurement spans, which no rotation changes. */
export function measurementLength(annotation: CadAnnotation): number {
	const [start, end] = annotation.points;
	return start && end ? Math.hypot(end[0] - start[0], end[1] - start[1]) : 0;
}

/** A measured distance as a plan reads it: millimetres below a metre, metres above. */
export function formatMeasurement(millimetres: number): string {
	return millimetres < 1000
		? `${Math.round(millimetres)} mm`
		: `${(millimetres / 1000).toFixed(2)} m`;
}

export interface AnnotationLabel {
	id: string;
	/** `length` is the live length of the segment a line in progress is drawing. */
	kind: "text" | "measure" | "length";
	/** For text, the anchor its baseline starts at; for a measurement or a length, its midpoint. */
	point: PlanPoint;
	text: string;
	/** Text's own height; a measurement's label follows the screen instead. */
	heightMillimetres: number | null;
}

/**
 * The words the items on a tile show: their text, each measurement's distance, and the length of
 * the segment a line in progress (the `draft`) is drawing up to the pointer.
 */
export function annotationLabels(
	annotations: readonly CadAnnotation[],
	rotationQuarterTurns = 0,
): AnnotationLabel[] {
	return annotations.flatMap((annotation): AnnotationLabel[] => {
		const place = (point: readonly [number, number]) =>
			viewPoint(annotation.view, point, rotationQuarterTurns);
		if (annotation.kind === "text" && annotation.points[0])
			return [
				{
					id: annotation.id,
					kind: "text",
					point: place(annotation.points[0]),
					text: annotation.text,
					heightMillimetres: annotation.textHeightMillimetres,
				},
			];
		if (annotation.kind === "measure" && annotation.points.length >= 2) {
			const [start, end] = annotation.points.map(place);
			return [
				{
					id: annotation.id,
					kind: "measure",
					point: [(start[0] + end[0]) / 2, (start[1] + end[1]) / 2],
					text: formatMeasurement(measurementLength(annotation)),
					heightMillimetres: null,
				},
			];
		}
		if (annotation.id === "draft" && annotation.kind === "polyline" && annotation.points.length >= 2) {
			const [start, end] = annotation.points.slice(-2).map(place);
			return [
				{
					id: annotation.id,
					kind: "length",
					point: [(start[0] + end[0]) / 2, (start[1] + end[1]) / 2],
					text: formatMeasurement(Math.hypot(end[0] - start[0], end[1] - start[1])),
					heightMillimetres: null,
				},
			];
		}
		return [];
	});
}

function segmentDistance(point: PlanPoint, a: PlanPoint, b: PlanPoint): number {
	const dx = b[0] - a[0];
	const dy = b[1] - a[1];
	const lengthSquared = dx * dx + dy * dy;
	const along = lengthSquared
		? Math.max(
				0,
				Math.min(
					1,
					((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / lengthSquared,
				),
			)
		: 0;
	return Math.hypot(point[0] - (a[0] + dx * along), point[1] - (a[1] + dy * along));
}

/**
 * The item under a point of a tile, or null. Lines are caught within `tolerance` millimetres;
 * text anywhere over the box its words roughly cover.
 */
export function hitAnnotation(
	annotations: readonly CadAnnotation[],
	point: PlanPoint,
	rotationQuarterTurns: number,
	tolerance: number,
): string | null {
	let best: { id: string; distance: number } | null = null;
	for (const annotation of annotations) {
		let distance = Number.POSITIVE_INFINITY;
		for (const run of annotationRuns(annotation, rotationQuarterTurns))
			for (let index = 1; index < run.length; index++)
				distance = Math.min(distance, segmentDistance(point, run[index - 1], run[index]));
		if (annotation.kind === "text" && annotation.points[0]) {
			const [x, y] = viewPoint(
				annotation.view,
				annotation.points[0],
				rotationQuarterTurns,
			);
			const height = annotation.textHeightMillimetres;
			const width = height * 0.6 * Math.max(1, annotation.text.length);
			if (
				point[0] >= x - tolerance &&
				point[0] <= x + width + tolerance &&
				point[1] >= y - tolerance &&
				point[1] <= y + height + tolerance
			)
				distance = 0;
		}
		if (distance <= tolerance && (!best || distance < best.distance))
			best = { id: annotation.id, distance };
	}
	return best?.id ?? null;
}
