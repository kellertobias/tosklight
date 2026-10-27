import type { LabelBounds } from "./labelPlacement";
import { planTransform } from "./planGeometry";
import { entityPlanGeometry, type PlanPoint } from "./projection";
import type { CadDrawing, CadEntity, CadViewDirection } from "./types";

/** Reserve the actual lamp drawing, including its rotated outline and mounting hardware. */
export function fixtureLabelObstacle(entity: CadEntity, drawing: CadDrawing | undefined,
	view: CadViewDirection, rotation: number, point: (p: PlanPoint) => PlanPoint,
	mountingHardware = true): LabelBounds {
	const geometry = entityPlanGeometry(entity, drawing, view, { mountingHardware });
	const transform = planTransform(entity, geometry, view, rotation);
	let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
	const include = (p: PlanPoint) => {
		const [x, y] = point(transform(p));
		minX = Math.min(minX, x); maxX = Math.max(maxX, x);
		minY = Math.min(minY, y); maxY = Math.max(maxY, y);
	};
	for (const triangle of geometry.triangles) for (const p of triangle.points) include(p);
	for (const outline of geometry.outlines) for (const p of outline) include(p);
	for (const line of geometry.lines) for (const p of line.points) include(p);
	if (!Number.isFinite(minX)) include([0, 0]);
	return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
}
