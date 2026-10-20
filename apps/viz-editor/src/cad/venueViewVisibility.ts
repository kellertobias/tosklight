/** Viewport cutaway is drawing policy; authored solids and native light blockers stay intact. */
import type { PlanGeometry, PlanLine } from "./projection";
import { rotateModelPoint } from "./projection";
import { cameraVector, dot3 } from "./projectionMath";
import type { CadEntity, CadViewDirection } from "./types";
import { venueWorldCentre } from "./venueGeometryPoints";

type Point = [number, number, number];
interface Subject {
	id: string;
	centre: Point;
	wall: { normal: Point; corners: Point[]; thickness: number } | null;
}
interface SceneVisibility {
	subjects: Subject[];
	views: Map<CadViewDirection, ReadonlySet<string>>;
}
const scenes = new WeakMap<readonly CadEntity[], SceneVisibility>();

/** Immutable CAD scene arrays share their bounds and per-view decisions across every frame. */
export function outlineVenueIds(
	entities: readonly CadEntity[],
	view: CadViewDirection,
): ReadonlySet<string> {
	let scene = scenes.get(entities);
	if (!scene) {
		scene = { subjects: entities.map(subject), views: new Map() };
		scenes.set(entities, scene);
	}
	let outlined = scene.views.get(view);
	if (!outlined) {
		outlined =
			view === "top_down" ? new Set() : occludingWalls(scene.subjects, view);
		scene.views.set(view, outlined);
	}
	return outlined;
}

function subject(entity: CadEntity): Subject {
	const isWall =
		entity.scenery?.architecture?.kind === "wall" ||
		entity.scenery?.kind === "wall";
	const [x, y, z] = entity.positionMillimetres;
	const world = (point: Point): Point => {
		const offset = rotateModelPoint(point, entity.rotationDegrees);
		return [x + offset[0], z + offset[1], -y + offset[2]];
	};
	if (!isWall) {
		const centre = venueWorldCentre(entity) ?? entity.positionMillimetres;
		return {
			id: entity.id,
			centre: [centre[0], centre[2], -centre[1]],
			wall: null,
		};
	}
	const [width, depth, height] = entity.sizeMillimetres;
	const base = entity.scenery?.architecture?.kind === "wall" ? 0 : -height / 2;
	const corners: Point[] = [];
	for (const across of [-width / 2, width / 2])
		for (const up of [base, base + height])
			for (const along of [-depth / 2, depth / 2])
				corners.push(world([across, up, along]));
	return {
		id: entity.id,
		centre: world([0, base + height / 2, 0]),
		wall: {
			normal: rotateModelPoint([0, 0, 1], entity.rotationDegrees),
			corners,
			thickness: depth,
		},
	};
}

function screen(point: Point, view: CadViewDirection): [number, number] {
	switch (view) {
		case "top_down":
			return [point[0], -point[2]];
		case "left_to_right":
			return [-point[2], point[1]];
		case "right_to_left":
			return [point[2], point[1]];
		case "front_to_back":
			return [point[0], point[1]];
		case "back_to_front":
			return [-point[0], point[1]];
	}
}

function occludingWalls(
	subjects: readonly Subject[],
	view: CadViewDirection,
): Set<string> {
	const camera = cameraVector(view);
	const projected = subjects.map((entry) => ({
		...entry,
		point: screen(entry.centre, view),
		depth: dot3(entry.centre, camera),
	}));
	const result = new Set<string>();
	for (const entity of projected) {
		if (!entity.wall || Math.abs(dot3(entity.wall.normal, camera)) < 0.05)
			continue;
		const thickness = entity.wall.thickness;
		const corners = entity.wall.corners.map((point) => screen(point, view));
		const low = [
			Math.min(...corners.map((p) => p[0])),
			Math.min(...corners.map((p) => p[1])),
		];
		const high = [
			Math.max(...corners.map((p) => p[0])),
			Math.max(...corners.map((p) => p[1])),
		];
		if (
			projected.some(
				(other) =>
					other.id !== entity.id &&
					other.depth < entity.depth - Math.max(0.5, thickness / 2) &&
					other.point[0] >= low[0] &&
					other.point[0] <= high[0] &&
					other.point[1] >= low[1] &&
					other.point[1] <= high[1],
			)
		)
			result.add(entity.id);
	}
	return result;
}

/** Keep perimeter/opening ink selectable, while its empty interior admits the rig behind it. */
export function outlineVenueGeometry(geometry: PlanGeometry): PlanGeometry {
	const edges: PlanLine[] = geometry.outlines.flatMap((outline) =>
		outline.map((point, index) => ({
			points: [
				point,
				outline[(index + 1) % outline.length],
			] as PlanLine["points"],
		})),
	);
	return {
		...geometry,
		triangles: [],
		pickTriangles: [],
		outlines: [],
		lines: [...geometry.lines, ...edges],
	};
}
