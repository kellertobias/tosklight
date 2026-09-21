/**
 * Plan-space geometry: where an entity's drawing lands on a view, how deep it sits, and what a
 * pointer at a plan coordinate is over.
 *
 * Everything here is pure and shared by the two consumers that must agree pixel for pixel: the
 * viewport renderer that draws the plan, and the picking that decides what an operator just
 * clicked on.
 */
import {
	entityPlanGeometry,
	type PlanGeometry,
	type PlanPoint,
	rotateDeskPoint,
} from "./projection";
import type {
	CadDrawing,
	CadEntity,
	CadViewDirection,
	TileCamera,
} from "./types";
import { planeDelta, projectPoint } from "./types";

/** The axis a move is constrained to while a drag is in flight. */
export type MoveAxis = "plane" | "horizontal" | "vertical";

/** The Show screen's read-only plan always looks down, turned a quarter turn anticlockwise. */
export const OVERVIEW_VIEW: CadViewDirection = "top_down";
export const OVERVIEW_ROTATION_QUARTER_TURNS = -1;

/** A camera that frames every entity of a plan inside the given viewport. */
export function fitCadOverview(
	entities: readonly CadEntity[],
	drawings: ReadonlyMap<string, CadDrawing>,
	viewportWidth: number,
	viewportHeight: number,
): TileCamera {
	if (!entities.length) return { pan: [0, 0], zoom: 0.08 };
	const points: PlanPoint[] = [];
	for (const entity of entities) {
		const geometry = worldGeometry(
			entity,
			entityPlanGeometry(entity, drawings.get(entity.drawingId), OVERVIEW_VIEW),
			OVERVIEW_VIEW,
			OVERVIEW_ROTATION_QUARTER_TURNS,
		);
		for (const triangle of geometry.triangles) points.push(...triangle.points);
		for (const outline of geometry.outlines) points.push(...outline);
		for (const line of geometry.lines) points.push(...line.points);
	}
	if (!points.length) {
		points.push(
			...entities.map((entity) =>
				projectPoint(
					entity.positionMillimetres,
					OVERVIEW_VIEW,
					OVERVIEW_ROTATION_QUARTER_TURNS,
				),
			),
		);
	}
	const minX = Math.min(...points.map((point) => point[0]));
	const maxX = Math.max(...points.map((point) => point[0]));
	const minY = Math.min(...points.map((point) => point[1]));
	const maxY = Math.max(...points.map((point) => point[1]));
	const width = Math.max(500, maxX - minX);
	const height = Math.max(500, maxY - minY);
	const availableWidth = Math.max(1, viewportWidth - 64);
	const availableHeight = Math.max(1, viewportHeight - 64);
	return {
		pan: [-(minX + maxX) / 2, -(minY + maxY) / 2],
		zoom: Math.max(
			0.001,
			Math.min(2.5, availableWidth / width, availableHeight / height),
		),
	};
}

/**
 * Where the two axes of an entity's own drawing end up on the page, once the entity is turned.
 *
 * A drawing is a flat slice: its points are `[across, up]` in the unturned axes of the view it was
 * built for. Turning it therefore means turning that slice's two axes in the show and projecting
 * each — not spinning the points about the page, which can only express yaw seen from above. Read
 * this way a lamp rolled 180 degrees about X hangs upside down in an elevation, and a truss yawed
 * away from the viewer foreshortens, both of which a single page angle cannot say.
 *
 * `live_model` geometry has already been turned in three dimensions when it was projected, so its
 * axes are taken as they are.
 */
function planBasis(
	entity: CadEntity,
	geometry: PlanGeometry,
	view: CadViewDirection,
	rotationQuarterTurns: number,
): [PlanPoint, PlanPoint] {
	const rotation: readonly [number, number, number] =
		geometry.source === "live_model" ? [0, 0, 0] : entity.rotationDegrees;
	const axis = (local: [number, number]): PlanPoint =>
		projectPoint(
			rotateDeskPoint(planeDelta(local, view), rotation),
			view,
			rotationQuarterTurns,
		);
	return [axis([1, 0]), axis([0, 1])];
}

/**
 * Where a point of an entity's own drawing lands on the page.
 *
 * Shared by the screen and the printed plan so the two cannot drift apart.
 */
export function planTransform(
	entity: CadEntity,
	geometry: PlanGeometry,
	view: CadViewDirection,
	rotationQuarterTurns: number,
	offset: readonly [number, number] = [0, 0],
): (point: PlanPoint) => PlanPoint {
	const centre = projectPoint(
		entity.positionMillimetres,
		view,
		rotationQuarterTurns,
	);
	const [across, up] = planBasis(entity, geometry, view, rotationQuarterTurns);
	return (point) => [
		centre[0] + point[0] * across[0] + point[1] * up[0] + offset[0],
		centre[1] + point[0] * across[1] + point[1] * up[1] + offset[1],
	];
}

/** An entity's own drawing, moved and turned into the plan the view shows. */
export function worldGeometry(
	entity: CadEntity,
	geometry: PlanGeometry,
	view: CadViewDirection,
	rotationQuarterTurns: number,
	offset: readonly [number, number] = [0, 0],
): PlanGeometry {
	const transform = planTransform(
		entity,
		geometry,
		view,
		rotationQuarterTurns,
		offset,
	);
	return {
		...geometry,
		triangles: geometry.triangles.map((triangle) => ({
			...triangle,
			points: triangle.points.map(transform) as [
				PlanPoint,
				PlanPoint,
				PlanPoint,
			],
		})),
		outlines: geometry.outlines.map((outline) => outline.map(transform)),
		lines: geometry.lines.map((line) => ({
			...line,
			points: line.points.map(transform) as [PlanPoint, PlanPoint],
		})),
	};
}

export function pointInTriangle(
	point: PlanPoint,
	triangle: [PlanPoint, PlanPoint, PlanPoint],
): boolean {
	const [a, b, c] = triangle;
	const area = (first: PlanPoint, second: PlanPoint, third: PlanPoint) =>
		(first[0] - third[0]) * (second[1] - third[1]) -
		(second[0] - third[0]) * (first[1] - third[1]);
	const total = area(a, b, c);
	if (Math.abs(total) < 0.0001) return false;
	const first = area(point, b, c) / total;
	const second = area(a, point, c) / total;
	const third = 1 - first - second;
	return first >= 0 && second >= 0 && third >= 0;
}

export function pointInPolygon(
	point: PlanPoint,
	polygon: readonly PlanPoint[],
): boolean {
	let inside = false;
	for (
		let current = 0, previous = polygon.length - 1;
		current < polygon.length;
		previous = current++
	) {
		const [currentX, currentY] = polygon[current];
		const [previousX, previousY] = polygon[previous];
		const crosses =
			currentY > point[1] !== previousY > point[1] &&
			point[0] <
				((previousX - currentX) * (point[1] - currentY)) /
					(previousY - currentY) +
					currentX;
		if (crosses) inside = !inside;
	}
	return inside;
}

/** Paint order along the axis the view looks down. */
export function viewDepth(entity: CadEntity, view: CadViewDirection): number {
	switch (view) {
		case "top_down":
			return entity.positionMillimetres[2];
		case "left_to_right":
			return -entity.positionMillimetres[0];
		case "right_to_left":
			return entity.positionMillimetres[0];
		case "front_to_back":
			return -entity.positionMillimetres[1];
		case "back_to_front":
			return entity.positionMillimetres[1];
	}
}

export function viewPositionDepth(
	position: readonly [number, number, number],
	view: CadViewDirection,
): number {
	// Matches the renderer-world depth convention used by live model triangles. Smaller values
	// are closer to the orthographic camera and therefore win the WebGL depth test.
	switch (view) {
		case "top_down":
			return -position[2];
		case "left_to_right":
			return position[0];
		case "right_to_left":
			return -position[0];
		case "front_to_back":
			return position[1];
		case "back_to_front":
			return -position[1];
	}
}

/**
 * The move gizmo's placement in plan millimetres: at one element's own origin, or at the centre of
 * the box around a selected group's origins.
 */
export function gizmoGeometry(
	entities: readonly CadEntity[],
	selected: ReadonlySet<string>,
	view: CadViewDirection,
	rotationQuarterTurns: number,
	camera: TileCamera,
	preview: readonly [number, number] = [0, 0],
) {
	const points = entities
		.filter(
			(entity) => entity.selectable && selected.has(entity.logicalFixtureId),
		)
		.map((entity) =>
			projectPoint(entity.positionMillimetres, view, rotationQuarterTurns),
		);
	if (!points.length) return null;
	const middle = (axis: 0 | 1) =>
		(Math.min(...points.map((point) => point[axis])) +
			Math.max(...points.map((point) => point[axis]))) /
		2;
	return {
		origin: [middle(0) + preview[0], middle(1) + preview[1]] as [number, number],
		length: 48 / camera.zoom,
		square: 7 / camera.zoom,
	};
}

export function pickGizmo(
	point: [number, number],
	entities: readonly CadEntity[],
	selected: ReadonlySet<string>,
	view: CadViewDirection,
	rotationQuarterTurns: number,
	camera: TileCamera,
): MoveAxis | null {
	const gizmo = gizmoGeometry(
		entities,
		selected,
		view,
		rotationQuarterTurns,
		camera,
	);
	if (!gizmo) return null;
	const { origin, length } = gizmo;
	const tolerance = 10 / camera.zoom;
	if (Math.hypot(point[0] - origin[0], point[1] - origin[1]) <= tolerance)
		return "plane";
	if (
		point[0] >= origin[0] &&
		point[0] <= origin[0] + length &&
		Math.abs(point[1] - origin[1]) <= tolerance
	)
		return "horizontal";
	if (
		point[1] >= origin[1] &&
		point[1] <= origin[1] + length &&
		Math.abs(point[0] - origin[0]) <= tolerance
	)
		return "vertical";
	return null;
}

/**
 * The entity under a plan point: the nearest one whose drawing actually covers the point, else the
 * nearest within a generous radius so a small symbol stays clickable. Locked entities are skipped,
 * since an operator cannot pick what they cannot move.
 */
export function pickEntity(
	point: PlanPoint,
	entities: readonly CadEntity[],
	drawings: ReadonlyMap<string, CadDrawing>,
	view: CadViewDirection,
	rotationQuarterTurns: number,
	camera: TileCamera,
): CadEntity | null {
	const ordered = [...entities].sort(
		(left, right) => viewDepth(right, view) - viewDepth(left, view),
	);
	let best: { entity: CadEntity; distance: number } | null = null;
	for (const entity of ordered) {
		if (!entity.selectable) continue;
		const projected = projectPoint(
			entity.positionMillimetres,
			view,
			rotationQuarterTurns,
		);
		const geometry = worldGeometry(
			entity,
			entityPlanGeometry(entity, drawings.get(entity.drawingId), view),
			view,
			rotationQuarterTurns,
		);
		if (
			geometry.triangles.some((triangle) =>
				pointInTriangle(point, triangle.points),
			)
		)
			return entity;
		if (geometry.outlines.some((outline) => pointInPolygon(point, outline)))
			return entity;
		const distance = Math.hypot(
			projected[0] - point[0],
			projected[1] - point[1],
		);
		const threshold = Math.max(90, 8 / camera.zoom);
		if (distance <= threshold && (!best || distance < best.distance)) {
			best = { entity, distance };
		}
	}
	return best?.entity ?? null;
}

/** World-space guide bounds that remain just outside both screen edges at any pan and zoom. */
export function viewportGuideRange(
	horizontal: boolean,
	camera: TileCamera,
	viewportWidth: number,
	viewportHeight: number,
): [number, number] {
	const pixels = horizontal ? viewportWidth : viewportHeight;
	const pan = horizontal ? camera.pan[0] : camera.pan[1];
	const halfVisible = pixels / (2 * camera.zoom);
	const margin = 16 / camera.zoom;
	return [-pan - halfVisible - margin, -pan + halfVisible + margin];
}
